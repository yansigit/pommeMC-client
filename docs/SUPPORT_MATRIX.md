# Minecraft support matrix

Pomme tracks protocol support separately from gameplay validation. An embedded
packet table does not by itself mean that a version is ready for general play.

## Support tiers

| Tier | Meaning |
| --- | --- |
| Embedded | Packet and registry data are present and compile. |
| Translated | The client has a translation path for the version, but gameplay coverage is not complete. |
| Playable | Core login, world loading, movement, interaction, inventory, chat, and disconnect flows pass automated tests. |
| Validated | Playable plus the real-server acceptance scenarios pass for the published platform and fixture. |

## Current baseline

| Protocol family | Embedded | Translated | Playable | Validated |
| --- | ---: | ---: | ---: | ---: |
| 1.20.x | Yes | Partial | Not yet claimed | No live-server result |
| 1.21.x | Yes | Partial | Not yet claimed | No live-server result |
| 26.1 | Yes | Yes | Not yet claimed | No live-server result |
| 26.2 | Yes | Native | Not yet claimed | SteelMC acceptance test available, run explicitly |

The repository's unit and integration tests currently establish protocol,
translation, transport, and integrated-server behavior. They do not establish
vanilla-equivalent gameplay, server compatibility, or release readiness. A
version may be promoted only when the acceptance scenarios in
`docs/ACCEPTANCE.md` pass and known limitations are documented.
