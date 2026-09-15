//! Per-block-state collision shapes for the handful of blocks whose hitbox
//! isn't a full cube. Ported from the vanilla block classes (`SlabBlock`,
//! `StairBlock`, etc.). Boxes are block-local (0..1); the caller offsets them
//! to the block position.
use azalea_block::BlockState;

use crate::world::block::PropMap;

/// A block-local axis-aligned box: `[min_x, min_y, min_z, max_x, max_y,
/// max_z]`.
pub type LocalBox = [f64; 6];

/// Cached collision boxes for `state`: `None` for a full cube, `Some(&[])` for
/// no collision, `Some(boxes)` for a partial shape.
pub fn partial_shape(state: BlockState) -> Option<&'static [LocalBox]> {
    crate::world::block::block_shape(state)
}

const FULL_CUBE_SHAPE: &[LocalBox] = &[[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]];

/// Boxes the interaction raycast clips against (vanilla `getShape`). Unlike
/// `partial_shape` the full-cube case is already resolved, so an empty slice
/// means "not targetable" rather than "no collision".
pub fn outline_shape(state: BlockState) -> &'static [LocalBox] {
    crate::world::block::block_outline(state).unwrap_or(FULL_CUBE_SHAPE)
}

/// Computes one state's shape. Takes id/props rather than a `BlockState` so
/// the block-table build can call it without re-entering the table.
pub(crate) fn compute_shape(id: &str, props: &PropMap) -> Option<Vec<LocalBox>> {
    if id.ends_with("_slab") {
        return Some(match props.get("type") {
            Some("top") => vec![[0.0, 0.5, 0.0, 1.0, 1.0, 1.0]],
            Some("double") => return None,             // full cube
            _ => vec![[0.0, 0.0, 0.0, 1.0, 0.5, 1.0]], // bottom
        });
    }

    if id.ends_with("_stairs") {
        return Some(stair_boxes(
            props.get("half").unwrap_or("bottom"),
            props.get("facing").unwrap_or("north"),
            props.get("shape").unwrap_or("straight"),
        ));
    }

    if id.ends_with("_trapdoor") {
        return Some(trapdoor_box(
            props.get("half").unwrap_or("bottom"),
            props.get("facing").unwrap_or("north"),
            props.get("open") == Some("true"),
        ));
    }

    if id.ends_with("_door") {
        return Some(door_box(
            props.get("half").unwrap_or("lower"),
            props.get("facing").unwrap_or("north"),
            props.get("hinge").unwrap_or("left"),
            props.get("open") == Some("true"),
        ));
    }

    if id.ends_with("_fence_gate") {
        return Some(fence_gate_boxes(
            props.get("facing").unwrap_or("north"),
            props.get("open") == Some("true"),
            props.get("in_wall") == Some("true"),
        ));
    }

    if id.ends_with("_fence") {
        return Some(fence_boxes(props));
    }

    if id.ends_with("_wall") {
        return Some(wall_boxes(props));
    }

    if id == "iron_bars" || id.ends_with("_glass_pane") {
        return Some(pane_boxes(props));
    }

    if id.ends_with("_bed") {
        return Some(bed_box(
            props.get("part").unwrap_or("foot"),
            props.get("facing").unwrap_or("north"),
        ));
    }

    if id == "cake" {
        let bites = props
            .get("bites")
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0)
            .min(6);
        let min_x = 1.0 / 16.0 + bites as f64 * 2.0 / 16.0;
        return Some(vec![[
            min_x,
            0.0,
            1.0 / 16.0,
            15.0 / 16.0,
            7.0 / 16.0,
            15.0 / 16.0,
        ]]);
    }

    if id.ends_with("_button") || id == "stone_button" || id == "polished_blackstone_button" {
        return Some(button_box(
            props.get("face").unwrap_or("wall"),
            props.get("facing").unwrap_or("north"),
            props.get("powered") == Some("true"),
        ));
    }

    if id == "lever" {
        return Some(lever_box(
            props.get("face").unwrap_or("wall"),
            props.get("facing").unwrap_or("north"),
        ));
    }

    match id {
        "dirt_path" | "farmland" => Some(vec![[0.0, 0.0, 0.0, 1.0, 0.9375, 1.0]]),
        _ if id.ends_with("_carpet") => Some(vec![[0.0, 0.0, 0.0, 1.0, 0.0625, 1.0]]),
        "cactus" => Some(vec![[
            1.0 / 16.0,
            0.0,
            1.0 / 16.0,
            15.0 / 16.0,
            15.0 / 16.0,
            15.0 / 16.0,
        ]]),
        "flower_pot" | "potted_cactus" => Some(vec![[
            2.0 / 16.0,
            0.0,
            2.0 / 16.0,
            14.0 / 16.0,
            6.0 / 16.0,
            14.0 / 16.0,
        ]]),
        // `SnowLayerBlock.getCollisionShape` is one layer shorter than its
        // outline, so a single layer has no collision at all.
        "snow" => Some(snow_shape(snow_layers(props) - 1)),
        _ => None,
    }
}

