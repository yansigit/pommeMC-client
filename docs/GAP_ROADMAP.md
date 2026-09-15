# Gap closure roadmap

This document is a current-state plan for closing Pomme's remaining client,
compatibility, rendering, validation, and integrated-server gaps. It is based on
local source inspection plus cross-repository research performed with the GitHub
CLI on 2026-09-15.

It is intentionally stricter than a feature checklist. A gap is closed only when
its user-visible behavior is exercised through the public client path with a
bounded, reproducible test.

## Scope and ordering

Pomme has two related but independent products:

1. A multiplayer client that connects to remote Minecraft Java servers.
2. An optional integrated singleplayer server provided by the vendored SteelMC
   workspace.

The remote client can reach a validated release before SteelMC reaches complete
survival parity. SteelMC mob spawning, hostile mobs, and bosses must therefore
remain a separate release track instead of blocking the client.

The phrase "all gaps" is necessarily bounded. This roadmap interprets it as all
gaps evidenced by first-party source markers, support claims, acceptance
requirements, and the vendored SteelMC implementation as of the research date.
It does not claim exhaustive parity with every undocumented Minecraft behavior,
and it treats the request as a researched implementation plan rather than a
request to implement every epic in one change.

The order below follows four rules:

1. Build acceptance evidence before adding more breadth.
2. Replace hard-coded approximations with generated or centralized data.
3. Complete state models before adding render layers or UI for that state.
4. Implement integrated-server gameplay from shared foundations upward:
   spawning, AI, common mob behavior, concrete mobs, then bosses.

## Current baseline

The repository is no longer missing several capabilities identified by earlier
audits. Current code already includes:

- modern signed chat, signed command arguments, and last-seen tracking in
  `pomme-client/src/net/connection.rs`;
- a reconnect action and Retry UI in `pomme-client/src/app/core.rs` and
  `pomme-client/src/ui/menu/servers.rs`;
- merchant offers, selection, and the merchant screen;
- the standard workstation and generic container routes documented in
  `docs/ACCEPTANCE.md`;
- scoreboard sidebar, action bar, projected player nameplates, boss bars, and
  spectator menu state;
- generated-world integrated-server login and chunk acceptance;
- noninteractive title-screen and gameplay screenshot capture.

These should not be reimplemented. Their remaining gap is live-server validation,
edge-case coverage, and parity across every published protocol family.

Local inspection found 126 explicit first-party TODO/FIXME-style markers, 125 of
them in `pomme-client`. They are concentrated in:

| Area | Main files |
| --- | --- |
| In-game behavior and UI integration | `app/phases/in_game.rs`, `app/core.rs` |
| Older protocol translation | `net/translate.rs`, `net/handler.rs` |
| Entity models and equipment layers | `renderer/entity_model.rs`, `renderer/pipelines/entity_renderer.rs` |
| HUD and overlays | `ui/hud.rs`, `ui/player_tab.rs`, `ui/boss_bar.rs` |
| Fluids and chunk rendering | `renderer/chunk/mesher.rs`, `renderer/chunk/buffer.rs` |
| Movement, abilities, and interaction | `physics/movement.rs`, `player/mod.rs`, `player/interaction.rs` |
| Camera and visual effects | `renderer/camera.rs` |

A TODO count is not a release metric. Several markers are cosmetic or maintenance
notes. The epics below classify them by user impact and define the evidence
needed to close each class.

## GitHub research snapshot

The following revisions were inspected with `gh repo view`, `gh api`, and
`gh search code`:

