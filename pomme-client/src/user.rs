use md5::Digest as _;

const MIN_USERNAME_LENGTH: usize = 3;
const MAX_USERNAME_LENGTH: usize = 16;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum UserDataError {
    #[error(
        "invalid Minecraft username {username:?}: use 3-16 ASCII letters, digits, or underscores"
    )]
    InvalidUsername { username: String },
}

pub struct UserData {
    pub username: String,
    pub uuid: uuid::Uuid,
    pub access_token: Option<String>,
    /// True when `uuid` is a launcher-supplied profile UUID rather than a
    /// synthesized offline one (which has no Mojang profile to fetch).
    pub has_profile: bool,
}

impl UserData {
    pub fn from_args(
        username: Option<String>,
        uuid: Option<String>,
        access_token: Option<String>,
    ) -> Result<Self, UserDataError> {
        let username = username.unwrap_or_else(|| "Steve".to_string());
        if !is_valid_username(&username) {
            return Err(UserDataError::InvalidUsername { username });
        }

        let uuid = uuid.and_then(|s| uuid::Uuid::parse_str(&s).ok());
        let has_profile = uuid.is_some();
        let uuid = uuid.unwrap_or_else(|| Self::offline_uuid(&username));

        Ok(Self {
            username,
            uuid,
            access_token,
            has_profile,
        })
    }

    /// Vanilla `UUIDUtil.createOfflinePlayerUUID`: a bare MD5 of the prefixed
    /// name, not a namespaced v3 UUID.
    fn offline_uuid(username: &str) -> uuid::Uuid {
        let digest = md5::Md5::digest(format!("OfflinePlayer:{username}").as_bytes());
        uuid::Builder::from_md5_bytes(digest.into()).into_uuid()
    }
}

/// Minecraft's login protocol accepts only the vanilla player-name grammar.
/// Keep this check at the public CLI boundary so an invalid name cannot reach a
/// server and be reported as an opaque malformed-login/disconnect error.
#[must_use]
pub fn is_valid_username(username: &str) -> bool {
    (MIN_USERNAME_LENGTH..=MAX_USERNAME_LENGTH).contains(&username.len())
        && username
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

#[cfg(test)]
mod tests {
    use super::{UserData, UserDataError, is_valid_username};

    #[test]
    fn validates_vanilla_player_names() {
        assert!(is_valid_username("Steve"));
        assert!(is_valid_username("Alex_123"));
        assert!(is_valid_username("AcceptancePlayer"));
        assert!(is_valid_username("abc"));
        assert!(is_valid_username("sixteen_chars___"));
        assert!(!is_valid_username("ab"));
        assert!(!is_valid_username("way_too_long_player_name"));
        assert!(!is_valid_username("name-with-dash"));
        assert!(!is_valid_username("プレイヤー"));
    }

    #[test]
    fn rejects_invalid_username_before_constructing_identity() {
        assert!(matches!(
            UserData::from_args(
                Some("AcceptancePlayerX".to_string()),
                None,
                None,
            ),
            Err(UserDataError::InvalidUsername { username })
                if username == "AcceptancePlayerX"
        ));
    }

    #[test]
    fn offline_uuid_matches_vanilla() {
        for (name, expected) in [
            ("Notch", "b50ad385-829d-3141-a216-7e7d7539ba7f"),
            ("Steve", "5627dd98-e6be-3c21-b8a8-e92344183641"),
            ("Alex", "36532b5e-c442-3dbb-a24c-c7e55d0f979a"),
        ] {
            assert_eq!(UserData::offline_uuid(name).to_string(), expected);
        }
    }
}