/// Vanilla `getShape` where it differs from `getCollisionShape`. `None` means
/// the two agree, so `compute_shape`'s result doubles as the outline.
pub(crate) fn compute_outline(id: &str, props: &PropMap) -> Option<Vec<LocalBox>> {
    match id {
        "snow" => Some(snow_shape(snow_layers(props))),
        // `LiquidBlock.getShape` and `BubbleColumnBlock.getShape` are
        // `Shapes.empty()`: the pick ray clips straight through them.
        "water" | "lava" | "bubble_column" => Some(Vec::new()),
        "torch" | "soul_torch" | "copper_torch" | "redstone_torch" => Some(vec![[
            5.0 / 16.0,
            0.0,
            5.0 / 16.0,
            11.0 / 16.0,
            10.0 / 16.0,
            11.0 / 16.0,
        ]]),
        "flower_pot" | "potted_cactus" => Some(vec![[
            2.0 / 16.0,
            0.0,
            2.0 / 16.0,
            14.0 / 16.0,
            6.0 / 16.0,
            14.0 / 16.0,
        ]]),
        _ if is_small_plant(id) => Some(vec![[
            3.0 / 16.0,
            0.0,
            3.0 / 16.0,
            13.0 / 16.0,
            1.0,
            13.0 / 16.0,
        ]]),
        _ => None,
    }
}

fn is_small_plant(id: &str) -> bool {
    matches!(
        id,
        "dandelion"
            | "poppy"
            | "blue_orchid"
            | "allium"
            | "azure_bluet"
            | "red_tulip"
            | "orange_tulip"
            | "white_tulip"
            | "pink_tulip"
            | "oxeye_daisy"
            | "cornflower"
            | "lily_of_the_valley"
            | "wither_rose"
            | "torchflower"
            | "short_grass"
            | "fern"
            | "dead_bush"
            | "seagrass"
            | "sweet_berry_bush"
    )
}

fn facing_box(facing: &str, thickness: f64, y0: f64, y1: f64) -> LocalBox {
    match facing {
        "south" => [0.0, y0, 1.0 - thickness, 1.0, y1, 1.0],
        "west" => [0.0, y0, 0.0, thickness, y1, 1.0],
        "east" => [1.0 - thickness, y0, 0.0, 1.0, y1, 1.0],
        _ => [0.0, y0, 0.0, 1.0, y1, thickness],
    }
}

fn trapdoor_box(half: &str, facing: &str, open: bool) -> Vec<LocalBox> {
    if open {
        return vec![facing_box(facing, 3.0 / 16.0, 0.0, 1.0)];
    }
    let (y0, y1) = if half == "top" {
        (13.0 / 16.0, 1.0)
    } else {
        (0.0, 3.0 / 16.0)
    };
    vec![[0.0, y0, 0.0, 1.0, y1, 1.0]]
}

fn door_box(half: &str, facing: &str, hinge: &str, open: bool) -> Vec<LocalBox> {
    let thickness = 3.0 / 16.0;
    let box_ = if !open {
        facing_box(facing, thickness, 0.0, 1.0)
    } else {
        let clockwise = matches!(
            (facing, hinge),
            ("north", "left") | ("south", "right") | ("east", "left") | ("west", "right")
        );
        let open_facing = if clockwise { cw(facing) } else { ccw(facing) };
        facing_box(open_facing, thickness, 0.0, 1.0)
    };
    let _ = half;
    vec![box_]
}

fn fence_boxes(props: &PropMap) -> Vec<LocalBox> {
    let mut boxes = vec![[6.0 / 16.0, 0.0, 6.0 / 16.0, 10.0 / 16.0, 1.5, 10.0 / 16.0]];
    for property in ["north", "south", "west", "east"] {
        if props.get(property) == Some("true") {
            boxes.push(match property {
                "north" => [6.0 / 16.0, 6.0 / 16.0, 0.0, 10.0 / 16.0, 1.5, 10.0 / 16.0],
                "south" => [6.0 / 16.0, 6.0 / 16.0, 6.0 / 16.0, 10.0 / 16.0, 1.5, 1.0],
                "west" => [0.0, 6.0 / 16.0, 6.0 / 16.0, 10.0 / 16.0, 1.5, 10.0 / 16.0],
                _ => [6.0 / 16.0, 6.0 / 16.0, 6.0 / 16.0, 1.0, 1.5, 10.0 / 16.0],
            });
        }
    }
    boxes
}

