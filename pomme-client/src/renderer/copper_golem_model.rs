//! Copper golem statue models from vanilla 26.2 `CopperGolemModel` pose layers.
//!
//! Vanilla renders these particle-only block models through
//! `CopperGolemStatueBlockRenderer`. The statue model resets its root y to 0
//! and rotates it by PI around Z, so the block-entity y-down convention omits
//! the normal living-entity ground-plane rebase.

// These values are copied from Mojang's generated model layer as f32 literals.
// Preserve them verbatim so future comparisons against the vanilla export stay
// mechanical rather than normalizing each constant to Clippy's preferred form.
#![allow(clippy::excessive_precision)]

use glam::Vec3;

use super::entity_model::{BakedEntityModel, EntityPart, ModelConvention, ModelCube, bake_model};

fn cube(origin: [f32; 3], size: [f32; 3], tex_offset: (i32, i32), deformation: f32) -> ModelCube {
    ModelCube {
        origin: Vec3::from_array(origin),
        size: Vec3::from_array(size),
        tex_offset,
        deformation,
        mirror: false,
    }
}

fn part(
    name: &'static str,
    offset: [f32; 3],
    rotation: [f32; 3],
    parent: Option<usize>,
    cubes: Vec<ModelCube>,
) -> EntityPart {
    EntityPart {
        name: name.into(),
        offset: Vec3::from_array(offset),
        default_rotation: Vec3::from_array(rotation),
        cubes,
        parent,
    }
}

fn finish(parts: Vec<EntityPart>) -> BakedEntityModel {
    bake_model(parts, 64, 64).with_convention(ModelConvention::BlockEntityYDown)
}

fn bake_standing() -> BakedEntityModel {
    finish(vec![
        part(
            "body",
            [0.0, -5.0, 0.0],
            [0.0, 0.0, 0.0],
            None,
            vec![cube([-4.0, -6.0, -3.0], [8.0, 6.0, 6.0], (0, 15), 0.0)],
        ),
        part(
            "head",
            [0.0, -6.0, 0.0],
            [0.0, 0.0, 0.0],
            Some(0),
            vec![
                cube([-4.0, -5.0, -5.0], [8.0, 5.0, 10.0], (0, 0), 0.0149999997),
                cube([-1.0, -2.0, -6.0], [2.0, 3.0, 2.0], (56, 0), 0.0),
                cube([-1.0, -9.0, -1.0], [2.0, 4.0, 2.0], (37, 8), -0.0149999997),
                cube([-2.0, -13.0, -2.0], [4.0, 4.0, 4.0], (37, 0), -0.0149999997),
            ],
        ),
        part(
            "right_arm",
            [-4.0, -6.0, 0.0],
            [0.0, 0.0, 0.0],
            Some(0),
            vec![cube([-3.0, -1.0, -2.0], [3.0, 10.0, 4.0], (36, 16), 0.0)],
        ),
        part(
            "left_arm",
            [4.0, -6.0, 0.0],
            [0.0, 0.0, 0.0],
            Some(0),
            vec![cube([0.0, -1.0, -2.0], [3.0, 10.0, 4.0], (50, 16), 0.0)],
        ),
        part(
            "right_leg",
            [0.0, -5.0, 0.0],
            [0.0, 0.0, 0.0],
            None,
            vec![cube([-4.0, 0.0, -2.0], [4.0, 5.0, 4.0], (0, 27), 0.0)],
        ),
        part(
            "left_leg",
            [0.0, -5.0, 0.0],
            [0.0, 0.0, 0.0],
            None,
            vec![cube([0.0, 0.0, -2.0], [4.0, 5.0, 4.0], (16, 27), 0.0)],
        ),
    ])
}

