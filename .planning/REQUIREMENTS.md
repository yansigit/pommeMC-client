# Requirements: PommeMC Client Gap Closure

**Status**: Active
**Tracking Source**: `acceptance/gap-ledger.json` and `docs/GAP_ROADMAP.md`

## Protocol Compatibility & Translation (Wave 2)

- [ ] **TRANS-01 (marker-526126612cd9)**: Implement pre-1.21.4 slot-based `pick_item` serverbound translation rather than quiet suppression.
- [ ] **TRANS-02 (marker-d6046eb03b41)**: Length-delimit pre-1.21.5 `set_creative_mode_slot` item component values.
- [ ] **TRANS-03 (marker-60faae95e31b)**: Retain prior `dimension_type` registry across incomplete configuration-phase reconfigurations.
- [ ] **TRANS-04 (marker-e417df24e455)**: Translate legacy item cooldown packets to modern cooldown groups when received on wire.
- [ ] **TRANS-05 (marker-edbd1ef1eaf8)**: Convert legacy item stack NBT (enchantments, display name, lore, damage) to modern 26.2 data components.
- [ ] **TRANS-06 (marker-dd2693cf179b)**: Translate pre-1.21.9 shoulder-parrot entity metadata compound NBT into modern OptionalInt variants.
- [ ] **TRANS-07 (marker-4759179667d0)**: Lift pre-1.21.6 hanging-entity metadata indices (item frame item/rotation and painting variant).
- [ ] **TRANS-08 (marker-e5f584498273)**: Rewrite complex particle payloads (`block`, `block_marker`, `falling_dust`, `dust_color_transition`, `item`).
- [ ] **TRANS-09 (marker-406fd7868806)**: Preserve real dimension keys on login/respawn packets when older packets omit modern fields.
- [ ] **TRANS-10 (marker-fbd3f0cfa68f)**: Encode legacy team colors into modern `Optional<TeamColor>` without dropping formatting.
- [ ] **TRANS-11 (marker-459a3b42a0f9)**: Track all scoreboard objective display slots (sidebar, list, below-name, team-specific).
- [ ] **TRANS-12 (marker-0c0145529194)**: Parse and retain mount equipment slot states and render equipment layers.
- [ ] **TRANS-13 (marker-2b775b36d3dd)**: Dispatch event 60 (`makePoofParticles`) when a mob entity death clock reaches 20 ticks.

## Authoritative State, Movement & Combat (Wave 3)

- [ ] **STATE-01 (marker-818a7735a864)**: Drive player field-of-view and movement speed dynamically from authoritative `MOVEMENT_SPEED` attributes.
- [ ] **STATE-02 (marker-9c116d39f412, marker-9a9b1408ef1f)**: Derive combat reach and entity interaction range from main-hand `ENTITY_INTERACTION_RANGE` and `BLOCK_INTERACTION_RANGE` attributes.
- [ ] **STATE-03 (marker-2116124f6945)**: Render progressive bow-draw narrowing and spyglass overlay scoping tied to active item use duration.
- [ ] **STATE-04 (marker-1149231da7e7)**: Display mount dash cooldown overlay when riding camels or nautilus mounts.

## World, Fluids, Models & Presentation (Wave 4)

- [ ] **RENDER-01 (marker-2483e2e23596, marker-dc572b7e0008)**: Compute per-corner height averaging and fluid levels for realistic water and lava surfaces.
- [ ] **RENDER-02 (marker-c509d8472536, marker-19a8ae089327)**: Implement animated flowing water textures with flow vectors and biome-blended water colors.
- [ ] **RENDER-03 (marker-76c43ee5d9c8)**: Implement depth sorting for overlapping translucent water and stained glass faces.
- [ ] **RENDER-04 (marker-c20a624e6f6c)**: Add model and texture definitions for 26.2 copper chest variants (exposed, weathered, oxidized).
- [ ] **RENDER-05 (marker-8aaefa04269d, marker-9ed474e5786f, marker-b14fcb593fd4)**: Render dynamic equipment layers for saddles, horse armor, and wolf armor.
- [ ] **RENDER-06 (marker-741d89c2b129, marker-3c7e58b51bc4)**: Render player skin outer jacket/sleeve/pants layers and custom worn head blocks.
- [ ] **RENDER-07 (marker-a9f9703d8ecc, marker-885632399f62)**: Render remote players in sleeping pose and correct vertical offset on beds.
- [ ] **RENDER-08 (marker-9ca1dd80f7c6)**: Implement swimming body pitch and limb animations based on player swim amount.
- [ ] **RENDER-09 (marker-14c1acc12dea)**: Implement F3+B debug hitbox and eye-line bounding box rendering.
- [ ] **RENDER-10 (marker-9f73e92d0c20, marker-9bc1f4dbca29)**: Support living entity sound event 3 and boss fight environmental audio (music, screen darkening).

## Container Inventories & Survival Semantics (Wave 5)

- [ ] **INV-01 (marker-d8fd8e2b1503)**: Implement `InBedChatScreen` with Leave Bed button and chat input while sleeping.
- [ ] **INV-02 (marker-87a7172f3637, marker-e202d4bc094a)**: Complete creative inventory tab focus rings, arrow-key navigation, and form button focus rings.
- [ ] **INV-03**: Complete specialized container screen interactions (beacon, brewing stand, anvil, enchanting, merchant, crafter).
- [ ] **INV-04**: Sync player inventory crafting grid state and client-side recipe book unlock notifications.

## SteelMC Integrated Server Foundations, Mobs & Bosses (Wave 6)

- [ ] **STEEL-01**: Implement natural mob spawning engine (chunk mob caps, spawn ticks, light rules, biome tags).
- [ ] **STEEL-02**: Tier 1 hostile mob behaviors (zombie pathfinding/reinforcements, skeleton archery, creeper priming, spider climbing, enderman teleportation).
- [ ] **STEEL-03**: Tier 2 passive and neutral mob behaviors (breeding, drops, sheep dyeing/shearing, cow milking, villager trade leveling).
- [ ] **STEEL-04**: Tier 3 structured encounters (witch huts, pillager patrols, raid waves, ocean monument guardians).
- [ ] **STEEL-05**: Wither boss sequence (three-head targeting, invulnerability charge, projectile skulls, wither armor at 50% health, nether star drop).
- [ ] **STEEL-06**: Ender Dragon boss sequence (end crystal healing, strafing flight, dragon breath fireballs, perched fountain breath, exit portal activation).
