//! Modern beacon screen with payment slot, effect selection, and player
//! inventory. The server validates beacon tier, payment, and selected effects.

use std::time::Instant;

use azalea_inventory::ItemStack;

use super::common::{FONT_SIZE, SLOT_STRIDE, WHITE, hit_test, rgb};
use super::container::{
    ContainerInput, ContainerResult, DragState, SlotCtx, push_backdrop, push_cursor_stack,
    resolve_gesture,
};
use crate::player::menu_click::ContainerKind;
use crate::renderer::pipelines::menu_overlay::MenuElement;

const SLOT_PAYMENT: u16 = 0;
const SLOT_MAIN_BASE: u16 = 1;
const SLOT_HOTBAR_BASE: u16 = 28;

// Registry ids in the current Java effect registry. These are sent as the
// protocol's optional effect ids, not as client-side gameplay state.
const PRIMARY_EFFECTS: [(&str, u32); 6] = [
    ("Speed", 1),
    ("Haste", 3),
    ("Resistance", 11),
    ("Jump Boost", 8),
    ("Strength", 5),
    ("Regeneration", 10),
];
const SECONDARY_EFFECTS: [(&str, u32); 4] = [
    ("Speed II", 1),
    ("Haste II", 3),
    ("Resistance II", 11),
    ("Strength II", 5),
];

#[allow(clippy::too_many_arguments)]
pub fn build_beacon(
    elements: &mut Vec<MenuElement>,
    screen_w: f32,
    screen_h: f32,
    cursor: (f32, f32),
    input: &ContainerInput,
    slots: &[ItemStack],
    title: &str,
    primary: Option<u32>,
    secondary: Option<u32>,
    cursor_item: &ItemStack,
    drag: &mut Option<DragState>,
    last_click: &mut Option<(u16, Instant)>,
    gs: f32,
) -> ContainerResult {
    let panel = push_backdrop(elements, screen_w, screen_h, gs, 230.0, 219.0);
    panel.label(elements, 8.0, 6.0, title);
    panel.label(elements, 27.0, 112.0, "Inventory");
    panel.label(elements, 12.0, 25.0, "Primary Power");
    panel.label(elements, 122.0, 25.0, "Secondary Power");

    let mut clicked = None;
    for (i, (name, id)) in PRIMARY_EFFECTS.iter().enumerate() {
        let x = 12.0 + (i % 2) as f32 * 52.0;
        let y = 39.0 + (i / 2) as f32 * 24.0;
        let selected = primary == Some(*id);
        let rect = [
            panel.ox + x * panel.scale,
            panel.oy + y * panel.scale,
            48.0 * panel.scale,
            20.0 * panel.scale,
        ];
        elements.push(MenuElement::Rect {
            x: rect[0],
            y: rect[1],
            w: rect[2],
            h: rect[3],
            corner_radius: 2.0 * panel.scale,
            color: if selected {
                [0.25, 0.55, 0.25, 1.0]
            } else if hit_test(cursor, rect) {
                [0.35, 0.35, 0.35, 1.0]
            } else {
                [0.18, 0.18, 0.18, 1.0]
            },
        });
        elements.push(MenuElement::Text {
            x: rect[0] + rect[2] / 2.0,
            y: rect[1] + 6.0 * panel.scale,
            text: (*name).into(),
            scale: FONT_SIZE * panel.scale * 0.8,
            color: WHITE,
            centered: true,
        });
        if input.left_pressed && hit_test(cursor, rect) {
            clicked = Some((Some(*id), secondary));
        }
    }
    for (i, (name, id)) in SECONDARY_EFFECTS.iter().enumerate() {
        let x = 122.0 + (i % 2) as f32 * 52.0;
        let y = 39.0 + (i / 2) as f32 * 24.0;
        let selected = secondary == Some(*id);
        let rect = [
            panel.ox + x * panel.scale,
            panel.oy + y * panel.scale,
            48.0 * panel.scale,
            20.0 * panel.scale,
        ];
        elements.push(MenuElement::Rect {
            x: rect[0],
            y: rect[1],
            w: rect[2],
            h: rect[3],
            corner_radius: 2.0 * panel.scale,
            color: if selected {
                [0.25, 0.55, 0.25, 1.0]
            } else if hit_test(cursor, rect) {
                [0.35, 0.35, 0.35, 1.0]
            } else {
                [0.18, 0.18, 0.18, 1.0]
            },
        });
        elements.push(MenuElement::Text {
            x: rect[0] + rect[2] / 2.0,
            y: rect[1] + 6.0 * panel.scale,
            text: (*name).into(),
            scale: FONT_SIZE * panel.scale * 0.8,
            color: WHITE,
            centered: true,
        });
        if input.left_pressed && hit_test(cursor, rect) {
            clicked = Some((primary, Some(*id)));
        }
    }

    let mut ctx = SlotCtx::new(
        elements,
        &panel,
        cursor,
        ContainerKind::Beacon,
        slots,
        cursor_item,
        drag,
    );
    ctx.player_rows(slots, SLOT_MAIN_BASE, SLOT_HOTBAR_BASE, 127.0);
    ctx.slot(
        107.0,
        91.0,
        slots
            .get(SLOT_PAYMENT as usize)
            .unwrap_or(&ItemStack::Empty),
        None,
        SLOT_PAYMENT,
    );
    let (hovered, shown_cursor) = ctx.finish(cursor_item);
    push_cursor_stack(elements, cursor, panel.scale, &shown_cursor);
    let (ops, clicked_outside) = resolve_gesture(
        input,
        hovered,
        &panel,
        cursor,
        ContainerKind::Beacon,
        slots,
        cursor_item,
        drag,
        last_click,
    );

    ContainerResult {
        clicked_outside,
        ops,
        button: None,
        beacon: clicked,
    }
}

#[cfg(test)]
mod tests {
    use super::{PRIMARY_EFFECTS, SECONDARY_EFFECTS};

    #[test]
    fn beacon_effect_choices_use_current_registry_ids() {
        assert_eq!(PRIMARY_EFFECTS[0], ("Speed", 1));
        assert_eq!(PRIMARY_EFFECTS[1], ("Haste", 3));
        assert_eq!(SECONDARY_EFFECTS[0].1, 1);
    }
}

#[allow(dead_code)]
const _: ([f32; 4], f32) = (rgb(0xFFFFFF), SLOT_STRIDE);
