# Roadmap: PommeMC Client Gap Closure

## Overview

This roadmap closes all documented compatibility, gameplay, rendering, and server gaps in the Pomme client and its embedded SteelMC server across 8 milestone phases, progressing from low-level protocol compatibility up through survival gameplay and boss sequences.

## Phases

- [ ] **Phase 1: Protocol Translation Closure (Wave 2)** - Complete all remaining legacy packet translations (763–774) and split translation modules.
- [ ] **Phase 2: Authoritative State & Combat Parity (Wave 3)** - Centralize attribute-driven reach/speed and active item use overlays.
- [ ] **Phase 3: Fluid & World Rendering Parity (Wave 4A)** - Implement per-corner water height averaging, flow vectors, and depth sorting.
- [ ] **Phase 4: Entity Models, Layers & Presentation (Wave 4B)** - Add armor/saddle layers, sleeping pose, debug hitboxes, and living sound events.
- [ ] **Phase 5: Containers & Inventory Semantics (Wave 5)** - Complete specialized container screens, bed chat, and creative inventory interaction.
- [ ] **Phase 6: SteelMC Spawning & Tier 1 Hostiles (Wave 6A)** - Implement natural mob spawning cycles, chunk caps, and core hostile mob AI.
- [ ] **Phase 7: SteelMC Passives, Encounters & Bosses (Wave 6B)** - Add passive mob interactions, raids, Wither fight, and Ender Dragon fight.
- [ ] **Phase 8: Full-Matrix Acceptance Verification** - Execute automated test matrices across all supported versions and game modes.

## Phase Details

### Phase 1: Protocol Translation Closure (Wave 2)
**Goal**: Eliminate all dropped packets, lossy fallbacks, and suppressions across protocols 763–774, ensuring lossless bidirectional compatibility.
**Depends on**: Nothing (unblocks all downstream testing on older servers)
**Requirements**: TRANS-01, TRANS-02, TRANS-03, TRANS-04, TRANS-05, TRANS-06, TRANS-07, TRANS-08, TRANS-09, TRANS-10, TRANS-11, TRANS-12, TRANS-13
**Success Criteria**:
  1. Pre-1.21.4 servers receive valid slot-based `pick_item` packets instead of dropping player pick actions.
  2. Outbound creative slot updates on protocols 769–773 delimit component byte lengths according to 26.2 stream codec rules.
  3. Inbound legacy NBT items decode into modern 26.2 data components (enchantments, custom names, damage) across all containers.
  4. Complex particle packets with embedded block/item states transcode into valid 26.2 particle definitions.
  5. All 13 remaining translation markers in `acceptance/gap-ledger.json` transition to `closed` with test evidence.

### Phase 2: Authoritative State & Combat Parity (Wave 3)
**Goal**: Ensure all player movement attributes, interaction boundaries, and active item use timers derive authoritatively from server state.
**Depends on**: Phase 1
**Requirements**: STATE-01, STATE-02, STATE-03, STATE-04
**Success Criteria**:
  1. Field of view and movement speed dynamically adjust to `MOVEMENT_SPEED` modifiers (potions, beacons) without client-side drift.
  2. Main-hand item interaction attributes accurately gate block and entity raycasting distances.
  3. Bow draw and spyglass zoom overlays render with duration-driven scaling.

### Phase 3: Fluid & World Rendering Parity (Wave 4A)
**Goal**: Bring water and lava surfaces, flowing animations, and translucent face rendering to 100% vanilla visual parity.
**Depends on**: Phase 2
**Requirements**: RENDER-01, RENDER-02, RENDER-03
**Success Criteria**:
  1. Water and lava block vertices calculate per-corner smoothed heights based on adjacent fluid level states.
  2. Flowing fluids display animated directional flow vectors and tint with biome water colors.
  3. Translucent geometry (water surfaces, stained glass, ice) is depth-sorted relative to camera position to eliminate alpha popping.

### Phase 4: Entity Models, Layers & Presentation (Wave 4B)
**Goal**: Complete missing entity render layers, models, poses, audio events, and debug visualizers.
**Depends on**: Phase 3
**Requirements**: RENDER-04, RENDER-05, RENDER-06, RENDER-07, RENDER-08, RENDER-09, RENDER-10
**Success Criteria**:
  1. Copper chest variants and 26.2 block entity models render with proper oxidation states.
  2. Wolf armor, horse armor, and saddle equipment layers render attached to entity meshes.
  3. Sleeping and swimming poses orient player models horizontally with matching bed offsets and bounding box shifts.
  4. F3+B renders authoritative entity collision boxes and eye look vectors.
  5. Living entity hurt/death sound event 3 and boss music triggers play correctly.

### Phase 5: Containers & Inventory Semantics (Wave 5)
**Goal**: Provide fully interactive container screens for all vanilla blocks and restore missing player interaction screens.
**Depends on**: Phase 4
**Requirements**: INV-01, INV-02, INV-03, INV-04
**Success Criteria**:
  1. `InBedChatScreen` allows sleeping players to chat and wake up via UI button.
  2. Beacon, brewing stand, anvil, enchanting table, merchant, and crafter screens allow complete slot interactions and recipe selections.
  3. Creative inventory supports keyboard arrow navigation, tab focus rings, and full item drag/drop.

### Phase 6: SteelMC Spawning & Tier 1 Hostiles (Wave 6A)
**Goal**: Implement natural mob spawning algorithms and core hostile mob AI in the integrated singleplayer server.
**Depends on**: Phase 5
**Requirements**: STEEL-01, STEEL-02
**Success Criteria**:
  1. Natural spawn ticks evaluate chunk spawn rules, light level thresholds, and mob caps every tick.
  2. Zombies, skeletons, creepers, spiders, and endermen spawn naturally in darkness, target players, navigate terrain, and execute attacks.

### Phase 7: SteelMC Passives, Encounters & Bosses (Wave 6B)
**Goal**: Complete passive animal breeding/farming, village raids, and full multi-stage Wither and Ender Dragon boss fights.
**Depends on**: Phase 6
**Requirements**: STEEL-03, STEEL-04, STEEL-05, STEEL-06
**Success Criteria**:
  1. Animals breed with feed items, drop loot/XP on death, and support milking, shearing, and dyeing.
  2. Wither summoning initiates charge-up, flying skull attacks, invulnerability shield at 50% HP, and drops Nether Star.
  3. Ender Dragon executes crystal healing, breath attacks, fountain perching, and spawns exit portal and egg upon defeat.

### Phase 8: Full-Matrix Acceptance Verification
**Goal**: Run end-to-end automated scenarios across all supported protocol versions and verify zero open gap markers.
**Depends on**: Phase 7
**Requirements**: Complete closure of all 18 requirements in `acceptance/gap-ledger.json`
**Success Criteria**:
  1. Automated headless and graphical test suites pass on 1.20.1, 1.20.2, 1.20.4, 1.20.6, 1.21.1, 1.21.4, and 26.2.
  2. Gap ledger verifies 126/126 markers closed with concrete test traceability.