fn wall_boxes(props: &PropMap) -> Vec<LocalBox> {
    let mut boxes = Vec::new();
    if props.get("up") == Some("true") {
        boxes.push([7.0 / 16.0, 0.0, 7.0 / 16.0, 9.0 / 16.0, 1.5, 9.0 / 16.0]);
    }
    for (property, axis) in [
        ("north", true),
        ("south", true),
        ("west", false),
        ("east", false),
    ] {
        let Some(height) = props.get(property).and_then(wall_height) else {
            continue;
        };
        if axis {
            let z0 = if property == "north" { 0.0 } else { 7.0 / 16.0 };
            boxes.push([7.0 / 16.0, 0.0, z0, 9.0 / 16.0, height, z0 + 9.0 / 16.0]);
        } else {
            let x0 = if property == "west" { 0.0 } else { 7.0 / 16.0 };
            boxes.push([x0, 0.0, 7.0 / 16.0, x0 + 9.0 / 16.0, height, 9.0 / 16.0]);
        }
    }
    boxes
}

fn wall_height(value: &str) -> Option<f64> {
    match value {
        "low" => Some(1.0),
        "tall" => Some(1.5),
        _ => None,
    }
}

fn pane_boxes(props: &PropMap) -> Vec<LocalBox> {
    let mut boxes = Vec::new();
    let post = [7.0 / 16.0, 0.0, 7.0 / 16.0, 9.0 / 16.0, 1.0, 9.0 / 16.0];
    boxes.push(post);
    for (property, box_) in [
        ("north", [7.0 / 16.0, 0.0, 0.0, 9.0 / 16.0, 1.0, 9.0 / 16.0]),
        ("south", [7.0 / 16.0, 0.0, 7.0 / 16.0, 9.0 / 16.0, 1.0, 1.0]),
        ("west", [0.0, 0.0, 7.0 / 16.0, 9.0 / 16.0, 1.0, 9.0 / 16.0]),
        ("east", [7.0 / 16.0, 0.0, 7.0 / 16.0, 1.0, 1.0, 9.0 / 16.0]),
    ] {
        if props.get(property) == Some("true") {
            boxes.push(box_);
        }
    }
    boxes
}

fn fence_gate_boxes(facing: &str, open: bool, in_wall: bool) -> Vec<LocalBox> {
    if open {
        return Vec::new();
    }
    let height = if in_wall { 1.0 } else { 1.5 };
    if matches!(facing, "east" | "west") {
        vec![[6.0 / 16.0, 0.0, 0.0, 10.0 / 16.0, height, 1.0]]
    } else {
        vec![[0.0, 0.0, 6.0 / 16.0, 1.0, height, 10.0 / 16.0]]
    }
}

fn bed_box(part: &str, facing: &str) -> Vec<LocalBox> {
    let _ = (part, facing);
    vec![[0.0, 0.0, 0.0, 1.0, 9.0 / 16.0, 1.0]]
}

fn button_box(face: &str, facing: &str, powered: bool) -> Vec<LocalBox> {
    let depth = if powered { 2.0 } else { 3.0 };
    let thickness = depth / 16.0;
    match face {
        "floor" => vec![[
            5.0 / 16.0,
            0.0,
            5.0 / 16.0,
            11.0 / 16.0,
            thickness,
            11.0 / 16.0,
        ]],
        "ceiling" => vec![[
            5.0 / 16.0,
            1.0 - thickness,
            5.0 / 16.0,
            11.0 / 16.0,
            1.0,
            11.0 / 16.0,
        ]],
        _ => vec![facing_box(facing, thickness, 5.0 / 16.0, 11.0 / 16.0)],
    }
}

fn lever_box(face: &str, facing: &str) -> Vec<LocalBox> {
    let base = match face {
        "floor" => [
            5.0 / 16.0,
            0.0,
            5.0 / 16.0,
            11.0 / 16.0,
            2.0 / 16.0,
            11.0 / 16.0,
        ],
        "ceiling" => [
            5.0 / 16.0,
            14.0 / 16.0,
            5.0 / 16.0,
            11.0 / 16.0,
            1.0,
            11.0 / 16.0,
        ],
        _ => facing_box(facing, 2.0 / 16.0, 5.0 / 16.0, 11.0 / 16.0),
    };
    vec![base]
}

fn snow_layers(props: &PropMap) -> i32 {
    props
        .get("layers")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
}

/// Vanilla `SnowLayerBlock.SHAPES[layers]`, two pixels per layer; index 0 is
/// empty.
fn snow_shape(layers: i32) -> Vec<LocalBox> {
    if layers <= 0 {
        return Vec::new();
    }
    vec![[0.0, 0.0, 0.0, 1.0, layers as f64 * 2.0 / 16.0, 1.0]]
}

