use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use azalea_buf::AzBuf;
use azalea_core::bitset::FixedBitSet;
use azalea_protocol::address::ServerAddr;
use azalea_protocol::packets::ClientIntention;
use azalea_protocol::packets::config::{ClientboundConfigPacket, ServerboundConfigPacket};
use azalea_protocol::packets::game::{ClientboundGamePacket, ServerboundGamePacket};
use azalea_protocol::packets::login::c_hello::ClientboundHello;
use azalea_protocol::packets::login::s_hello::ServerboundHello;
use azalea_protocol::packets::login::s_key::ServerboundKey;
use azalea_protocol::packets::login::s_login_acknowledged::ServerboundLoginAcknowledged;
use azalea_protocol::packets::login::{ClientboundLoginPacket, ServerboundLoginPacket};
use azalea_protocol::read::{ReadPacketError, deserialize_packet};
use crossbeam_channel::Sender;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

use super::NetworkEvent;
use super::conn::{Conn, MemoryEnd, RawWriter};
use super::handler::{handle_game_packet, handle_raw_game_packet};
use super::sender::{Outbound, PacketSender};

pub(crate) const SECURE_CHAT_NOTICE: &str =
    "Chat disabled: this server requires signed chat, which this client does not support yet.";

struct ChatSigningState {
    account_uuid: uuid::Uuid,
    session_id: uuid::Uuid,
    certificates: azalea_auth::certs::Certificates,
    message_index: u32,
    last_seen: LastSeenMessagesTracker,
}

const LAST_SEEN_CAPACITY: usize = 20;

struct LastSeenMessagesTracker {
    signatures: Vec<Option<azalea_crypto::signing::MessageSignature>>,
    tail: usize,
    offset: u32,
}

impl Default for LastSeenMessagesTracker {
    fn default() -> Self {
        Self {
            signatures: vec![None; LAST_SEEN_CAPACITY],
            tail: 0,
            offset: 0,
        }
    }
}

impl LastSeenMessagesTracker {
    fn add(&mut self, signature: azalea_crypto::signing::MessageSignature) {
        let previous_index = (self.tail + LAST_SEEN_CAPACITY - 1) % LAST_SEEN_CAPACITY;
        if self.signatures[previous_index].as_ref() == Some(&signature) {
            return;
        }
        self.signatures[self.tail] = Some(signature);
        self.tail = (self.tail + 1) % LAST_SEEN_CAPACITY;
        self.offset = self.offset.saturating_add(1);
    }

    fn snapshot_and_acknowledge(
        &mut self,
    ) -> (
        Vec<azalea_crypto::signing::MessageSignature>,
        azalea_protocol::packets::game::s_chat::LastSeenMessagesUpdate,
    ) {
        let mut acknowledged = FixedBitSet::<LAST_SEEN_CAPACITY>::new();
        let mut signatures = Vec::with_capacity(LAST_SEEN_CAPACITY);
        for position in 0..LAST_SEEN_CAPACITY {
            let index = (self.tail + position) % LAST_SEEN_CAPACITY;
            if let Some(signature) = &self.signatures[index] {
                acknowledged.set(position);
                signatures.push(signature.clone());
            }
        }
        let offset = std::mem::take(&mut self.offset);
        let checksum = last_seen_checksum(&signatures);
        (
            signatures,
            azalea_protocol::packets::game::s_chat::LastSeenMessagesUpdate {
                offset,
                acknowledged,
                checksum,
            },
        )
    }
}

fn last_seen_checksum(signatures: &[azalea_crypto::signing::MessageSignature]) -> u8 {
    if signatures.is_empty() {
        return 0;
    }
    let mut checksum = 1i32;
    for signature in signatures {
        let mut signature_hash = 1i32;
        for byte in signature.bytes {
            signature_hash = signature_hash
                .wrapping_mul(31)
                .wrapping_add(i32::from(byte));
        }
        checksum = checksum.wrapping_mul(31).wrapping_add(signature_hash);
    }
    let checksum = checksum as u8;
    if checksum == 0 { 1 } else { checksum }
}

impl ChatSigningState {
    fn from_certificates(
        account_uuid: uuid::Uuid,
        certificates: azalea_auth::certs::Certificates,
    ) -> Self {
        Self {
            account_uuid,
            session_id: uuid::Uuid::new_v4(),
            certificates,
            message_index: 0,
            last_seen: LastSeenMessagesTracker::default(),
        }
    }

    fn session_update(&self) -> ServerboundGamePacket {
        ServerboundGamePacket::ChatSessionUpdate(
            azalea_protocol::packets::game::s_chat_session_update::ServerboundChatSessionUpdate {
                chat_session:
                    azalea_protocol::packets::game::s_chat_session_update::RemoteChatSessionData {
                        session_id: self.session_id,
                        profile_public_key:
                            azalea_protocol::packets::game::s_chat_session_update::ProfilePublicKeyData {
                                expires_at: self.certificates.expires_at.timestamp_millis().max(0)
                                    as u64,
                                key: self.certificates.public_key_der.clone(),
                                key_signature: self.certificates.signature_v2.clone(),
                            },
                    },
            },
        )
    }

    fn sign_content(
        &mut self,
        content: &str,
        timestamp: std::time::SystemTime,
        salt: u64,
        last_seen: &[azalea_crypto::signing::MessageSignature],
    ) -> azalea_crypto::signing::MessageSignature {
        let signature = sign_chat_message(
            self.account_uuid,
            self.session_id,
            self.message_index,
            salt,
            timestamp,
            content,
            last_seen,
            &self.certificates.private_key,
        );
        self.message_index = self.message_index.saturating_add(1);
        signature
    }

    fn sign_message(
        &mut self,
        message: String,
        timestamp: std::time::SystemTime,
    ) -> ServerboundGamePacket {
        let salt = azalea_crypto::signing::make_salt();
        let (last_seen, last_seen_messages) = self.last_seen.snapshot_and_acknowledge();
        let signature = self.sign_content(&message, timestamp, salt, &last_seen);
        ServerboundGamePacket::Chat(azalea_protocol::packets::game::s_chat::ServerboundChat {
            message,
            timestamp: timestamp
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            salt,
            signature: Some(signature),
            last_seen_messages,
        })
    }

    fn sign_command(
        &mut self,
        command: String,
        arguments: Vec<(String, String)>,
        timestamp: std::time::SystemTime,
    ) -> ServerboundGamePacket {
        let salt = azalea_crypto::signing::make_salt();
        let (last_seen, last_seen_messages) = self.last_seen.snapshot_and_acknowledge();
        let argument_signatures = arguments
            .into_iter()
            .map(|(name, value)| {
                azalea_protocol::packets::game::s_chat_command_signed::ArgumentSignature {
                    name,
                    signature: self.sign_content(&value, timestamp, salt, &last_seen),
                }
            })
            .collect();
        ServerboundGamePacket::ChatCommandSigned(
            azalea_protocol::packets::game::s_chat_command_signed::ServerboundChatCommandSigned {
                command,
                timestamp: timestamp
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                salt,
                argument_signatures,
                last_seen_messages,
            },
        )
    }

    fn observe_inbound(&mut self, signature: azalea_crypto::signing::MessageSignature) {
        self.last_seen.add(signature);
    }
}