fn bake_running() -> BakedEntityModel {
    finish(vec![
        part(
            "body",
            [-1.06400001, -5.0, 0.0],
            [0.0, 0.0, 0.0],
            None,
            vec![],
        ),
        part(
            "body_r1",
            [1.10000002, 0.100000001, 0.699999988],
            [0.120399997, -0.00639999984, -0.0778999999],
            Some(0),
            vec![cube(
                [-4.01999998, -6.11600018, -3.5],
                [8.0, 6.0, 6.0],
                (0, 15),
                0.0,
            )],
        ),
        part(
            "head",
            [0.699999988, -5.5999999, -1.79999995],
            [0.0, 0.0, 0.0],
            Some(0),
            vec![
                cube([-4.0, -5.0999999, -5.0], [8.0, 5.0, 10.0], (0, 0), 0.0),
                cube(
                    [-1.01999998, -2.0999999, -6.0],
                    [2.0, 3.0, 2.0],
                    (56, 0),
                    0.0,
                ),
                cube(
                    [-1.01999998, -9.10000038, -1.0],
                    [2.0, 4.0, 2.0],
                    (37, 8),
                    -0.0149999997,
                ),
                cube(
                    [-2.0, -13.1000004, -2.0],
                    [4.0, 4.0, 4.0],
                    (37, 0),
                    -0.0149999997,
                ),
            ],
        ),
        part(
            "right_arm",
            [-4.0, -6.0, 0.0],
            [0.0, 0.0, 0.0],
            Some(0),
            vec![],
        ),
        part(
            "right_arm_r1",
            [0.699999988, -0.247999996, -1.62],
            [1.0036, 0.0, 0.0],
            Some(3),
            vec![cube(
                [-3.05200005, -1.11000001, -2.03600001],
                [3.0, 10.0, 4.0],
                (36, 16),
                0.0,
            )],
        ),
        part(
            "left_arm",
            [4.0, -6.0, 0.0],
            [0.0, 0.0, 0.0],
            Some(0),
            vec![],
        ),
        part(
            "left_arm_r1",
            [0.731999993, 0.0, 0.0],
            [-0.871500015, -0.0535000004, -0.0449000001],
            Some(5),
            vec![cube(
                [0.0320000015, -1.10000002, -2.0],
                [3.0, 10.0, 4.0],
                (50, 16),
                0.0,
            )],
        ),
        part(
            "right_leg",
            [-3.06399989, -5.0, 0.0],
            [0.0, 0.0, 0.0],
            None,
            vec![],
        ),
        part(
            "right_leg_r1",
            [1.04799998, 0.0, -0.899999976],
            [-0.872699976, 0.0, 0.0],
            Some(7),
            vec![cube(
                [-1.85599995, -0.100000001, -1.09000003],
                [4.0, 5.0, 4.0],
                (0, 27),
                0.0,
            )],
        ),
        part(
            "left_leg",
            [0.93599999, -5.0, 0.0],
            [0.0, 0.0, 0.0],
            None,
            vec![],
        ),
        part(
            "left_leg_r1",
            [1.0, 0.0, 0.0],
            [0.785399973, 0.0, 0.0],
            Some(9),
            vec![cube(
                [-2.08800006, -0.100000001, -2.0],
                [4.0, 5.0, 4.0],
                (16, 27),
                0.0,
            )],
        ),
    ])
}

fn bake_sitting() -> BakedEntityModel {
    finish(vec![
        part(
            "body",
            [0.0, -3.0, 2.32500005],
            [0.0, 0.0, 0.0],
            None,
            vec![
                cube([-3.0, -4.0, -4.5250001], [6.0, 1.0, 6.0], (3, 19), 0.0),
                cube([-4.0, -3.0, -3.5250001], [8.0, 6.0, 6.0], (0, 15), 0.0),
            ],
        ),
        part(
            "body_r1",
            [0.0, -1.0, -4.32499981],
            [0.0, 0.0, -3.14159989],
            Some(0),
            vec![cube(
                [-4.0, -3.0, -2.20000005],
                [8.0, 6.0, 3.0],
                (3, 18),
                0.0,
            )],
        ),
        part(
            "head",
            [0.0, -6.0, -0.200000003],
            [0.0, 0.0, 0.0],
            Some(0),
            vec![
                cube(
                    [-1.0, -7.0, -3.29999995],
                    [2.0, 4.0, 2.0],
                    (37, 8),
                    -0.0149999997,
                ),
                cube(
                    [-2.0, -11.0, -4.30000019],
                    [4.0, 4.0, 4.0],
                    (37, 0),
                    -0.0149999997,
                ),
                cube([-4.0, -3.0, -7.32499981], [8.0, 5.0, 10.0], (0, 0), 0.0),
                cube([-1.0, 0.0, -8.32499981], [2.0, 3.0, 2.0], (56, 0), 0.0),
            ],
        ),
        part(
            "right_arm",
            [-4.0, -5.5999999, -1.79999995],
            [0.436300009, 0.0, 0.0],
            Some(0),
            vec![],
        ),
        part(
            "right_arm_r1",
            [0.0, 0.0892999992, 0.119800001],
            [-1.04719996, 0.0, 0.0],
            Some(3),
            vec![cube(
                [-3.07500005, -0.97329998, -1.99660003],
                [3.0, 10.0, 4.0],
                (36, 16),
                0.0,
            )],
        ),
        part(
            "left_arm",
            [4.0, -5.5999999, -1.70000005],
            [0.436300009, 0.0, 0.0],
            Some(0),
            vec![],
        ),
        part(
            "left_arm_r1",
            [0.0, -0.00150000001, -0.0807999969],
            [-1.04719996, 0.0, 0.0],
            Some(5),
            vec![cube(
                [0.075000003, -1.04429996, -1.89970005],
                [3.0, 10.0, 4.0],
                (50, 16),
                0.0,
            )],
        ),
        part(
            "right_leg",
            [-2.0999999, -2.0999999, -2.07500005],
            [0.0, 0.0, 0.0],
            None,
            vec![],
        ),
        part(
            "right_leg_r1",
            [0.0500000007, -1.89999998, 1.07500005],
            [-1.57079995, 0.0, 0.0],
            Some(7),
            vec![cube(
                [-2.0, 0.975000024, 0.0],
                [4.0, 5.0, 4.0],
                (0, 27),
                0.0,
            )],
        ),
        part(
            "left_leg",
            [2.0, -2.0, -2.07500005],
            [0.0, 0.0, 0.0],
            None,
            vec![],
        ),
        part(
            "left_leg_r1",
            [0.0500000007, -2.0, 1.07500005],
            [-1.57079995, 0.0, 0.0],
            Some(9),
            vec![cube(
                [-2.0, 0.975000024, 0.0],
                [4.0, 5.0, 4.0],
                (16, 27),
                0.0,
            )],
        ),
    ])
}

