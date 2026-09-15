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

## Integrated singleplayer gameplay status

Pomme's singleplayer mode embeds SteelMC. It can start a persistent world,
generate and light chunks, stream them through the normal client connection,
and support core movement, collision, block interaction, inventories, and
rendering. This is an exploration and implementation-testing baseline, not a
claim of complete Minecraft survival gameplay.

| Area | Current evidence | Status |
| --- | --- | --- |
| Terrain and structures | SteelMC's parity suite reports 7,500 sampled chunks across the Overworld, Nether, and End matching its vanilla reference block-for-block. Pomme's integrated-server acceptance test also requires a generated chunk to reach the client. | Strongest area, but not proof that every seed, chunk, or gameplay interaction is identical. |
| Passive mobs | Concrete server implementations and tests exist for pigs, chickens, cows, and sheep. The focused mob suite currently passes 95 tests. | Partial. Most vanilla entities and behaviors are not covered by this suite. |
| Hostile mobs | The embedded server currently has a concrete hostile implementation for the endermite only. | Incomplete. Zombies, skeletons, creepers, and the broader hostile roster are not functional server-side equivalents. |
| Natural spawning | The chunk-generation spawn stage is currently a no-op. | Not implemented for normal world population. |
| Bosses | The server boss module has no concrete boss implementation. | Ender Dragon and Wither battles are not implemented. |
| Survival parity | SteelMC identifies itself as pre-alpha and states that survival gameplay, entities, and vanilla parity remain incomplete. | Not ready to be described as a complete Minecraft replacement. |

Client support for receiving or rendering an entity, boss bar, particle, sound,
or packet does not imply that the integrated server implements the associated
AI, spawning rules, combat encounter, progression, or persistence behavior.
