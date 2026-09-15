# Client acceptance scenarios

These scenarios are the minimum evidence required before calling a protocol
version playable. They are intentionally separated from unit tests because
they exercise the public client connection path and a running server.

## Local deterministic gate

Run the real integrated-server scenario with:

```sh
cargo test -p pomme-client --all-features integrated_server_acceptance_reaches_gameplay -- --ignored --nocapture
```

The scenario launches SteelMC through `pomme_singleplayer::launch`, uses the
same `Transport::Memory` and `spawn_connection` path as the application, waits
with bounded polling instead of a fixed startup delay, and isolates its temporary world directory. It
asserts:

- the integrated server reaches `Progress::Ready` within 120 seconds;
- the client emits `Connected` and `PlayerLogin`;
- dimension information and at least one chunk arrive within 60 seconds;
- a server disconnect during the flow fails the test with its reason;
- the generated world is saved and cleaned up.

The client-side gameplay gate also has focused coverage for the standard menu
contracts and server-authoritative operations: generic 3x3 menus (dispenser,
dropper, and crafter), grindstone, lectern, loom, smithing, cartography, and
stonecutter slot layouts, plus creative-mode item dropping. These checks prove
wire/menu routing and compilation, but they do not replace click-through tests
against a live vanilla server.

## Non-interactive graphics smoke checks

The client can capture a rendered frame without sending synthetic mouse or
keyboard input:

```sh
# Initializes Vulkan, renders the title screen, and writes a PNG.
cargo run -p pomme-client -- \
  --capture-first-frame \
  --exit-after-screenshot

# With a reachable server, captures the first fully loaded gameplay frame.
cargo run -p pomme-client -- \
  --quick-access-multiplayer 127.0.0.1:25565 \
  --capture-gameplay-frame \
  --exit-after-screenshot
```

Captures are written to `<game directory>/screenshots` using the normal F2
readback path. The first command validates window creation, asset loading,
GPU rendering, presentation, and PNG readback without starting a world. The
second also needs a server and validates the loaded-world render boundary, but
neither command proves pixel parity or general gameplay. Review the PNG or
compare it with a platform-specific baseline separately.

The CLI rejects names outside Minecraft's 3-16 character ASCII grammar before
the login packet is sent. For example, the former `VisualAcceptance2` test name
is 17 characters and previously reached SteelMC as an opaque `malformed packet
value: To long` disconnect.

## Required server-backed scenarios

The following scenarios remain required for a validated release tier:

1. Login and configuration, including encryption and resource-pack policy.
2. World loading, light updates, movement, collision, mining, placement, and item use.
3. Inventory transactions, every supported container, merchant trades, and rejected actions.
4. Chat, signed chat, signed commands, suggestions, and last-seen acknowledgements.
5. Entities, effects, vehicles, dimensions, death, respawn, and forced disconnect.
6. Explicit Retry behavior, including a failed retry and a successful retry.

Each scenario must use a bounded timeout, capture the protocol/version, and
report the first failing event or packet. Passing unit tests alone is not a
substitute for this matrix.