#[expect(clippy::too_many_arguments)]
fn sign_chat_message(
    account_uuid: uuid::Uuid,
    session_id: uuid::Uuid,
    message_index: u32,
    salt: u64,
    timestamp: std::time::SystemTime,
    content: &str,
    last_seen: &[azalea_crypto::signing::MessageSignature],
    private_key: &rsa::RsaPrivateKey,
) -> azalea_crypto::signing::MessageSignature {
    use rsa::signature::{RandomizedSigner, SignatureEncoding};

    let mut data_to_sign = Vec::new();
    1i32.azalea_write(&mut data_to_sign).unwrap();
    account_uuid.azalea_write(&mut data_to_sign).unwrap();
    session_id.azalea_write(&mut data_to_sign).unwrap();
    message_index.azalea_write(&mut data_to_sign).unwrap();
    salt.azalea_write(&mut data_to_sign).unwrap();
    timestamp
        .duration_since(std::time::UNIX_EPOCH)
        .expect("timestamp must be after epoch")
        .as_secs()
        .azalea_write(&mut data_to_sign)
        .unwrap();
    let message_len: u32 = content.len().try_into().expect("chat message fits u32");
    message_len.azalea_write(&mut data_to_sign).unwrap();
    data_to_sign.extend_from_slice(content.as_bytes());
    let last_seen_len: i32 = last_seen
        .len()
        .try_into()
        .expect("last-seen window fits i32");
    last_seen_len.azalea_write(&mut data_to_sign).unwrap();
    for signature in last_seen {
        data_to_sign.extend_from_slice(&signature.bytes);
    }

    let signing_key = rsa::pkcs1v15::SigningKey::<sha2::Sha256>::new(private_key.clone());
    let mut rng = rand::rng();
    let signature = signing_key
        .sign_with_rng(&mut rng, &data_to_sign)
        .to_bytes();
    azalea_crypto::signing::MessageSignature {
        bytes: signature
            .as_ref()
            .try_into()
            .expect("signature must be 256 bytes"),
    }
}

#[derive(Error, Debug)]
pub enum ConnectionError {
    #[error("invalid server address: {0}")]
    InvalidAddress(String),

    #[error("connection failed: {0}")]
    Connect(std::io::Error),

    #[error("packet read error: {0}")]
    Read(#[from] Box<ReadPacketError>),

    #[error("packet write error: {0}")]
    Write(#[from] std::io::Error),

    #[error("authentication failed: {0}")]
    Auth(String),

    #[error("disconnected by server: {0}")]
    Disconnected(String),

    #[error("encryption failed: {0}")]
    Encryption(String),

    #[error("joining {0} servers is not supported yet")]
    Unjoinable(String),
}

enum WriterRequest {
    Frame(Vec<u8>),
    Stop(oneshot::Sender<RawWriter>),
}

fn spawn_writer(
    mut writer: RawWriter,
) -> (
    mpsc::UnboundedSender<WriterRequest>,
    mpsc::UnboundedReceiver<ConnectionError>,
) {
    let (request_tx, mut request_rx) = mpsc::unbounded_channel();
    let (error_tx, error_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(request) = request_rx.recv().await {
            match request {
                WriterRequest::Frame(frame) => {
                    if let Err(error) = writer.write(&frame).await {
                        let _ = error_tx.send(ConnectionError::Write(error));
                        return;
                    }
                }
                WriterRequest::Stop(return_writer) => {
                    let _ = return_writer.send(writer);
                    return;
                }
            }
        }
    });
    (request_tx, error_rx)
}

async fn stop_writer(
    writer_tx: &mpsc::UnboundedSender<WriterRequest>,
    writer_error_rx: &mut mpsc::UnboundedReceiver<ConnectionError>,
) -> Result<RawWriter, ConnectionError> {
    let (return_tx, return_rx) = oneshot::channel();
    writer_tx
        .send(WriterRequest::Stop(return_tx))
        .map_err(|_| ConnectionError::Write(std::io::Error::other("writer task stopped")))?;
    tokio::select! {
        writer = return_rx => writer.map_err(|_| ConnectionError::Write(std::io::Error::other("writer task stopped"))),
        Some(error) = writer_error_rx.recv() => Err(error),
    }
}

impl From<super::resolve::ConnectError> for ConnectionError {
    fn from(e: super::resolve::ConnectError) -> Self {
        use super::resolve::ConnectError;
        match e {
            ConnectError::Resolve(e) => Self::InvalidAddress(e.to_string()),
            ConnectError::Io(e) => Self::Connect(e),
        }
    }
}

pub enum Transport {
    Remote {
        server: String,
        /// The server's protocol from an earlier server-list ping, when joining
        /// from the list; saves `negotiate_wire_version` its status probe.
        protocol: Option<i32>,
    },
    /// An integrated server in this process, reached over an in-memory pipe.
    #[allow(
        dead_code,
        reason = "the integrated server constructs these once it lands"
    )]
    Memory(MemoryEnd),
}

pub struct ConnectArgs {
    pub transport: Transport,
    pub username: String,
    pub uuid: uuid::Uuid,
    pub access_token: Option<String>,
    pub view_distance: u8,
}

pub struct ConnectionHandle {
    pub event_rx: crossbeam_channel::Receiver<NetworkEvent>,
    pub chat_tx: crossbeam_channel::Sender<String>,
    pub packet_tx: PacketSender,
    pub task: tokio::task::JoinHandle<()>,
}

impl Drop for ConnectionHandle {
    fn drop(&mut self) {
        self.task.abort();
        // The session is over: restore the launched version's wire protocol
        // and block table so nothing stale leaks into the next one.
        crate::version::clear_session_protocol();
        crate::world::block::set_active_protocol(crate::version::selected_protocol());
    }
}

pub fn spawn_connection(rt: &tokio::runtime::Runtime, args: ConnectArgs) -> ConnectionHandle {
    let (event_tx, event_rx) = crossbeam_channel::bounded(4096);
    let (chat_tx, chat_rx) = crossbeam_channel::bounded::<String>(64);
    let (packet_tx, packet_rx) = mpsc::unbounded_channel::<Outbound>();
    let game_packet_tx = packet_tx.clone();
    let secure_chat_required = Arc::new(AtomicBool::new(false));
    let secure_chat_required_task = Arc::clone(&secure_chat_required);
    let packet_tx = PacketSender::new(packet_tx);
    let task = rt.spawn(async move {
        if let Err(e) = connect_to_server(
            args,
            event_tx.clone(),
            chat_rx,
            game_packet_tx,
            packet_rx,
            secure_chat_required_task,
        )
        .await
        {
            tracing::error!("Network error: {e}");
            let reason = friendly_error_reason(&e);
            let _ = event_tx.try_send(NetworkEvent::Disconnected { reason });
        }
    });
    ConnectionHandle {
        event_rx,
        chat_tx,
        packet_tx,
        task,
    }
}

