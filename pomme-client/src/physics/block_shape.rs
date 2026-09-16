//! Generated per-block-state collision and outline shapes.
//!
//! Geometry is generated from the pinned Azalea 26.2 shape table by
//! `tools/blockgen`. Boxes are block-local (0..1); callers add the block
//! position after vanilla's deterministic per-position shape offset.

use std::borrow::Cow;

use azalea_block::BlockState;
use azalea_core::math::get_seed;
use azalea_core::position::BlockPos;

/// A block-local axis-aligned box: `[min_x, min_y, min_z, max_x, max_y,
/// max_z]`.
pub type LocalBox = [f64; 6];

const FULL_CUBE_SHAPE: &[LocalBox] = &[[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]];

/// Collision boxes for `state`: `None` for a full cube, `Some(&[])` for no
/// collision, and `Some(boxes)` for a partial shape.
pub fn partial_shape(state: BlockState, pos: BlockPos) -> Option<Cow<'static, [LocalBox]>> {
    crate::world::block::block_shape(state).map(|shape| apply_shape_offset(state, pos, shape))
}

/// Boxes the interaction raycast clips against (vanilla `getShape`). An empty
/// slice means the block is not targetable.
pub fn outline_shape(state: BlockState, pos: BlockPos) -> Cow<'static, [LocalBox]> {
    let shape = crate::world::block::block_outline(state).unwrap_or(FULL_CUBE_SHAPE);
    apply_shape_offset(state, pos, shape)
}

fn apply_shape_offset(
    state: BlockState,
    pos: BlockPos,
    shape: &'static [LocalBox],
) -> Cow<'static, [LocalBox]> {
    let offset_kind = crate::world::block::block_shape_offset(state);
    if shape.is_empty() || offset_kind == 0 {
        return Cow::Borrowed(shape);
    }

    let id = crate::world::block::block_id(state);
    let max_horizontal = if matches!(id, "pointed_dripstone" | "sulfur_spike") {
        2.0 / 16.0
    } else {
        0.25
    };
    let max_vertical = if id == "small_dripleaf" { 0.1 } else { 0.2 };
    let offset_pos = if offset_kind == 1 { pos.with_y(0) } else { pos };
    let seed = get_seed(offset_pos);
    let y = if offset_kind == 2 {
        ((((seed >> 4) & 15) as f32 / 15.0) as f64 - 1.0) * max_vertical
    } else {
        0.0
    };
    let x =
        ((((seed & 15) as f32 / 15.0) as f64 - 0.5) * 0.5).clamp(-max_horizontal, max_horizontal);
    let z = (((((seed >> 8) & 15) as f32 / 15.0) as f64 - 0.5) * 0.5)
        .clamp(-max_horizontal, max_horizontal);

    Cow::Owned(
        shape
            .iter()
            .map(|b| [b[0] + x, b[1] + y, b[2] + z, b[3] + x, b[4] + y, b[5] + z])
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::block::{block_id, block_properties, try_state};

    fn state(name: &str, wanted: &[(&str, &str)]) -> BlockState {
        crate::world::block::init("26.2");
        (0..)
            .map_while(try_state)
            .find(|state| {
                block_id(*state) == name
                    && wanted
                        .iter()
                        .all(|&(key, value)| block_properties(*state).get(key) == Some(value))
            })
            .unwrap_or_else(|| panic!("missing state {name} {wanted:?}"))
    }

    #[test]
    fn generated_shapes_cover_full_empty_and_partial_states() {
        let stone = state("stone", &[]);
        assert!(partial_shape(stone, BlockPos::ZERO).is_none());

        let air = state("air", &[]);
        assert!(partial_shape(air, BlockPos::ZERO).unwrap().is_empty());

        let slab = state("oak_slab", &[("type", "bottom"), ("waterlogged", "false")]);
        assert_eq!(
            partial_shape(slab, BlockPos::ZERO).unwrap().as_ref(),
            [[0.0, 0.0, 0.0, 1.0, 0.5, 1.0]]
        );
    }

    #[test]
    fn generated_outline_can_differ_from_collision() {
        let torch = state("torch", &[]);
        assert!(partial_shape(torch, BlockPos::ZERO).unwrap().is_empty());
        assert_eq!(
            outline_shape(torch, BlockPos::ZERO).as_ref(),
            [[0.375, 0.0, 0.375, 0.625, 0.625, 0.625]]
        );
    }

    #[test]
    fn random_shape_offset_is_deterministic_and_xz_only_for_flowers() {
        let flower = state("dandelion", &[]);
        let pos = BlockPos::new(7, 12, -3);
        let first = outline_shape(flower, pos);
        let second = outline_shape(flower, pos);
        assert_eq!(first, second);
        assert_ne!(
            first.as_ref(),
            [[0.3125, 0.0, 0.3125, 0.6875, 0.625, 0.6875]]
        );
        assert_eq!(first[0][1], 0.0);
        assert_eq!(first[0][4], 0.625);
    }
}