fn bake_star() -> BakedEntityModel {
    finish(vec![
        part(
            "body",
            [0.0, -5.0, 0.0],
            [0.0, 0.0, 0.0],
            None,
            vec![cube([-4.0, -6.0, -3.0], [8.0, 6.0, 6.0], (0, 15), 0.0)],
        ),
        part(
            "head",
            [0.0, -6.0, 0.0],
            [0.0, 0.0, 0.0],
            Some(0),
            vec![
                cube([-4.0, -5.0, -5.0], [8.0, 5.0, 10.0], (0, 0), 0.0),
                cube([-1.0, -2.0, -6.0], [2.0, 3.0, 2.0], (56, 0), 0.0),
                cube([-1.0, -9.0, -1.0], [2.0, 4.0, 2.0], (37, 8), -0.0149999997),
                cube([-2.0, -13.0, -2.0], [4.0, 4.0, 4.0], (37, 0), -0.0149999997),
            ],
        ),
        part(
            "right_arm",
            [-4.0, -6.0, 0.0],
            [0.0, 0.0, 0.0],
            Some(0),
            vec![],
        ),
        part(
            "right_arm_r1",
            [1.0, 1.0, 0.0],
            [0.0, 0.0, 1.91989994],
            Some(2),
            vec![cube([-1.5, -5.0, -2.0], [3.0, 10.0, 4.0], (36, 16), 0.0)],
        ),
        part(
            "rightItem",
            [-1.0, 7.4000001, -1.0],
            [0.0, 0.0, 0.0],
            Some(2),
            vec![],
        ),
        part(
            "left_arm",
            [4.0, -6.0, 0.0],
            [0.0, 0.0, 0.0],
            Some(0),
            vec![],
        ),
        part(
            "left_arm_r1",
            [-1.0, 1.0, 0.0],
            [0.0, 0.0, -1.91989994],
            Some(5),
            vec![cube([-1.5, -5.0, -2.0], [3.0, 10.0, 4.0], (50, 16), 0.0)],
        ),
        part(
            "right_leg",
            [-3.0, -5.0, 0.0],
            [0.0, 0.0, 0.0],
            None,
            vec![],
        ),
        part(
            "right_leg_r1",
            [0.349999994, 2.0, 0.00999999978],
            [0.0, 0.0, 0.261799991],
            Some(7),
            vec![cube([-2.0, -2.5, -2.0], [4.0, 5.0, 4.0], (0, 27), 0.0)],
        ),
        part("left_leg", [1.0, -5.0, 0.0], [0.0, 0.0, 0.0], None, vec![]),
        part(
            "left_leg_r1",
            [1.64999998, 2.0, 0.0],
            [0.0, 0.0, -0.261799991],
            Some(9),
            vec![cube([-2.0, -2.5, -2.0], [4.0, 5.0, 4.0], (16, 27), 0.0)],
        ),
    ])
}

/// Pose order used by `variant_for_block`: standing, running, sitting, star.
pub fn bake_models() -> Vec<BakedEntityModel> {
    vec![bake_standing(), bake_running(), bake_sitting(), bake_star()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::entity_model::PartAnim;

    #[test]
    fn statue_pose_layers_match_vanilla_shape_counts() {
        let models = bake_models();
        assert_eq!(models.len(), 4);
        assert_eq!(
            models
                .iter()
                .map(|model| model.parts.len())
                .collect::<Vec<_>>(),
            [6, 11, 11, 11]
        );
        assert_eq!(
            models
                .iter()
                .map(|model| model.vertices.len())
                .collect::<Vec<_>>(),
            [9 * 36, 9 * 36, 11 * 36, 9 * 36]
        );
        assert!(
            models
                .iter()
                .all(|model| matches!(model.convention, ModelConvention::BlockEntityYDown))
        );
    }

    #[test]
    fn statue_roots_skip_the_living_entity_ground_rebase() {
        let standing = bake_standing();
        let transforms = standing.compute_part_transforms(&PartAnim::default());
        let body_origin = transforms[0].transform_point3(Vec3::ZERO);
        assert!((body_origin.y - 5.0 / 16.0).abs() < 1.0e-6);
    }
}