pub async fn connect_to_server(
    args: ConnectArgs,
    event_tx: Sender<NetworkEvent>,
    chat_rx: crossbeam_channel::Receiver<String>,
    game_packet_tx: mpsc::UnboundedSender<Outbound>,
    mut game_packet_rx: mpsc::UnboundedReceiver<Outbound>,
    secure_chat_required: Arc<AtomicBool>,
) -> Result<(), ConnectionError> {
    let ConnectArgs {
        transport,
        username,
        uuid,
        access_token,
        view_distance,
    } = args;

    let mut conn = match transport {
        Transport::Remote { server, protocol } => {
            let server_addr: ServerAddr = server
                .as_str()
                .try_into()
                .map_err(|_| ConnectionError::InvalidAddress(server.clone()))?;
            negotiate_wire_version(&server_addr, protocol).await?;
            super::resolve::connect(&server_addr, ClientIntention::Login).await?
        }
        Transport::Memory(end) => {
            // An integrated server speaks the latest protocol, so there is
            // nothing to probe and translation stays inert for the session.
            adopt_wire_protocol(pomme_protocol::version::LATEST.protocol);
            let mut conn = Conn::from_memory(end);
            super::resolve::send_intention(&mut conn, "localhost", 0, ClientIntention::Login)
                .await?;
            conn
        }
    };

    let hello = ServerboundLoginPacket::Hello(ServerboundHello {
        name: username.clone(),
        profile_id: uuid,
    });
    let frame = serialize_frame(&hello)?;
    let frame = match super::translate::active() {
        Some(t) => t.translate_outbound_login_frame(frame),
        None => frame,
    };
    conn.writer.write(&frame).await?;

    tracing::info!("Sent login hello as {username} ({uuid})");
    if access_token.is_none() {
        tracing::warn!(
            "Connecting offline (no access token). The server keys op/permissions to the \
             authenticated account, so op-only commands like /time may return \"Unknown command\" \
             under this offline identity."
        );
    }

    login_sequence(&mut conn, &uuid, access_token.as_deref()).await?;

    let chat_signing = if super::translate::active().is_none() {
        if let Some(access_token) = access_token.as_deref() {
            match azalea_auth::certs::fetch_certificates(access_token).await {
                Ok(certificates) => {
                    tracing::info!("Fetched Mojang chat-signing certificates");
                    Some(Arc::new(parking_lot::Mutex::new(
                        ChatSigningState::from_certificates(uuid, certificates),
                    )))
                }
                Err(error) => {
                    tracing::warn!("Could not fetch chat-signing certificates: {error}");
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    // 1.20.1 and older have no configuration phase: the server enters play as
    // soon as it has sent the profile, and the registries ride in the game
    // login packet rather than in registry_data packets.
    let no_config = super::translate::active().is_some_and(|t| t.no_config_phase());
    if !no_config {
        conn.write_packet(ServerboundLoginAcknowledged {}).await?;
    }

    let joined = if no_config {
        tracing::info!("Skipping configuration phase");
        read_inline_registries(&mut conn).await?
    } else {
        tracing::info!("Entering configuration phase");
        Joined {
            registries: config_sequence(
                &mut conn,
                view_distance,
                &event_tx,
                &mut game_packet_rx,
                None,
            )
            .await?,
            deferred_login: None,
        }
    };

    tracing::info!("Entering game state");
    let biome_colors = extract_biome_climate(&joined.registries);
    let _ = event_tx.try_send(NetworkEvent::BiomeColors {
        colors: biome_colors,
    });
    let _ = event_tx.try_send(NetworkEvent::Connected);

    game_loop(
        conn,
        &event_tx,
        chat_rx,
        game_packet_tx,
        game_packet_rx,
        joined,
        view_distance,
        secure_chat_required,
        chat_signing,
    )
    .await
}

/// What the phases before the game loop produced.
struct Joined {
    registries: std::sync::Arc<azalea_core::registry_holder::RegistryHolder>,
    /// The game `login` frame, when it was already read off the wire: a server
    /// with no configuration phase sends it before the game loop starts, which
    /// then replays it as its first packet.
    deferred_login: Option<Box<[u8]>>,
}

/// Reads the registries a pre-configuration-phase server ships inside its game
/// `login` packet, returning them with the untranslated login frame for the
/// game loop to replay. That frame is the first the server sends after the
/// profile (`PlayerList.placeNewPlayer`), with no acknowledgement in between.
async fn read_inline_registries(conn: &mut Conn) -> Result<Joined, ConnectionError> {
    use azalea_core::registry_holder::RegistryHolder;

    let login = conn.reader.read().await?;
    let translation = super::translate::active().expect("translation for a config-less version");
    let Some(frames) = translation.split_login_registries(&login) else {
        // A server that turns the join away here does it with a play-phase
        // disconnect, the login phase having already ended.
        if let Some(raw) = translation.translate_game_frame(login)
            && let Ok(ClientboundGamePacket::Disconnect(p)) =
                deserialize_packet::<ClientboundGamePacket>(&mut std::io::Cursor::new(&raw))
        {
            return Err(ConnectionError::Disconnected(format!("{}", p.reason)));
        }
        return Err(ConnectionError::Disconnected(
            "could not read the registries from the login packet".into(),
        ));
    };

    let mut registry_holder = RegistryHolder::default();
    for frame in frames {
        match deserialize_packet::<ClientboundConfigPacket>(&mut std::io::Cursor::new(&frame)) {
            Ok(ClientboundConfigPacket::RegistryData(p)) => {
                registry_holder.append(p.registry_id, p.entries);
            }
            Ok(_) => {}
            Err(e) => skip_malformed_packet(e)?,
        }
    }
    Ok(Joined {
        registries: std::sync::Arc::new(registry_holder),
        deferred_login: Some(login),
    })
}

/// Adopts the server's protocol as the wire version when translation data
/// for it exists, so one client joins any supported server version;
/// otherwise the launched version is kept (and the server shows its own
/// mismatch message, as before). A launched version without translation
/// data (listed for pings, not yet joinable) can never complete a join —
/// the handshake either gets the server's mismatch rejection or succeeds
/// and breaks mid-connect on untranslated packets — so it is refused up
/// front on every path, a failed probe or a stale server-list `known`
/// protocol included. The protocol comes from `known` (a server-list ping)
/// or a status probe. Sets the session protocol and the matching
/// block-state table, so it must run before the login handshake and before
/// any world state loads.
async fn negotiate_wire_version(
    server_addr: &ServerAddr,
    known: Option<i32>,
) -> Result<(), ConnectionError> {
    let selected = crate::version::selected_protocol();
    let probed = match known {
        Some(p) => Some(p),
        None => {
            let probe = async {
                let (status, _) = super::resolve::request_status(server_addr).await.ok()?;
                Some(status.version.protocol)
            };
            tokio::time::timeout(std::time::Duration::from_secs(5), probe)
                .await
                .ok()
                .flatten()
        }
    };
    let wire = resolve_wire(probed, selected).map_err(|p| {
        let name = pomme_protocol::ProtocolVersion::from_protocol(p)
            .map(|v| v.name.to_string())
            .unwrap_or_else(|| format!("protocol {p}"));
        ConnectionError::Unjoinable(name)
    })?;
    tracing::info!("Negotiated wire protocol {wire}");
    adopt_wire_protocol(wire);
    Ok(())
}

/// Speaks `wire` for the rest of the session. The translation layer and the
/// block-state tables both key off it, so they always move together.
fn adopt_wire_protocol(wire: i32) {
    crate::version::set_session_protocol(wire);
    crate::world::block::set_active_protocol(wire);
}

/// The wire protocol to speak given the probed server protocol and the
/// launched (`selected`) one; `Err` carries an unjoinable outcome (see
/// [`negotiate_wire_version`]). Inert while `selected` is joinable — every
/// arm then yields a joinable wire.
fn resolve_wire(probed: Option<i32>, selected: i32) -> Result<i32, i32> {
    let wire = match probed {
        Some(p) if super::translate::joinable(p) => p,
        Some(p) => {
            tracing::warn!("Server speaks unsupported protocol {p}; falling back to {selected}");
            selected
        }
        None => {
            tracing::warn!("Server protocol probe failed; falling back to {selected}");
            selected
        }
    };
    if super::translate::joinable(wire) {
        Ok(wire)
    } else {
        Err(wire)
    }
}

async fn login_sequence(
    conn: &mut Conn,
    uuid: &uuid::Uuid,
    access_token: Option<&str>,
) -> Result<(), ConnectionError> {
    loop {
        // Read the raw frame ourselves so older-version layouts can be
        // rewritten before the typed decode (26.1's login_finished lacks the
        // trailing session id).
        let raw = conn.reader.read().await?;
        let raw = match super::translate::active() {
            Some(t) => t.translate_login_frame(raw),
            None => raw,
        };
        let packet: ClientboundLoginPacket = deserialize_packet(&mut std::io::Cursor::new(&raw))?;
        tracing::info!("Login packet: {:?}", std::mem::discriminant(&packet));
        match packet {
            ClientboundLoginPacket::Hello(p) => {
                handle_encryption(conn, &p, uuid, access_token).await?;
            }
            ClientboundLoginPacket::LoginCompression(p) => {
                conn.set_compression_threshold(p.compression_threshold);
                tracing::info!(
                    "Compression enabled (threshold: {})",
                    p.compression_threshold
                );
            }
            ClientboundLoginPacket::LoginFinished(p) => {
                tracing::info!(
                    "Login success: {} ({})",
                    p.game_profile.name,
                    p.game_profile.uuid
                );
                return Ok(());
            }
            ClientboundLoginPacket::LoginDisconnect(p) => {
                return Err(ConnectionError::Disconnected(format!("{}", p.reason)));
            }
            ClientboundLoginPacket::CookieRequest(p) => {
                conn.write_packet(
                    azalea_protocol::packets::login::s_cookie_response::ServerboundCookieResponse {
                        key: p.key,
                        payload: None,
                    },
                )
                .await?;
            }
            _ => {
                tracing::debug!("Login packet: {:?}", std::mem::discriminant(&packet));
            }
        }
    }
}

async fn handle_encryption(
    conn: &mut Conn,
    hello: &ClientboundHello,
    uuid: &uuid::Uuid,
    access_token: Option<&str>,
) -> Result<(), ConnectionError> {
    let e = azalea_crypto::encrypt(&hello.public_key, &hello.challenge)
        .map_err(ConnectionError::Encryption)?;

    if hello.should_authenticate {
        let access_token = access_token.ok_or_else(|| {
            ConnectionError::Auth(
                "server requires authentication but no access token provided".into(),
            )
        })?;

        tracing::info!("Authenticating with session server (uuid: {uuid})");
        azalea_auth::sessionserver::join(azalea_auth::sessionserver::SessionServerJoinOpts {
            access_token,
            public_key: &hello.public_key,
            private_key: &e.secret_key,
            uuid,
            server_id: &hello.server_id,
            proxy: None,
        })
        .await
        .map_err(|e| ConnectionError::Auth(e.to_string()))?;
        tracing::info!("Session server authentication successful");
    } else {
        tracing::info!("Server does not require authentication");
    }

    conn.write_packet(ServerboundKey {
        key_bytes: e.encrypted_public_key,
        encrypted_challenge: e.encrypted_challenge,
    })
    .await?;

    conn.set_encryption_key(e.secret_key);
    tracing::info!("Encryption enabled");
    Ok(())
}

async fn config_sequence(
    conn: &mut Conn,
    view_distance: u8,
    event_tx: &Sender<NetworkEvent>,
    outbound_rx: &mut mpsc::UnboundedReceiver<Outbound>,
    // `Some` on a mid-session reconfiguration: the previous registries,
    // kept when the server re-sends nothing (vanilla's RegistryDataCollector
    // returns the original registries unchanged in that case).
    previous_registries: Option<&std::sync::Arc<azalea_core::registry_holder::RegistryHolder>>,
) -> Result<std::sync::Arc<azalea_core::registry_holder::RegistryHolder>, ConnectionError> {
    use azalea_core::registry_holder::RegistryHolder;
    use azalea_protocol::packets::config::*;

    let mut registry_holder = RegistryHolder::default();
    let mut received_registry_data = false;

    // Vanilla sends brand and client information once, from the login
    // listener; a reconfiguration sends neither.
    if previous_registries.is_none() {
        // Some servers key off the brand.
        write_config_packet(
            conn,
            ServerboundConfigPacket::CustomPayload(s_custom_payload::ServerboundCustomPayload {
                identifier: "minecraft:brand".into(),
                data: super::brand_payload().into(),
            }),
        )
        .await?;

        write_config_packet(
            conn,
            ServerboundConfigPacket::ClientInformation(
                s_client_information::ServerboundClientInformation {
                    information: super::client_information(view_distance),
                },
            ),
        )
        .await?;
    }

    // Config frames are read raw so older wire versions translate (765's
    // registry_data fans out into several frames, hence the queue).
    let mut pending = std::collections::VecDeque::new();
    loop {
        let packet = if let Some(packet) = pending.pop_front() {
            packet
        } else {
            tokio::select! {
                raw = conn.reader.read() => {
                    let raw = match raw {
                        Ok(raw) => raw,
                        Err(e) => {
                            skip_malformed_packet(e)?;
                            continue;
                        }
                    };
                    let frames = match super::translate::active() {
                        Some(t) => t.translate_config_frame(raw),
                        None => vec![raw],
                    };
                    for frame in frames {
                        match deserialize_packet::<ClientboundConfigPacket>(
                            &mut std::io::Cursor::new(&frame),
                        ) {
                            Ok(packet) => pending.push_back(packet),
                            Err(e) => skip_malformed_packet(e)?,
                        }
                    }
                    continue;
                }
                // `Some(..)` disables the branch when the channel closes instead
                // of busy-looping on a closed receiver.
                Some(outbound) = outbound_rx.recv() => {
                    if let Outbound::Packet(packet) = outbound
                        && let ServerboundGamePacket::ResourcePack(p) = *packet
                    {
                        use azalea_protocol::packets::config::s_resource_pack as config_pack;
                        use azalea_protocol::packets::game::s_resource_pack as game_pack;
                        let action = match p.action {
                            game_pack::Action::SuccessfullyLoaded => config_pack::Action::SuccessfullyLoaded,
                            game_pack::Action::Declined => config_pack::Action::Declined,
                            game_pack::Action::FailedDownload => config_pack::Action::FailedDownload,
                            game_pack::Action::Accepted => config_pack::Action::Accepted,
                            game_pack::Action::InvalidUrl => config_pack::Action::InvalidUrl,
                            game_pack::Action::FailedReload => config_pack::Action::FailedReload,
                            game_pack::Action::Discarded => config_pack::Action::Discarded,
                        };
                        write_config_packet(conn, ServerboundConfigPacket::ResourcePack(
                            config_pack::ServerboundResourcePack { id: p.id, action },
                        )).await?;
                    }
                    // Anything else is discarded: vanilla defers its outbound
                    // queue, but pomme's game keeps ticking through a
                    // reconfiguration, so stale movement/actions are best dropped.
                    continue;
                }
            }
        };
        match packet {
            ClientboundConfigPacket::RegistryData(p) => {
                received_registry_data = true;
                registry_holder.append(p.registry_id, p.entries);
            }
            ClientboundConfigPacket::UpdateTags(_) => {
                tracing::debug!("Received tags");
            }
            ClientboundConfigPacket::SelectKnownPacks(_) => {
                // Claiming no known packs forces the server to send NBT for
                // every registry entry; `variant_index` (handler.rs) relies on
                // that to equate registry-map position with protocol id.
                write_config_packet(
                    conn,
                    ServerboundConfigPacket::SelectKnownPacks(
                        s_select_known_packs::ServerboundSelectKnownPacks {
                            known_packs: vec![],
                        },
                    ),
                )
                .await?;
            }
            ClientboundConfigPacket::KeepAlive(p) => {
                write_config_packet(
                    conn,
                    ServerboundConfigPacket::KeepAlive(s_keep_alive::ServerboundKeepAlive {
                        id: p.id,
                    }),
                )
                .await?;
            }
            ClientboundConfigPacket::FinishConfiguration(_) => {
                write_config_packet(
                    conn,
                    ServerboundConfigPacket::FinishConfiguration(
                        s_finish_configuration::ServerboundFinishConfiguration {},
                    ),
                )
                .await?;
                return Ok(match previous_registries {
                    Some(previous) if !received_registry_data => previous.clone(),
                    _ => std::sync::Arc::new(registry_holder),
                });
            }
            ClientboundConfigPacket::Disconnect(p) => {
                return Err(ConnectionError::Disconnected(format!("{}", p.reason)));
            }
            ClientboundConfigPacket::CookieRequest(p) => {
                write_config_packet(
                    conn,
                    ServerboundConfigPacket::CookieResponse(
                        s_cookie_response::ServerboundCookieResponse {
                            key: p.key,
                            payload: None,
                        },
                    ),
                )
                .await?;
            }
            ClientboundConfigPacket::ResourcePackPush(p) => {
                tracing::info!(
                    "Server pushing resource pack {} (required: {})",
                    p.id,
                    p.required
                );
                let _ = event_tx.try_send(NetworkEvent::ResourcePackPush {
                    id: p.id,
                    url: p.url.clone(),
                    hash: p.hash.clone(),
                    required: p.required,
                });
                write_config_packet(
                    conn,
                    ServerboundConfigPacket::ResourcePack(
                        s_resource_pack::ServerboundResourcePack {
                            id: p.id,
                            action: s_resource_pack::Action::Accepted,
                        },
                    ),
                )
                .await?;
            }
            ClientboundConfigPacket::ResourcePackPop(p) => {
                tracing::info!("Server popping resource pack {:?}", p.id);
                let _ = event_tx.try_send(NetworkEvent::ResourcePackPop { id: p.id });
            }
            _ => {
                tracing::debug!("Config packet: {:?}", std::mem::discriminant(&packet));
            }
        }
    }
}

fn extract_biome_climate(
    holder: &azalea_core::registry_holder::RegistryHolder,
) -> std::collections::HashMap<u32, crate::renderer::chunk::mesher::BiomeClimate> {
    use crate::renderer::chunk::mesher::{BiomeClimate, GrassColorModifier, int_to_rgb};

    let mut result = std::collections::HashMap::new();
    let biome_key: azalea_registry::identifier::Identifier = "minecraft:worldgen/biome".into();
    if let Some(registry) = holder.extra.get(&biome_key) {
        for (id, (_, nbt)) in registry.map.iter().enumerate() {
            let temp = nbt_float(nbt, "temperature").unwrap_or(0.8);
            let downfall = nbt_float(nbt, "downfall").unwrap_or(0.4);

            let effects = nbt.get("effects").and_then(|v| match v {
                simdnbt::owned::NbtTag::Compound(c) => Some(c),
                _ => None,
            });

            let grass_color_override = effects
                .and_then(|e| nbt_color_from_compound(e, "grass_color"))
                .map(int_to_rgb);

            let foliage_color_override = effects
                .and_then(|e| nbt_color_from_compound(e, "foliage_color"))
                .map(int_to_rgb);

            let dry_foliage_color_override = effects
                .and_then(|e| nbt_color_from_compound(e, "dry_foliage_color"))
                .map(int_to_rgb);

            let grass_color_modifier = effects
                .and_then(|e| nbt_string_from_compound(e, "grass_color_modifier"))
                .map(|s| match s.as_str() {
                    "dark_forest" => GrassColorModifier::DarkForest,
                    "swamp" => GrassColorModifier::Swamp,
                    _ => GrassColorModifier::None,
                })
                .unwrap_or(GrassColorModifier::None);

            result.insert(
                id as u32,
                BiomeClimate {
                    temperature: temp,
                    downfall,
                    grass_color_override,
                    grass_color_modifier,
                    foliage_color_override,
                    dry_foliage_color_override,
                },
            );
        }
    }
    tracing::info!("Extracted {} biome climate entries", result.len());
    result
}

fn nbt_float(nbt: &simdnbt::owned::NbtCompound, key: &str) -> Option<f32> {
    nbt.get(key).and_then(|v| match v {
        simdnbt::owned::NbtTag::Float(f) => Some(*f),
        simdnbt::owned::NbtTag::Double(d) => Some(*d as f32),
        _ => None,
    })
}

fn nbt_color_from_compound(compound: &simdnbt::owned::NbtCompound, key: &str) -> Option<i32> {
    compound.get(key).and_then(|v| match v {
        simdnbt::owned::NbtTag::Int(i) => Some(*i),
        simdnbt::owned::NbtTag::Long(l) => Some(*l as i32),
        simdnbt::owned::NbtTag::String(s) => {
            let s = s.to_string();
            let hex = s.strip_prefix('#').unwrap_or(&s);
            i32::from_str_radix(hex, 16).ok()
        }
        _ => None,
    })
}

fn nbt_string_from_compound(compound: &simdnbt::owned::NbtCompound, key: &str) -> Option<String> {
    compound.get(key).and_then(|v| match v {
        simdnbt::owned::NbtTag::String(s) => Some(s.to_string()),
        _ => None,
    })
}

#[allow(clippy::too_many_arguments)]
async fn game_loop(
    mut conn: Conn,
    event_tx: &Sender<NetworkEvent>,
    chat_rx: crossbeam_channel::Receiver<String>,
    outbound_tx: mpsc::UnboundedSender<Outbound>,
    mut outbound_rx: mpsc::UnboundedReceiver<Outbound>,
    joined: Joined,
    view_distance: u8,
    secure_chat_required: Arc<AtomicBool>,
    chat_signing: Option<Arc<parking_lot::Mutex<ChatSigningState>>>,
) -> Result<(), ConnectionError> {
    let Joined {
        registries: mut registry_holder,
        mut deferred_login,
    } = joined;
    let sender = PacketSender::new(outbound_tx.clone());

    let shared_tree: crate::net::commands::SharedCommandTree =
        std::sync::Arc::new(parking_lot::Mutex::new(None));

    let chat_outbound_tx = outbound_tx;
    let chat_tree = shared_tree.clone();
    let chat_secure_required = Arc::clone(&secure_chat_required);
    let chat_event_tx = event_tx.clone();
    let chat_signing_state = chat_signing.clone();
    tokio::task::spawn_blocking(move || {
        while let Ok(msg) = chat_rx.recv() {
            let packet = if let Some(command) = msg.strip_prefix('/') {
                tracing::info!("Sending command: {command:?}");
                let signable_arguments = chat_tree
                    .lock()
                    .as_ref()
                    .map(|tree| tree.signable_arguments(command))
                    .unwrap_or_default();
                if !signable_arguments.is_empty() {
                    if let Some(state) = &chat_signing_state {
                        let timestamp = std::time::SystemTime::now();
                        let mut state = state.lock();
                        state.sign_command(command.to_string(), signable_arguments, timestamp)
                    } else {
                        tracing::warn!(
                            "Command has signable arguments but chat signing is unavailable"
                        );
                        if chat_secure_required.load(Ordering::Acquire) {
                            let _ = chat_event_tx.try_send(NetworkEvent::ChatMessage {
                                spans: vec![crate::ui::text::TextSpan::new(
                                    "Chat disabled: this server requires signed commands, which this client does not support yet.".into(),
                                    [1.0, 0.35, 0.35, 1.0],
                                )],
                            });
                            continue;
                        }
                        ServerboundGamePacket::ChatCommand(
                            azalea_protocol::packets::game::s_chat_command::ServerboundChatCommand {
                                command: command.to_string(),
                            },
                        )
                    }
                } else {
                    ServerboundGamePacket::ChatCommand(
                        azalea_protocol::packets::game::s_chat_command::ServerboundChatCommand {
                            command: command.to_string(),
                        },
                    )
                }
            } else {
                if chat_secure_required.load(Ordering::Acquire) && chat_signing_state.is_none() {
                    let _ = chat_event_tx.try_send(NetworkEvent::ChatMessage {
                        spans: vec![crate::ui::text::TextSpan::new(
                            SECURE_CHAT_NOTICE.into(),
                            [1.0, 0.35, 0.35, 1.0],
                        )],
                    });
                    continue;
                }
                let timestamp = std::time::SystemTime::now();
                if let Some(state) = &chat_signing_state {
                    let mut state = state.lock();
                    state.sign_message(msg, timestamp)
                } else {
                    ServerboundGamePacket::Chat(
                        azalea_protocol::packets::game::s_chat::ServerboundChat {
                            message: msg,
                            timestamp: timestamp
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64,
                            salt: 0,
                            signature: None,
                            last_seen_messages: Default::default(),
                        },
                    )
                }
            };
            if chat_outbound_tx
                .send(Outbound::Packet(Box::new(packet)))
                .is_err()
            {
                break;
            }
        }
    });

    // Share the registries with the game loop for hashing predicted container
    // clicks.
    let _ = event_tx.try_send(NetworkEvent::Registries(registry_holder.clone()));

    let translation = super::translate::active();
    if deferred_login.is_some() {
        // 1.20.1 sends both from its login handler, where later versions send
        // them in the configuration phase (ClientPacketListener.handleLogin).
        let info = ServerboundGamePacket::ClientInformation(
            azalea_protocol::packets::game::s_client_information::ServerboundClientInformation {
                client_information: super::client_information(view_distance),
            },
        );
        write_game_frame(&mut conn.writer, translation, serialize_frame(&info)?).await?;

        let brand = ServerboundGamePacket::CustomPayload(
            azalea_protocol::packets::game::s_custom_payload::ServerboundCustomPayload {
                identifier: "minecraft:brand".into(),
                data: super::brand_payload().into(),
            },
        );
        write_game_frame(&mut conn.writer, translation, serialize_frame(&brand)?).await?;
    }

    let Conn { reader, writer } = conn;
    let mut reader = reader;
    let (mut writer_tx, mut writer_error_rx) = spawn_writer(writer);
    loop {
        let raw = if let Some(raw) = deferred_login.take() {
            Ok(raw)
        } else {
            tokio::select! {
                Some(out) = outbound_rx.recv() => {
                    let frame = match out {
                        Outbound::Packet(mut packet) => {
                            if let Some(t) = translation {
                                t.remap_outbound(&mut packet);
                            }
                            serialize_frame(&*packet)?
                        }
                        Outbound::Raw(bytes) => bytes,
                    };
                    let frames = match translation {
                        Some(t) if t.translates_outbound() => t.translate_outbound_game_frame(frame),
                        _ => vec![frame],
                    };
                    for frame in frames {
                        writer_tx
                            .send(WriterRequest::Frame(frame))
                            .map_err(|_| ConnectionError::Write(std::io::Error::other("writer task stopped")))?;
                    }
                    continue;
                }
                Some(error) = writer_error_rx.recv() => return Err(error),
                raw = reader.read() => raw,
            }
        };
        let raw = match raw {
            Ok(raw) => raw,
            Err(e) => {
                skip_malformed_packet(e)?;
                continue;
            }
        };
        let raw = match translation {
            Some(t) => match t.translate_game_frame(raw) {
                Some(raw) => raw,
                None => continue,
            },
            None => raw,
        };
        if handle_raw_game_packet(&raw, event_tx) {
            continue;
        }
        match deserialize_packet::<ClientboundGamePacket>(&mut std::io::Cursor::new(&raw)) {
            Ok(mut packet) => {
                if matches!(packet, ClientboundGamePacket::StartConfiguration(_)) {
                    // Vanilla clears the client level before acknowledging
                    // (ClientPacketListener.handleConfigurationStart); chat
                    // survives the transition.
                    let _ = event_tx.try_send(NetworkEvent::Reconfiguring);
                    let writer = stop_writer(&writer_tx, &mut writer_error_rx).await?;
                    let mut config_conn = Conn { reader, writer };
                    let ack = ServerboundGamePacket::ConfigurationAcknowledged(
                        azalea_protocol::packets::game::s_configuration_acknowledged::ServerboundConfigurationAcknowledged,
                    );
                    write_game_frame(&mut config_conn.writer, translation, serialize_frame(&ack)?)
                        .await?;
                    let holder = config_sequence(
                        &mut config_conn,
                        view_distance,
                        event_tx,
                        &mut outbound_rx,
                        Some(&registry_holder),
                    )
                    .await?;
                    if !std::sync::Arc::ptr_eq(&holder, &registry_holder) {
                        registry_holder = holder;
                        let _ =
                            event_tx.try_send(NetworkEvent::Registries(registry_holder.clone()));
                        let _ = event_tx.try_send(NetworkEvent::BiomeColors {
                            colors: extract_biome_climate(&registry_holder),
                        });
                    }
                    let Conn {
                        reader: next_reader,
                        writer,
                    } = config_conn;
                    reader = next_reader;
                    (writer_tx, writer_error_rx) = spawn_writer(writer);
                    continue;
                }
                if let Some(t) = translation
                    && !t.remap_inbound(&mut packet)
                {
                    continue;
                }
                if let ClientboundGamePacket::Login(login) = &packet {
                    secure_chat_required.store(login.enforces_secure_chat, Ordering::Release);
                    if login.enforces_secure_chat {
                        tracing::warn!(
                            "Server requires secure chat; unsigned chat and signable commands will be blocked"
                        );
                    }
                    if let Some(state) = &chat_signing {
                        let session_update = state.lock().session_update();
                        let frame = serialize_frame(&session_update)?;
                        writer_tx.send(WriterRequest::Frame(frame)).map_err(|_| {
                            ConnectionError::Write(std::io::Error::other("writer task stopped"))
                        })?;
                    }
                }
                if let ClientboundGamePacket::PlayerChat(chat) = &packet
                    && let Some(signature) = &chat.signature
                    && let Some(state) = &chat_signing
                {
                    state.lock().observe_inbound(signature.clone());
                }
                handle_game_packet(&packet, &sender, event_tx, &registry_holder, &shared_tree)
            }
            Err(e) => skip_malformed_packet(e)?,
        }
    }
}

fn serialize_frame<P: azalea_protocol::packets::ProtocolPacket + std::fmt::Debug>(
    packet: &P,
) -> Result<Vec<u8>, ConnectionError> {
    azalea_protocol::write::serialize_packet(packet)
        .map(Vec::from)
        .map_err(|e| ConnectionError::Write(std::io::Error::other(e)))
}

/// Writes one latest-layout frame, translating it for older wire versions.
async fn write_game_frame(
    writer: &mut RawWriter,
    translation: Option<&super::translate::Translation>,
    frame: Vec<u8>,
) -> Result<(), ConnectionError> {
    let frames = match translation {
        Some(t) if t.translates_outbound() => t.translate_outbound_game_frame(frame),
        _ => vec![frame],
    };
    for frame in frames {
        writer.write(&frame).await?;
    }
    Ok(())
}

/// Writes one latest-layout configuration packet, translating it for older
/// wire versions (765 down: id remap plus suppression of packets the wire
/// version lacks).
async fn write_config_packet(
    conn: &mut Conn,
    packet: ServerboundConfigPacket,
) -> Result<(), ConnectionError> {
    let Some(t) = super::translate::active().filter(|t| t.translates_config()) else {
        return Ok(conn.write_packet(packet).await?);
    };
    if let Some(frame) = t.translate_outbound_config_frame(serialize_frame(&packet)?) {
        conn.writer.write(&frame).await?;
    }
    Ok(())
}

/// Recoverable decode errors skip the packet; anything else tears down the
/// connection.
fn skip_malformed_packet(err: Box<ReadPacketError>) -> Result<(), ConnectionError> {
    match &*err {
        ReadPacketError::Parse { .. }
        | ReadPacketError::UnknownPacketId { .. }
        | ReadPacketError::LeftoverData { .. } => {
            tracing::warn!("Skipping malformed packet: {err}");
            Ok(())
        }
        _ => Err(err.into()),
    }
}

fn friendly_error_reason(err: &ConnectionError) -> String {
    let msg = err.to_string();
    if msg.contains("connection refused") || msg.contains("Connection refused") {
        "Connection refused".to_string()
    } else if msg.contains("Connection closed")
        || msg.contains("connection reset")
        || msg.contains("broken pipe")
    {
        "Server closed".to_string()
    } else if msg.contains("timed out") || msg.contains("Timed out") {
        "Connection timed out".to_string()
    } else if msg.contains("no addresses found") || msg.contains("failed to lookup") {
        "Unknown host".to_string()
    } else {
        msg
    }
}

#[cfg(test)]
mod tests {
    use pomme_protocol::version::LATEST;

    use super::*;

    fn test_signature(byte: u8) -> azalea_crypto::signing::MessageSignature {
        azalea_crypto::signing::MessageSignature { bytes: [byte; 256] }
    }

    #[test]
    fn last_seen_window_acknowledges_recent_signed_messages() {
        let mut tracker = LastSeenMessagesTracker::default();
        tracker.add(test_signature(1));
        tracker.add(test_signature(2));
        tracker.add(test_signature(2));

        let (signatures, update) = tracker.snapshot_and_acknowledge();
        assert_eq!(signatures, vec![test_signature(1), test_signature(2)]);
        assert_eq!(update.offset, 2);
        assert!(update.acknowledged.index(18));
        assert!(update.acknowledged.index(19));
        assert!(!update.acknowledged.index(17));
        assert_ne!(update.checksum, 0);

        let (_, repeated) = tracker.snapshot_and_acknowledge();
        assert_eq!(repeated.offset, 0);
        assert_eq!(repeated.acknowledged, update.acknowledged);
        assert_eq!(repeated.checksum, update.checksum);
    }

    #[test]
    fn last_seen_window_keeps_only_the_twenty_most_recent_messages() {
        let mut tracker = LastSeenMessagesTracker::default();
        for byte in 0..21 {
            tracker.add(test_signature(byte));
        }

        let (signatures, update) = tracker.snapshot_and_acknowledge();
        assert_eq!(update.offset, 21);
        assert_eq!(signatures.len(), LAST_SEEN_CAPACITY);
        assert_eq!(signatures[0], test_signature(1));
        assert_eq!(signatures[19], test_signature(20));
        for index in 0..LAST_SEEN_CAPACITY {
            assert!(update.acknowledged.index(index));
        }
    }

    /// 762 (1.19.4) is not a supported version at all, so it never gains a
    /// wire translation; 775 has one.
    #[test]
    fn resolve_wire_gates_unjoinable_versions() {
        let latest = LATEST.protocol;
        assert_eq!(resolve_wire(Some(775), latest), Ok(775));
        assert_eq!(resolve_wire(Some(762), latest), Ok(latest));
        assert_eq!(resolve_wire(None, latest), Ok(latest));
        // An untranslated launched version is refused whatever the probe
        // yielded, unless the server itself speaks a joinable protocol.
        assert_eq!(resolve_wire(None, 762), Err(762));
        assert_eq!(resolve_wire(Some(762), 762), Err(762));
        assert_eq!(resolve_wire(Some(latest), 762), Ok(latest));
        // A staged version (tables embedded, not yet in TRANSLATED) is
        // refused when launched and adopted around when the server is
        // joinable.
        for v in pomme_protocol::version::VERSIONS {
            if pomme_protocol::PacketTable::for_protocol(v.protocol).is_some()
                && !crate::net::translate::joinable(v.protocol)
            {
                assert_eq!(
                    resolve_wire(Some(v.protocol), latest),
                    Ok(latest),
                    "{}",
                    v.name
                );
                assert_eq!(
                    resolve_wire(None, v.protocol),
                    Err(v.protocol),
                    "{}",
                    v.name
                );
            }
        }
    }

    /// The whole join over an in-memory pipe, against a peer that sends only
    /// the two frames the client actually requires: login finished, then
    /// finish configuration. No registry data, no compression, no encryption.
    #[tokio::test]
    async fn joins_an_integrated_server_over_the_pipe() {
        use azalea_auth::game_profile::GameProfile;
        use azalea_protocol::packets::config::c_finish_configuration::ClientboundFinishConfiguration;
        use azalea_protocol::packets::handshake::ServerboundHandshakePacket;
        use azalea_protocol::packets::login::c_login_finished::ClientboundLoginFinished;
        use uuid::Uuid;

        use crate::net::conn::memory_pipes;

        /// The next packet the client sent, in whichever phase the caller
        /// names.
        async fn sent<P: azalea_protocol::packets::ProtocolPacket + std::fmt::Debug>(
            peer: &mut Conn,
        ) -> P {
            peer.read_packet().await.unwrap()
        }

        let (client_end, server_end) = memory_pipes();
        let mut peer = Conn::from_memory(server_end);

        let (event_tx, event_rx) = crossbeam_channel::bounded(4096);
        let (_chat_tx, chat_rx) = crossbeam_channel::bounded(64);
        let (packet_tx, packet_rx) = mpsc::unbounded_channel();
        let secure_chat_required = Arc::new(AtomicBool::new(false));

        let client = tokio::spawn(connect_to_server(
            ConnectArgs {
                transport: Transport::Memory(client_end),
                username: "Steve".to_owned(),
                uuid: Uuid::nil(),
                access_token: None,
                view_distance: 8,
            },
            event_tx,
            chat_rx,
            packet_tx,
            packet_rx,
            secure_chat_required,
        ));

        assert!(matches!(
            sent(&mut peer).await,
            ServerboundHandshakePacket::Intention(p) if p.protocol_version == LATEST.protocol
        ));
        assert!(matches!(
            sent(&mut peer).await,
            ServerboundLoginPacket::Hello(p) if p.name == "Steve"
        ));

        peer.write_packet(ClientboundLoginFinished {
            game_profile: GameProfile::new(Uuid::nil(), "Steve".to_owned()),
            session_id: Uuid::nil(),
        })
        .await
        .unwrap();

        assert!(matches!(
            sent(&mut peer).await,
            ServerboundLoginPacket::LoginAcknowledged(_)
        ));
        assert!(matches!(
            sent(&mut peer).await,
            ServerboundConfigPacket::CustomPayload(p) if p.identifier.to_string() == "minecraft:brand"
        ));
        assert!(matches!(
            sent(&mut peer).await,
            ServerboundConfigPacket::ClientInformation(p) if p.information.view_distance == 8
        ));

        peer.write_packet(ClientboundFinishConfiguration)
            .await
            .unwrap();

        assert!(matches!(
            sent(&mut peer).await,
            ServerboundConfigPacket::FinishConfiguration(_)
        ));

        // Emitted as soon as configuration ends, before any game packet.
        let events: Vec<_> = std::iter::from_fn(|| event_rx.recv().ok())
            .take(2)
            .collect();
        assert!(matches!(events[0], NetworkEvent::BiomeColors { .. }));
        assert!(matches!(events[1], NetworkEvent::Connected));

        client.abort();
    }

    #[tokio::test]
    async fn blocked_writer_does_not_stop_peer_reads() {
        use tokio::time::{Duration, timeout};

        use crate::net::conn::memory_pipes;

        let (client_end, server_end) = memory_pipes();
        let mut client = Conn::from_memory(client_end);
        let mut server = Conn::from_memory(server_end);
        let (writer_tx, mut writer_error_rx) = spawn_writer(client.writer);

        // Fill the writer's bounded pipe without reading it from the server
        // side. The writer task will eventually await backpressure here.
        for _ in 0..128 {
            writer_tx
                .send(WriterRequest::Frame(vec![0; 64 * 1024]))
                .unwrap();
        }
        tokio::task::yield_now().await;

        // A game loop must still receive server traffic while that write is
        // blocked. A one-byte raw frame is enough to exercise the read half.
        server.writer.write(&[0]).await.unwrap();
        let read = timeout(Duration::from_millis(250), client.reader.read())
            .await
            .expect("reader was blocked by the writer")
            .unwrap();
        assert_eq!(&read[..], &[0]);

        // Close the peer's read half so the blocked writer exits cleanly.
        drop(server.reader);
        drop(writer_tx);
        let _ = timeout(Duration::from_millis(250), writer_error_rx.recv()).await;
    }

    #[tokio::test]
    async fn strict_login_blocks_unsigned_chat_at_the_network_boundary() {
        use std::time::Duration;

        use azalea_auth::game_profile::GameProfile;
        use azalea_core::entity_id::MinecraftEntityId;
        use azalea_core::game_type::{GameMode, OptionalGameType};
        use azalea_protocol::packets::config::ServerboundConfigPacket;
        use azalea_protocol::packets::config::c_finish_configuration::ClientboundFinishConfiguration;
        use azalea_protocol::packets::game::ClientboundGamePacket;
        use azalea_protocol::packets::game::c_login::ClientboundLogin;
        use azalea_protocol::packets::handshake::ServerboundHandshakePacket;
        use azalea_protocol::packets::login::{ClientboundLoginFinished, ServerboundLoginPacket};
        use azalea_registry::DataRegistry;
        use uuid::Uuid;

        use crate::net::conn::memory_pipes;

        async fn sent<P: azalea_protocol::packets::ProtocolPacket + std::fmt::Debug>(
            peer: &mut Conn,
        ) -> P {
            peer.read_packet().await.unwrap()
        }

        async fn received(event_rx: &crossbeam_channel::Receiver<NetworkEvent>) -> NetworkEvent {
            let event_rx = event_rx.clone();
            tokio::task::spawn_blocking(move || {
                event_rx
                    .recv_timeout(Duration::from_secs(2))
                    .expect("network event channel closed or timed out")
            })
            .await
            .expect("network event receiver task panicked")
        }

        let (client_end, server_end) = memory_pipes();
        let mut peer = Conn::from_memory(server_end);
        let (event_tx, event_rx) = crossbeam_channel::bounded(4096);
        let (chat_tx, chat_rx) = crossbeam_channel::bounded(64);
        let (packet_tx, packet_rx) = mpsc::unbounded_channel();
        let secure_chat_required = Arc::new(AtomicBool::new(false));

        let client = tokio::spawn(connect_to_server(
            ConnectArgs {
                transport: Transport::Memory(client_end),
                username: "Steve".to_owned(),
                uuid: Uuid::nil(),
                access_token: None,
                view_distance: 8,
            },
            event_tx,
            chat_rx,
            packet_tx,
            packet_rx,
            secure_chat_required,
        ));

        assert!(matches!(
            sent(&mut peer).await,
            ServerboundHandshakePacket::Intention(p) if p.protocol_version == LATEST.protocol
        ));
        assert!(matches!(
            sent(&mut peer).await,
            ServerboundLoginPacket::Hello(p) if p.name == "Steve"
        ));
        peer.write_packet(ClientboundLoginFinished {
            game_profile: GameProfile::new(Uuid::nil(), "Steve".to_owned()),
            session_id: Uuid::nil(),
        })
        .await
        .unwrap();
        assert!(matches!(
            sent(&mut peer).await,
            ServerboundLoginPacket::LoginAcknowledged(_)
        ));
        assert!(matches!(
            sent(&mut peer).await,
            ServerboundConfigPacket::CustomPayload(p) if p.identifier.to_string() == "minecraft:brand"
        ));
        assert!(matches!(
            sent(&mut peer).await,
            ServerboundConfigPacket::ClientInformation(p) if p.information.view_distance == 8
        ));
        peer.write_packet(ClientboundFinishConfiguration)
            .await
            .unwrap();
        let _ = received(&event_rx).await;
        let _ = received(&event_rx).await;

        peer.write_packet(ClientboundGamePacket::Login(ClientboundLogin {
            player_id: MinecraftEntityId(7),
            hardcore: false,
            levels: vec!["minecraft:overworld".into()],
            max_players: 20,
            chunk_radius: 12,
            simulation_distance: 10,
            reduced_debug_info: false,
            show_death_screen: true,
            do_limited_crafting: false,
            common: azalea_protocol::packets::common::CommonPlayerSpawnInfo {
                dimension_type: azalea_registry::data::DimensionKind::new_raw(0),
                dimension: "minecraft:overworld".into(),
                seed: 42,
                game_type: GameMode::Survival,
                previous_game_type: OptionalGameType(None),
                is_debug: false,
                is_flat: false,
                last_death_location: None,
                portal_cooldown: 0,
                sea_level: 63,
            },
            online_mode: false,
            enforces_secure_chat: true,
        }))
        .await
        .unwrap();

        let mut login_events = Vec::new();
        while !login_events
            .iter()
            .any(|event| matches!(event, NetworkEvent::PlayerLogin { .. }))
        {
            login_events.push(received(&event_rx).await);
        }

        chat_tx.send("hello".into()).unwrap();
        let warning = loop {
            if let NetworkEvent::ChatMessage { spans } = received(&event_rx).await {
                break spans;
            }
        };
        assert_eq!(warning[0].text, SECURE_CHAT_NOTICE);

        let deadline = tokio::time::Instant::now() + Duration::from_millis(100);
        let mut unsigned_chat_sent = false;
        while tokio::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            match tokio::time::timeout(remaining, peer.read_packet::<ServerboundGamePacket>()).await
            {
                Ok(Ok(packet)) => {
                    if matches!(
                        packet,
                        ServerboundGamePacket::Chat(_) | ServerboundGamePacket::ChatCommand(_)
                    ) {
                        unsigned_chat_sent = true;
                        break;
                    }
                }
                Ok(Err(_)) | Err(_) => break,
            }
        }
        assert!(
            !unsigned_chat_sent,
            "unsigned chat was written to the server"
        );
        client.abort();
    }

    /// Boots the real integrated SteelMC server, joins it through the same
    /// public connection handle used by the application, and waits for the
    /// first gameplay events. This is intentionally ignored because it
    /// generates and saves a world, but it is the first end-to-end gate for
    /// login, configuration, terrain, and client/server transport wiring.
    #[cfg(feature = "singleplayer")]
    #[test]
    #[ignore = "boots SteelMC and generates terrain"]
    fn integrated_server_acceptance_reaches_gameplay() {
        use std::fs;
        use std::time::Duration;

        use pomme_singleplayer::{LaunchOptions, Progress, ServerTransport, launch};
        use tokio::runtime::Runtime;

        struct TempWorld(std::path::PathBuf);

        impl Drop for TempWorld {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }

        let runtime = Runtime::new().expect("acceptance runtime should start");
        runtime.block_on(async {
            let world = TempWorld(
                std::env::temp_dir()
                    .join(format!("pomme-client-acceptance-{}", std::process::id())),
            );
            let _ = fs::remove_dir_all(&world.0);
            fs::create_dir_all(&world.0).expect("acceptance world directory should be writable");

            let (client_end, server_end) = crate::net::conn::memory_pipes();
            let mut pending = launch(LaunchOptions {
                save_path: world.0.clone(),
                seed: 1,
                view_distance: 2,
                simulation_distance: 2,
                transport: ServerTransport {
                    read: Box::new(server_end.rx),
                    write: Box::new(server_end.tx),
                },
            });

            let startup = tokio::time::timeout(Duration::from_secs(120), async {
                loop {
                    match pending.poll() {
                        Progress::Starting => tokio::time::sleep(Duration::from_millis(25)).await,
                        Progress::Ready(world) => break world,
                        Progress::Failed(error) => panic!("integrated server failed: {error}"),
                    }
                }
            })
            .await
            .expect("integrated server startup timed out");

            let connection = spawn_connection(
                &runtime,
                ConnectArgs {
                    transport: Transport::Memory(client_end),
                    username: "AcceptancePlayer".to_owned(),
                    uuid: uuid::Uuid::nil(),
                    access_token: None,
                    view_distance: 2,
                },
            );

            let mut connected = false;
            let mut player_login = false;
            let mut dimension = false;
            let mut chunk = false;
            let mut trace = vec![format!("protocol={}", LATEST.protocol)];
            tokio::time::timeout(Duration::from_secs(60), async {
                while !(connected && player_login && dimension && chunk) {
                    while let Ok(event) = connection.event_rx.try_recv() {
                        match event {
                            NetworkEvent::Connected => {
                                trace.push("Connected".into());
                                connected = true;
                            }
                            NetworkEvent::PlayerLogin { .. } => {
                                trace.push("PlayerLogin".into());
                                player_login = true;
                            }
                            NetworkEvent::DimensionInfo { .. } => {
                                trace.push("DimensionInfo".into());
                                dimension = true;
                            }
                            NetworkEvent::ChunkLoaded { .. } => {
                                trace.push("ChunkLoaded".into());
                                chunk = true;
                            }
                            NetworkEvent::Disconnected { reason } => {
                                panic!(
                                    "integrated server disconnected during acceptance: {reason}; \
                                     trace={trace:?}"
                                )
                            }
                            _ => {}
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(25)).await;
                }
            })
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "client did not reach gameplay before the acceptance timeout; trace={trace:?}"
                )
            });

            assert!(connected, "client never emitted Connected");
            assert!(player_login, "client never emitted PlayerLogin");
            assert!(dimension, "client never emitted DimensionInfo");
            assert!(chunk, "client never emitted ChunkLoaded");

            drop(connection);
            drop(startup);
            drop(pending);
            assert!(world.0.join("minecraft").is_dir());
        });
    }
}
