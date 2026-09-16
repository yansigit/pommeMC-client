# Summary: Plan 04-03 (Hitbox Toggle & Drowned Model Verification)

**Status**: Complete
**Requirements**: RENDER-08, RENDER-09

## Delivered

1. **F3+B Hitbox Toggle (`marker-14c1acc12dea`)**:
   - Implemented F3+B chord handling in `handle_debug_key` to toggle `show_hitboxes` and emit on-screen debug feedback.

2. **Drowned Model Layer (`marker-9ca1dd80f7c6`)**:
   - Verified `bake_drowned_model` applies independent left arm/leg UV regions and 0.25 clothing inflation.
   - Cleaned up stale marker in `entity_model.rs`.

## Verification
All 400 tests in `pomme-client` pass.