/// Vanilla `StairBlock` shape: a half-slab plus 1–3 upper corner pillars,
/// rotated to `facing`/`shape` and Y-flipped for the top half.
fn stair_boxes(half: &str, facing: &str, shape: &str) -> Vec<LocalBox> {
    // Base shape faces north, bottom half. SHAPE_OUTER is the half-slab plus one
    // corner; STRAIGHT adds its 90° rotation; INNER adds a third corner.
    let mut boxes = vec![[0.0, 0.0, 0.0, 1.0, 0.5, 1.0]];
    let corner: LocalBox = [0.0, 0.5, 0.0, 0.5, 1.0, 0.5];
    match shape {
        "inner_left" | "inner_right" => {
            boxes.push(corner);
            boxes.push(rot_y90(corner));
            boxes.push(rot_y90(rot_y90(corner)));
        }
        "outer_left" | "outer_right" => boxes.push(corner),
        _ => {
            boxes.push(corner);
            boxes.push(rot_y90(corner));
        }
    }

    if half == "top" {
        for b in &mut boxes {
            *b = invert_y(*b);
        }
    }

    // Vanilla derives the lookup direction from facing and shape.
    let dir = match shape {
        "inner_left" => ccw(facing),
        "outer_right" => cw(facing),
        _ => facing,
    };
    for _ in 0..dir_steps(dir) {
        for b in &mut boxes {
            *b = rot_y90(*b);
        }
    }

    boxes
}

/// Rotate a box 90° about the block's vertical center axis: `(x, z)` -> `(1-z,
/// x)`.
fn rot_y90([x0, y0, z0, x1, y1, z1]: LocalBox) -> LocalBox {
    [1.0 - z1, y0, x0, 1.0 - z0, y1, x1]
}

fn invert_y([x0, y0, z0, x1, y1, z1]: LocalBox) -> LocalBox {
    [x0, 1.0 - y1, z0, x1, 1.0 - y0, z1]
}

fn dir_steps(facing: &str) -> u32 {
    match facing {
        "east" => 1,
        "south" => 2,
        "west" => 3,
        _ => 0, // north
    }
}

fn cw(facing: &str) -> &'static str {
    match facing {
        "north" => "east",
        "east" => "south",
        "south" => "west",
        _ => "north",
    }
}

fn ccw(facing: &str) -> &'static str {
    match facing {
        "north" => "west",
        "west" => "south",
        "south" => "east",
        _ => "north",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(pairs: &[(&'static str, &'static str)]) -> PropMap {
        PropMap::from_pairs(pairs.to_vec())
    }

    #[test]
    fn closed_trapdoor_is_a_three_pixel_slab() {
        let shape = compute_shape(
            "oak_trapdoor",
            &props(&[("half", "bottom"), ("facing", "north"), ("open", "false")]),
        )
        .unwrap();
        assert_eq!(shape, vec![[0.0, 0.0, 0.0, 1.0, 3.0 / 16.0, 1.0]]);
    }

    #[test]
    fn open_trapdoor_becomes_a_vertical_panel() {
        let shape = compute_shape(
            "oak_trapdoor",
            &props(&[("half", "bottom"), ("facing", "east"), ("open", "true")]),
        )
        .unwrap();
        assert_eq!(shape, vec![[13.0 / 16.0, 0.0, 0.0, 1.0, 1.0, 1.0]]);
    }

    #[test]
    fn door_upper_half_stays_in_its_own_block_cell() {
        let shape = compute_shape(
            "oak_door",
            &props(&[
                ("half", "upper"),
                ("facing", "north"),
                ("hinge", "left"),
                ("open", "false"),
            ]),
        )
        .unwrap();
        assert_eq!(shape, vec![[0.0, 0.0, 0.0, 1.0, 1.0, 3.0 / 16.0]]);
    }

    #[test]
    fn fence_connection_adds_only_the_requested_arm() {
        let shape = compute_shape(
            "oak_fence",
            &props(&[
                ("north", "false"),
                ("south", "false"),
                ("west", "false"),
                ("east", "true"),
            ]),
        )
        .unwrap();
        assert_eq!(shape.len(), 2);
        assert!(
            shape
                .iter()
                .any(|b| { *b == [6.0 / 16.0, 6.0 / 16.0, 6.0 / 16.0, 1.0, 1.5, 10.0 / 16.0,] })
        );
    }

    #[test]
    fn torch_has_an_outline_but_no_collision_shape() {
        let props = props(&[]);
        assert_eq!(compute_shape("torch", &props), None);
        assert_eq!(
            compute_outline("torch", &props),
            Some(vec![[
                5.0 / 16.0,
                0.0,
                5.0 / 16.0,
                11.0 / 16.0,
                10.0 / 16.0,
                11.0 / 16.0,
            ]])
        );
    }
}
