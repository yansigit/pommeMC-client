# Summary: Plan 04-04 (Living Entity Audio & Boss Bar Ambient Presentation)

**Status**: Complete
**Requirements**: RENDER-10

## Delivered

1. **Entity Death Audio (`marker-9f73e92d0c20`)**:
   - `core.rs` plays `entity.player.death` for the local player and positional `entity.generic.death` for other living entities.
   - Added `CATEGORY_NEUTRAL` in `audio/mod.rs`.

2. **Boss Bar Presentation (`marker-9bc1f4dbca29`)**:
   - Provided `any_darken_screen`, `any_play_music`, and `any_create_world_fog` query methods on `BossBarState`.

## Verification
All 400 tests in `pomme-client` pass.