| Repository | Revision | License | Primary value |
| --- | --- | --- | --- |
| [azalea-rs/azalea](https://github.com/azalea-rs/azalea/tree/b65fa8cf1bb957976cefa926b9b500d44767d806) | `b65fa8c` | MIT | Rust protocol, signing, physics, and deterministic simulation tests |
| [GeyserMC/MCProtocolLib](https://github.com/GeyserMC/MCProtocolLib/tree/19783c29ece24bc3f07f8ff08628549527e3de20) | `19783c2` | MIT | Typed packets and phase/state separation |
| [ViaVersion/ViaVersion](https://github.com/ViaVersion/ViaVersion/tree/5f98cba45b2d9c72875dfcbcf70994bd702f5193) | `5f98cba` | GPL-3.0 | Version-keyed packet, registry, item, entity, and component rewriters |
| [PrismarineJS/node-minecraft-protocol](https://github.com/PrismarineJS/node-minecraft-protocol/tree/0ebdd5120eca8840e3ffd6597cd4e83732b59c9a) | `0ebdd51` | BSD-3-Clause | Login/encryption/protocol fixtures and server/client test harnesses |
| [PrismarineJS/mineflayer](https://github.com/PrismarineJS/mineflayer/tree/91204b2a034f0663b39814771e236bcb7c8f26c8) | `91204b2` | MIT | High-level scripted gameplay and server-backed bot scenarios |
| [Bixilon/Minosoft](https://github.com/Bixilon/Minosoft/tree/18a17f311c8a0f52185a9065d096416b9bf2885d) | `18a17f3` | GPL-3.0 | Modern client state models, fluids, renderer features, and integration tests |
| [iceiix/stevenarella](https://github.com/iceiix/stevenarella/tree/815ac883389a871a888ea4436ad1af192cfeca7b) | `815ac88` | Apache-2.0 | Rust client rendering and entity organization |
| [Lea-fish/Leafish](https://github.com/Lea-fish/Leafish/tree/784d76f6836964c746621c506d0051b394d2d31f) | `784d76f` | Apache-2.0 | Rust inventory, entity, and particle organization |
| [KernelFreeze/Litecraft](https://github.com/KernelFreeze/Litecraft/tree/b357ea4d913fb8130b03ec5efc0185759e060b11) | `b357ea4` | MIT | Small clean-room Rust client patterns |
| [Pumpkin-MC/Pumpkin](https://github.com/Pumpkin-MC/Pumpkin/tree/02b16e15b224433892e84277ac99a3c919e6f33a) | `02b16e1` | GPL-3.0 | Rust server entity, combat, world, and gameplay systems |
| [Minestom/Minestom](https://github.com/Minestom/Minestom/tree/338580108e849e078cad18bb2bcb4ffb11e073bf) | `3385801` | Apache-2.0 | Composable AI goals, targets, navigation, metadata, and test architecture |

The research is reproducible with commands of this form:

```sh
gh repo view OWNER/REPO --json defaultBranchRef,description,licenseInfo,url
sha=$(gh api repos/OWNER/REPO/commits/HEAD --jq .sha)
gh api "repos/OWNER/REPO/git/trees/$sha?recursive=1" --jq '.tree[].path'
gh search code 'QUERY repo:OWNER/REPO' --json path,repository,url
```

Pomme is GPL-3.0-or-later, so the listed licenses are generally compatible with
the project. Directly adapted code still requires source-level provenance and an
entry in `THIRD_PARTY_LICENSES.md`. The vendored SteelMC workspace is
AGPL-3.0-or-later, so server-side adaptations must retain its notices and source
availability obligations. Keep client and integrated-server provenance
separate. Decompiled Mojang code may be used only to understand behavior and
construct independent tests. It must not be copied.

## P0: Make the acceptance matrix executable

This is the highest priority because every later epic needs an objective exit
gate. `docs/ACCEPTANCE.md` currently names the required scenarios, but only the
integrated login/chunk path and graphics smoke paths are automated.

### Deliverables

1. Add a `pomme-acceptance` crate or `tools/acceptance` binary.
2. Define a checked-in scenario manifest with:
   - protocol/version;
   - server distribution and fixture hash;
   - account mode;
   - bounded startup, action, and shutdown timeouts;
   - required client events and packet trace checkpoints;
   - screenshot or state artifacts;
   - expected disconnect or retry result.
3. Add a non-GUI scripted action channel to the client. It should invoke the
   same application actions as keyboard and mouse input, not mutate game state
   directly.
4. Record a compact packet/event trace for each failed scenario and report the
   first unmet checkpoint.
5. Run each scenario against:
   - the in-memory protocol fixture;
   - integrated SteelMC where applicable;
   - a pinned vanilla-compatible server fixture;
   - at least one strict online-mode server for encryption and secure chat.

### Required scenarios

- login, encryption, configuration, cookies, known packs, and resource-pack
  accept/decline/failure policy;
- initial chunk, light updates, movement, collision, teleport confirmation,
  mining, rollback, placement, and item use;
- every supported container, rejected transactions, merchant selection, and
  creative actions;
- unsigned and signed chat, signed commands, suggestions, and last-seen
  acknowledgements;
- entity spawn/update/remove, effects, equipment, passengers, vehicles,
  dimension changes, death, respawn, and forced disconnect;
- Retry after a failed connection and Retry after a server becomes available;
- title-screen capture, loaded-world capture, and deterministic visual fixtures.

### Reference patterns

- Azalea's simulation harness:
  [`simulation.rs`](https://github.com/azalea-rs/azalea/blob/b65fa8cf1bb957976cefa926b9b500d44767d806/azalea-client/src/test_utils/simulation.rs),
  [`correct_movement.rs`](https://github.com/azalea-rs/azalea/blob/b65fa8cf1bb957976cefa926b9b500d44767d806/azalea-client/tests/simulation/correct_movement.rs),
  [`mine_block_rollback.rs`](https://github.com/azalea-rs/azalea/blob/b65fa8cf1bb957976cefa926b9b500d44767d806/azalea-client/tests/simulation/mine_block_rollback.rs), and
  [`change_dimension_to_nether_and_back.rs`](https://github.com/azalea-rs/azalea/blob/b65fa8cf1bb957976cefa926b9b500d44767d806/azalea-client/tests/simulation/change_dimension_to_nether_and_back.rs).
- Minosoft's fixture-oriented integration tests, especially
  [`ContainerTransactionTest.kt`](https://github.com/Bixilon/Minosoft/blob/18a17f311c8a0f52185a9065d096416b9bf2885d/src/integration-test/kotlin/de/bixilon/minosoft/data/container/transaction/ContainerTransactionTest.kt)
  and its binary signed-chat packet fixtures.
- node-minecraft-protocol and mineflayer for process orchestration, temporary
  servers, scripted actions, and failure-first diagnostics.

### Exit gate

A protocol family cannot move to `Playable` until every required scenario has a
bounded automated result. It cannot move to `Validated` until the same scenario
set passes against the published real-server fixture on the release platform.

## P0: Finish compatibility behavior, not just packet-id translation

`pomme-client/src/net/translate.rs` translates protocols 763 through 775, while
26.2 is native. The remaining work is the set of explicitly documented lossy or
suppressed behaviors plus real-server validation.

### Close the explicit translation gaps

1. Restore pre-1.21.4 `pick_item` behavior.
2. Translate pre-1.21.5 creative-slot component values rather than passing an
   ambiguous bare component stream.
3. Rewrite 1.21.1 explosions instead of dropping them.
4. Preserve advancement toast icons on protocol 765.
5. Convert legacy mount slot counts to modern column counts.
6. Preserve dimension registries across incomplete reconfiguration.
7. Convert legacy cooldown item ids to cooldown groups.
8. Translate legacy item NBT used by player input and item interactions.
9. Lift shoulder-parrot, hanging-entity, arrow, and trident metadata indices.
10. Rewrite every remaining particle payload containing block states, items, or
    nested registry ids.
11. Carry the actual login/respawn dimension when older packets omit a modern
    field instead of synthesizing the Overworld where avoidable.
12. Encode older team color forms exactly.

### Architecture change

Split the large translation module by source protocol family while preserving the
existing direct-to-latest model:

```text
net/translate/
  mod.rs
  registry.rs
  p763.rs
  p764.rs
  ...
  p775.rs
  fixtures/
```

Each source-version module owns its packet layout changes, registry changes, and
golden binary fixtures. Share small field or component rewriters where layouts
are identical. ViaVersion's explicit protocol-path and packet-rewriter
registration is a useful organization reference, but Pomme should not introduce
an adjacent-version pipeline unless direct translation becomes unmaintainable.

### Exit gate

For every supported protocol, replay captured clientbound and serverbound frames
through the translator, decode them with the typed latest model, and pass the P0
acceptance suite on a real server.

## P0: Centralize authoritative client state

Several remaining gameplay and rendering TODOs are symptoms of missing shared
state rather than isolated render bugs.

### Abilities and attributes

Complete the local ability model with invulnerable, instabuild, and may-build
flags. Drive movement speed, sneaking speed, water movement efficiency, oxygen
bonus, attack range, and FOV from one attribute/effect pipeline.

Relevant local markers include:

- `app/core.rs` ability handling and FOV approximation;
- `net/handler.rs` incomplete ability flags;
- `physics/movement.rs` sneaking and water-efficiency constants;
- `player/mod.rs` ability and oxygen fields;
- `app/phases/in_game.rs` attack-range approximation.

Use Azalea's
[`attributes.rs`](https://github.com/azalea-rs/azalea/blob/b65fa8cf1bb957976cefa926b9b500d44767d806/azalea-entity/src/attributes.rs)
and Minosoft's
[`AttributeIT.kt`](https://github.com/Bixilon/Minosoft/blob/18a17f311c8a0f52185a9065d096416b9bf2885d/src/integration-test/kotlin/de/bixilon/minosoft/data/physics/attribute/AttributeIT.kt)
as state-model and test references.

### Entity presentation state

Add normalized pose, hand use, body rotation, sleep position, equipment assets,
wetness, vehicle state, and per-part visibility to `LivingEntity`. Renderers
should consume this normalized model and never decode version-specific metadata.

This unlocks:

- remote sleeping and correct swimming/gliding eye height;
- third-person held items, armor, saddles, horse armor, wolf armor, and custom
  head layers;
- handed bow/crossbow poses, zombie item-aware arms, drowned swim pose, and
  Enderman carried blocks;
- correct body rotation, water/rain wetness, death particles, and item-use FOV;
- vehicle HUD cooldowns and mount inventory correctness.

### Exit gate

Add metadata fixtures for each supported protocol and snapshot the normalized
entity state before testing rendering.

## P1: Replace hand-written block approximations with generated data

`pomme-client/src/physics/block_shape.rs` now covers common partial blocks, but
its own module description still limits it to a "handful" of classes and unknown
states fall back to full cubes. This can silently create collision and targeting
errors.

### Implementation

1. Extend `blockgen` to generate, for every block state:
   - collision boxes;
   - outline/interaction boxes;
   - occlusion faces;
   - suffocation and view-blocking flags;
   - fluid occupancy and fluid height inputs;
   - friction, speed, jump, and fall modifiers.
2. Store canonical deduplicated shape tables in `pomme-block`.
3. Remove family-name heuristics from the runtime path.
4. Keep hand-written logic only for neighbor-dependent shapes such as fences,
   walls, panes, redstone, scaffolding, and moving pistons.
5. Add exhaustive assertions that every embedded block state has an intentional
   collision and outline classification.

### References

- Azalea collision shapes:
  [`collision/blocks.rs`](https://github.com/azalea-rs/azalea/blob/b65fa8cf1bb957976cefa926b9b500d44767d806/azalea-physics/src/collision/blocks.rs).
- Minosoft collision and fluid integration fixtures:
  [`CollisionIT.kt`](https://github.com/Bixilon/Minosoft/blob/18a17f311c8a0f52185a9065d096416b9bf2885d/src/integration-test/kotlin/de/bixilon/minosoft/data/physics/gravity/CollisionIT.kt).

### Exit gate

Run generated-state exhaustiveness tests, shape golden tests, and scripted
walking/mining/placement scenarios through doors, fences, walls, panes, stairs,
slabs, beds, trapdoors, snow, scaffolding, fluids, and moving blocks.

## P1: Complete movement, fluids, interaction, and combat

Build on the generated block data and centralized attributes.

### Movement and fluids

- model eyes-in-lava and water-vision adaptation;
- use per-fluid drag, acceleration, and height rather than block-name checks;
- finish fall-distance edge cases and effect-driven movement modifiers;
- validate sprint, sneak, swim, crawl, climb, glide, fly, spectator noclip,
  knockback, and vehicle movement against deterministic traces.

Useful references are Azalea's
[`fluids.rs`](https://github.com/azalea-rs/azalea/blob/b65fa8cf1bb957976cefa926b9b500d44767d806/azalea-physics/src/fluids.rs)
and Minosoft's
[`FlowingFluidIT.kt`](https://github.com/Bixilon/Minosoft/blob/18a17f311c8a0f52185a9065d096416b9bf2885d/src/integration-test/kotlin/de/bixilon/minosoft/data/physics/fluid/flowing/FlowingFluidIT.kt).

### Interaction and combat

- centralize item-use duration and use-animation state;
- finish spear, bow, crossbow, shield, food, bucket, and place/use branching;
- make attack cooldown, attack range, creative instant break, may-build, and
  invulnerability authoritative;
- add rollback tests for rejected mining, placement, container, and entity
  actions;
- complete riding input, dismount, mount inventory, and passenger transitions.

### Exit gate

Every action must be checked in three states: locally predicted, server accepted,
and server rejected/resynchronized.

## P1: Complete inventory semantics

The UI routes are broad, but visual support is not proof of transaction parity.
Use one container transaction engine for click prediction, changed-slot hashing,
carried-item state, rejection rollback, and version translation.

### Work

- add property/state support for every displayed workstation;
- validate quick-craft, double-click collection, shift transfer, hotbar swap,
  clone, throw, outside click, and drag distribution;
- complete mount inventories and specialized result-slot rules;
- test merchant demand, special price, disabled offers, out-of-stock state, and
  rejected selections;
- preserve item components across old-protocol translation instead of creating
  component-less approximations where a reversible mapping exists.

Minosoft's
[`ContainerTransactionManager.kt`](https://github.com/Bixilon/Minosoft/blob/18a17f311c8a0f52185a9065d096416b9bf2885d/src/main/java/de/bixilon/minosoft/data/container/transaction/ContainerTransactionManager.kt)
is a useful transaction-boundary reference. Leafish's `src/inventory/` tree is a
smaller Rust reference for separating slot layouts from screens.

## P1: Finish world and entity rendering parity

### Fluids and translucency

Close the explicit markers in `renderer/chunk/mesher.rs` and
`renderer/chunk/buffer.rs`:

- biome water tint;
- per-corner fluid height averaging;
- flowing texture selection and direction;
- level-dependent surface heights;
- water/lava adjacency rules;
- depth sorting for translucent surfaces;
- non-occluding full-cube ambient-occlusion behavior.

Minosoft's
[`FluidCornerHeightUtil.kt`](https://github.com/Bixilon/Minosoft/blob/18a17f311c8a0f52185a9065d096416b9bf2885d/src/main/java/de/bixilon/minosoft/gui/rendering/chunk/mesher/fluid/FluidCornerHeightUtil.kt)
and
[`FluidCornerHeightUtilTest.kt`](https://github.com/Bixilon/Minosoft/blob/18a17f311c8a0f52185a9065d096416b9bf2885d/src/integration-test/kotlin/de/bixilon/minosoft/gui/rendering/chunk/mesher/fluid/FluidCornerHeightUtilTest.kt)
provide a good algorithm/test split.

### Block entities

- sample sign lighting at the block position and support wall-sign orientation;
- render copper chest variants and remaining supported chest combinations;
- add deterministic fixtures for every block entity model and state;
- keep unknown block entities losslessly stored even when not rendered.

Minosoft's
[`SignBlockEntityRenderer.kt`](https://github.com/Bixilon/Minosoft/blob/18a17f311c8a0f52185a9065d096416b9bf2885d/src/main/java/de/bixilon/minosoft/gui/rendering/chunk/entities/renderer/sign/SignBlockEntityRenderer.kt)
and
[`SignBlockEntityTest.kt`](https://github.com/Bixilon/Minosoft/blob/18a17f311c8a0f52185a9065d096416b9bf2885d/src/integration-test/kotlin/de/bixilon/minosoft/data/entities/block/sign/SignBlockEntityTest.kt)
show the desired separation between parsed state and renderer.

### Entity layers and debug rendering

Implement the normalized entity state first, then add equipment layers, carried
items/blocks, overlays, armor, saddles, flowers, nameplate depth behavior, and
F3+B hitboxes. Minosoft's
[`EntityRendererManager.kt`](https://github.com/Bixilon/Minosoft/blob/18a17f311c8a0f52185a9065d096416b9bf2885d/src/main/java/de/bixilon/minosoft/gui/rendering/entities/EntityRendererManager.kt)
and
[`HitboxFeature.kt`](https://github.com/Bixilon/Minosoft/blob/18a17f311c8a0f52185a9065d096416b9bf2885d/src/main/java/de/bixilon/minosoft/gui/rendering/entities/feature/hitbox/HitboxFeature.kt)
are useful feature-registration references.

### Particles, audio, and asset-derived behavior

- generate block sound groups instead of maintaining the partial mapping in
  `world/block/sound.rs` by hand;
- complete level-event and entity-event particle/sound routing, including death
  poofs and payloads that contain block or item state;
- test sound category, attenuation, looping, replacement, and stop semantics;
- finish fixed and biome-derived foliage colors, default-skin selection, player
  overlay layers, and missing equipment assets;
- add fixture coverage for particles and sounds even when a stable pixel or
  waveform baseline is not practical.

Minosoft's particle renderer test at
[`ParticleRendererTest.kt`](https://github.com/Bixilon/Minosoft/blob/18a17f311c8a0f52185a9065d096416b9bf2885d/src/integration-test/kotlin/de/bixilon/minosoft/gui/rendering/particle/ParticleRendererTest.kt)
is a useful example of testing lifecycle behavior separately from a full frame.

### Exit gate

Create deterministic visual scenes for fluids, partial blocks, each equipment
layer, each block entity, weather, dimensions, and HUD overlays. Compare captures
with platform-specific baselines and a small tolerance rather than relying on
manual inspection alone.

## P1: Finish HUD, camera, input, and accessibility behavior

Group the remaining cosmetic and interaction markers so they do not fragment
core work:

- camera: bow draw, spyglass, smooth camera, nausea, death, water vision,
  dimension fog, rain attenuation, and invert-Y preference;
- HUD: powder snow, spyglass, nausea and portal overlays, world-border vignette,
  heart blink/damage history, regen wave, poison/wither/frozen/hardcore variants,
  food shake, mount cooldowns, and boss-bar music/darkening/fog flags;
- debug: permission-aware F3 actions, F3+B hitboxes, debug query results, F3+P,
  panoramic screenshots, and debug options;
- input: complete migration to the action system, dynamic gamepad rebinding, and
  focus-ring coverage for every screen;
- UI: language selection, pause-on-focus-loss preference, clickable screenshot
  links, InBed chat, tutorial toasts, and nonblocking world file operations.

These are P1 when they affect control, information, or accessibility. Pure visual
flavor remains P2.

## P0 server track: Implement natural spawning foundations

SteelMC already has entity factories, spawn reasons, common spawn coordination,
AI goals, navigation, combat, persistence, passive animals, and many shared
systems. The world-generation spawn stage is nevertheless empty:
`third_party/SteelMC/steel-core/src/worldgen/stages/spawn.rs` contains a no-op
`generate` function. The loading pyramid also uses a no-op Spawn stage.

Implement natural spawning before adding many concrete mobs.

### Required foundations

1. Spawn placement registry keyed by entity type and placement category.
2. Biome/dimension spawn tables and weighted group selection.
3. Per-category mob caps, local density limits, player-distance rules, and chunk
   eligibility.
4. Light, block-tag, fluid, collision, difficulty, game-rule, and structure
   predicates.
5. Pack/group creation with shared `SpawnGroupData` and finalization.
6. Despawn, persistence-required, named, leashed, passenger, and recently-hurt
   rules.
7. Deterministic random sources and tests for every predicate.
8. Separate chunk-generation creature spawning from per-tick natural spawning.

### Reference architecture

- Pumpkin is the closest Rust server reference for entity and gameplay
  organization. Treat GPL code as an attributed reference or adapt it with clear
  provenance.
- Minestom's `EntityAIGroup`, `GoalSelector`, `TargetSelector`, and pathfinding
  packages are useful for modular AI composition, but Minestom is an engine and
  does not aim at automatic vanilla survival parity.
- SteelMC's existing `entity/ai/goal/`, `entity/ai/navigation.rs`,
  `entity/animal.rs`, and `entity/spawn.rs` should remain the local foundation.
  Do not replace them with a second AI framework.

### Exit gate

Given fixed seeds, players, chunks, time, weather, and difficulty, spawning tests
must produce deterministic category counts and placements within vanilla-derived
bounds. Long-running tests must prove caps, despawning, and persistence.

## P1 server track: Expand mobs in dependency tiers

Do not implement mobs in registry order. Implement them by reusable behavior.

### Tier 1: Common hostile baseline

- zombie, husk, drowned;
- skeleton, stray, bogged;
- spider and cave spider;
- creeper;
- slime and magma cube.

Build shared daylight burning, melee/ranged attack, targeting, equipment,
reinforcement, conversion, swelling/explosion, climbing, and size-splitting
behaviors while adding this tier.

### Tier 2: Common passive and neutral families

- rabbit, wolf, cat, fox, goat, horse family, llama, camel;
- villagers, wandering traders, iron golems, snow golems;
- bees, turtles, frogs, axolotls, fish, squid, dolphins;
- piglins, hoglins, zombified piglins, Endermen.

This tier adds taming, trust, anger, temptation, breeding variants, inventories,
trading, village/POI integration, aquatic navigation, schools/flocks, and
cross-entity reputation.

### Tier 3: Structured encounters

- raids and illagers;
- guardians and ocean monuments;
- wardens and vibration behavior;
- trial spawner and breeze encounter behavior;
- shulkers and End-city behavior.

Each concrete mob requires unit tests for attributes, metadata, AI selection,
damage, drops/XP, save/load, and packet-observable state, plus one integrated
encounter test.

## P1 server track: Complete the survival substrate

Mob breadth alone will not make integrated singleplayer complete. SteelMC uses
an NBT-preserving `UnimplementedBlockEntity` fallback for registry types that do
not yet have runtime behavior. Its own README also describes entity, block
entity, and gameplay support as early and incomplete.

Maintain a generated implementation ledger for blocks, block entities, items,
menus, recipes, loot tables, advancements, entities, and world events. Each
registry entry must be classified as complete, intentionally data-only,
losslessly preserved, or unsupported. Then close the systems in dependency
order:

1. container-owning and ticking block entities;
2. furnaces, brewing, enchanting, anvils, smithing, trading, recipes, and fuel;
3. loot, drops, XP, statistics, advancements, and progression triggers;
4. redstone components, scheduled ticks, pistons, rails, and vehicles;
5. portals, dimension travel, beds/anchors, maps, and world saved data;
6. remaining structure post-processing and flat-world feature/lake generation;
7. projectiles, explosions, fishing, item frames, paintings, and hanging
   entities;
8. save/load, chunk unload/reload, and restart tests for every stateful system.

The fallback is valuable because it prevents data loss, but preservation is not
behavioral support. The ledger and acceptance suite must keep those claims
separate.

## P2 server track: Bosses and progression

`third_party/SteelMC/steel-core/src/entity/entities/mobs/bosses/mod.rs` currently
contains no concrete boss implementation. Bosses should be last because they
integrate nearly every prior subsystem.

### Wither sequence

1. Summoning block pattern and world event.
2. Invulnerability charge and explosion.
3. Target selection, flight, skull projectiles, armor phase, regeneration, and
   block destruction.
4. Boss bar, sounds, particles, damage immunities, drops, and advancement hooks.
5. Save/load and chunk-ticket behavior.

### Ender Dragon sequence

1. End fight persistent state and crystal tracking.
2. Phase manager and flight/path nodes.
3. Per-part hitboxes and damage routing.
4. Perching, breath, charge, strafing, crystal healing, and death sequence.
5. Exit portal, gateways, egg, XP, boss bar, music, fog, and persistence.

A boss is complete only when a full deterministic encounter can start, survive a
save/reload, finish, and produce the expected world/progression state.

## P2: Performance, platform, and maintenance cleanup

After correctness gates are in place:

- lower meshing worker priority on non-Windows;
- deduplicate renderer meshes and packing implementations;
- make large world-file operations asynchronous;
- extend cloud range without unbounded geometry or memory growth;
- finish atlas and stream utility cleanups;
- add benchmark thresholds for chunk meshing, translation, world generation,
  entity ticking, and screenshot capture.

Performance changes must keep the acceptance traces and visual baselines stable.

## Recommended execution waves

| Wave | Scope | Gate before next wave |
| --- | --- | --- |
| 0 | Acceptance runner, scenario manifest, trace artifacts | Current integrated and graphics scenarios run through the new runner |
| 1 | Live 26.2 login, secure chat, resource packs, movement, inventory, entities, retry | 26.2 is `Validated` on one pinned fixture/platform |
| 2 | Complete lossy translation TODOs and run the same matrix for 1.21.x | 1.21.x is `Playable`, then `Validated` |
| 3 | Run and repair the matrix for 1.20.x | 1.20.x is `Playable`, then `Validated` |
| 4 | Generated block shapes, centralized attributes/abilities, interaction rollback | Physics and interaction scenes pass deterministic traces |
| 5 | Fluid renderer, entity presentation state/layers, block entities, HUD/camera | Visual fixture suite passes baselines |
| 6 | SteelMC natural spawning, survival-substrate ledger, and hostile Tier 1 | Deterministic caps, despawn, stateful block-entity, save/load, and encounter tests pass |
| 7 | Remaining survival substrate, passive/neutral Tier 2, and structured Tier 3 | Survival loops and integrated encounters pass |
| 8 | Wither, Ender Dragon, progression, and final polish | Full encounter and persistence tests pass |

## Definition of done for every gap

A gap is closed only when all of these are true:

1. The behavior is represented in an authoritative model rather than inferred in
   multiple render or UI call sites.
2. Version-specific conversion happens only at the network boundary.
3. Unit or fixture tests cover the underlying algorithm or mapping.
4. A public-path acceptance scenario covers the user-visible result.
5. The test has a bounded timeout and reports the first failed checkpoint.
6. Any adapted third-party code has pinned provenance and license attribution.
7. `docs/SUPPORT_MATRIX.md` is updated only after the corresponding evidence
   passes.

This ordering turns the current broad alpha into measurable vertical slices. The
first practical milestone is not "implement every TODO". It is a validated 26.2
remote-client path, followed by protocol-family promotion, generated physics
inputs, rendering parity, and a separately gated SteelMC survival track.

## Verification record for this roadmap

The current baseline and ordering were exercised through the project's public
paths on 2026-09-15, not only inferred from source inspection:

- `cargo test -p pomme-client` passed 361 tests with one ignored acceptance test;
- `cargo test -p pomme-protocol` passed all 40 protocol tests;
- `cargo test -p pomme-singleplayer -- --include-ignored` passed the real
  two-world lifecycle test;
- the documented ignored integrated-server acceptance test passed, proving the
  in-memory client connection reached gameplay through SteelMC;
- the documented title-screen capture initialized audio, Vulkan on an Apple M3
  Pro, assets, block/item models, and renderer pipelines, then wrote a valid
  1,017,396-byte PNG;
- the documented singleplayer gameplay capture launched SteelMC, generated the
  world and copper-golem fixture, completed login and configuration, entered the
  game, received dimension/player state, rendered a valid 940,684-byte PNG, and
  saved 1,127 chunks during clean shutdown.

The first title-screen attempt also found a real macOS integration defect: local
Homebrew Vulkan loaders were installed but not provided to the linker. The client
build script now discovers either `VULKAN_SDK/lib`, Apple Silicon Homebrew, or
Intel Homebrew loader paths. Re-running the same public command then passed.
The official release Clippy command also exposed two pre-existing test-module
ordering failures. Moving those tests behind their production items preserved
all behavior, after which formatting, release Clippy with warnings denied, and
the complete client invocation finished with 361 passed tests and the documented
integrated acceptance test ignored. That ignored test passed when invoked
separately.

The full remote-server matrix remains unverified because the repository does not
yet provide the pinned vanilla-compatible fixtures, online-mode credentials, or
acceptance runner required to execute it reproducibly. Closing that observable
constraint is Wave 0 and is why the roadmap does not promote any remote protocol
family beyond its evidence in `docs/SUPPORT_MATRIX.md`.

### Whole-result requirement traceability

After all roadmap, build, and quality-gate changes were present together, the
complete result was checked again:

| Requirement or changed output | Whole-result check | Observed result |
| --- | --- | --- |
| Bound "all gaps" to repository evidence | Re-ran the Git-native marker census at the original revision and final `HEAD` | Both revisions contain 126 first-party Rust markers, 125 in `pomme-client`; the roadmap's classification did not lose or invent a source marker |
| Do not re-plan existing behavior | Ran the full client tests and integrated gameplay path | 361 default client tests passed, one acceptance test was intentionally ignored, and that acceptance passed separately through the real client-to-SteelMC connection |
| Research implementations across repositories with `gh` | Resolved every pinned full SHA and repository license through the GitHub REST API | All 11 revisions and SPDX identifiers matched the research table |
| Cover client gaps | Checked the roadmap's protocol, state, physics, movement, interaction, inventory, rendering, fluid, entity, UI, camera, input, and accessibility sections | Every mapped client domain is present with deliverables, references, and an exit gate |
| Cover integrated-server gaps separately | Checked the spawning, mob tiers, survival substrate, bosses, and progression sections, then ran integrated gameplay | The tracks remain independently gated; SteelMC launched, generated a world, admitted the client, rendered gameplay, and saved cleanly |
| Provide actionable ordering | Checked Waves 0 through 8 and each epic's gate | The document orders evidence first, client validation next, shared foundations before presentation, and bosses last |
| Preserve license boundaries | Rechecked remote SPDX data plus local Pomme and SteelMC license declarations | Client references remain GPL-compatible and SteelMC's AGPL boundary is called out separately |
| Link the roadmap from the public support claims | Resolved every local Markdown target and followed the support-matrix link | `docs/SUPPORT_MATRIX.md` resolves to this roadmap and all changed local links exist |
| Keep the macOS public client command buildable | Built a release arm64 executable and inspected its dynamic libraries | The binary links Homebrew's `libvulkan.1.dylib`; both debug and packaged-release title captures initialized Vulkan on Apple M3 Pro |
| Preserve menu and beacon behavior while fixing the quality gate | Ran every command in `client-pre-pr` manually because `just` is unavailable in the environment | Formatting, release Clippy with warnings denied, 40 protocol tests, 361 default client tests, and the singleplayer lifecycle test all passed |
| Exercise packaging and end-user paths | Built `target/release/pomme-client`, then ran it with the launch-token guard through title and integrated gameplay captures | The packaged title path wrote a valid 1,017,149-byte PNG; packaged gameplay completed login/configuration/world rendering, wrote a valid 1,066,896-byte PNG, saved 1,351 chunks, and shut down cleanly |
| Report rather than hide unavailable validation | Attempted only reproducible checked-in paths and inspected the acceptance prerequisites | Remote online-mode and multi-version server validation remains blocked by missing pinned fixtures, credentials, and runner, and is explicitly assigned to Wave 0 |

The release build reports a future-incompatibility warning from `winit 0.30.13`.
It does not fail the current build or Clippy gate, but dependency migration is a
concrete maintenance risk for the performance/platform cleanup wave.
