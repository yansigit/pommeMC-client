# Summary: Plan 01-05 (Scoreboard Display Slots, Mount Equipment & Death Poof Particles)

**Status**: Complete
**Requirements**: TRANS-11, TRANS-12, TRANS-13
**Commits**: `1740f1f`, `dc2bc9c`

## Delivered

1. **Scoreboard Display Slots (`marker-459a3b42a0f9`)**:
   - Forwarded all `SetDisplayObjective` display slots (`Sidebar`, `List`, `BelowName`, `TeamBlack`..=`TeamWhite`) through `NetworkEvent::ScoreboardDisplay`.
   - Updated `Scoreboard` state in `ui/hud.rs` to track list, below-name, and team sidebars, with automatic slot cleanup on objective removal.
   - Added unit test `scoreboard_tracks_all_display_slots`.

2. **Mount Equipment Slot Tracking (`marker-0c0145529194`)**:
   - Extended `SetEquipment` packet handling to extract and emit `Body` slot items (horse armor, wolf armor) alongside `Saddle`.
   - Stored `body_armor` in `LivingEntity` state.
   - Added unit test `set_equipment_emits_body_armor`.

3. **Entity Death Poof Particles (`marker-2b775b36d3dd`)**:
   - Handled `EntityEvent` event 60 in `handler.rs` by emitting `NetworkEvent::EntityPoof`.
   - Implemented `ParticleStore::add_poof` spawning a 20-particle explosion scatter around the entity volume.
   - Added unit test `entity_event_60_emits_poof`.

## Verification
All 395 tests in `pomme-client` pass.
