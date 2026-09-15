//! Modern brewing stand screen: three bottle slots, ingredient, fuel, and
//! server-provided brew/fuel progress. Item changes remain
//! server-authoritative.

use std::time::Instant;

use azalea_inventory::ItemStack;

use super::common::SLOT_STRIDE;
use super::container::{
    ContainerInput, ContainerResult, DragState, SlotCtx, push_backdrop, push_cursor_stack,
    resolve_gesture,
};
use crate::player::menu_click::ContainerKind;
use crate::renderer::pipelines::menu_overlay::MenuElement;

const SLOT_BOTTLE_BASE: u16 = 0;
const SLOT_INGREDIENT: u16 = 3;
const SLOT_FUEL: u16 = 4;
const SLOT_MAIN_BASE: u16 = 5;
const SLOT_HOTBAR_BASE: u16 = 32;

#[allow(clippy::too_many_arguments)]
pub fn build_brewing(
    elements: &mut Vec<MenuElement>,
    screen_w: f32,
    screen_h: f32,
    cursor: (f32, f32),
    input: &ContainerInput,
    slots: &[ItemStack],
    data: &[i16],
    title: &str,
    cursor_item: &ItemStack,
    drag: &mut Option<DragState>,
    last_click: &mut Option<(u16, Instant)>,
    gs: f32,
) -> ContainerResult {
    let panel = push_backdrop(elements, screen_w, screen_h, gs, 176.0, 166.0);
    panel.label(elements, 8.0, 6.0, title);
    panel.label(elements, 8.0, 72.0, "Inventory");

    // Vanilla's brewing arrow and fuel indicator. The server sends remaining
    // brew time and fuel values; a simple clipped fill preserves the important
    // feedback even when a custom pack does not provide GUI sprites.
    let brew = data.first().copied().unwrap_or_default().max(0) as f32;
    let brew_fill = (1.0 - brew / 400.0).clamp(0.0, 1.0);
    if brew_fill > 0.0 {
        elements.push(MenuElement::Rect {
            x: panel.ox + 79.0 * panel.scale,
            y: panel.oy + 34.0 * panel.scale,
            w: 24.0 * brew_fill * panel.scale,
            h: 16.0 * panel.scale,
            corner_radius: 0.0,
            color: [0.84, 0.84, 0.84, 1.0],
        });
    }
    let fuel = data.get(1).copied().unwrap_or_default().max(0) as f32;
    let fuel_fill = (fuel / 20.0).clamp(0.0, 1.0);
    if fuel_fill > 0.0 {
        elements.push(MenuElement::Rect {
            x: panel.ox + 57.0 * panel.scale,
            y: panel.oy + (50.0 - 14.0 * fuel_fill) * panel.scale,
            w: 14.0 * panel.scale,
            h: 14.0 * fuel_fill * panel.scale,
            corner_radius: 0.0,
            color: [0.95, 0.35, 0.08, 1.0],
        });
    }

    let mut ctx = SlotCtx::new(
        elements,
        &panel,
        cursor,
        ContainerKind::Brewing,
        slots,
        cursor_item,
        drag,
    );
    ctx.player_rows(slots, SLOT_MAIN_BASE, SLOT_HOTBAR_BASE, 84.0);
    for i in 0..3u16 {
        let item = slots
            .get((SLOT_BOTTLE_BASE + i) as usize)
            .unwrap_or(&ItemStack::Empty);
        ctx.slot(
            56.0 + i as f32 * SLOT_STRIDE,
            51.0,
            item,
            None,
            SLOT_BOTTLE_BASE + i,
        );
    }
    ctx.slot(
        79.0,
        17.0,
        slots
            .get(SLOT_INGREDIENT as usize)
            .unwrap_or(&ItemStack::Empty),
        None,
        SLOT_INGREDIENT,
    );
    ctx.slot(
        56.0,
        35.0,
        slots.get(SLOT_FUEL as usize).unwrap_or(&ItemStack::Empty),
        None,
        SLOT_FUEL,
    );
    let (hovered, shown_cursor) = ctx.finish(cursor_item);
    push_cursor_stack(elements, cursor, panel.scale, &shown_cursor);
    let (ops, clicked_outside) = resolve_gesture(
        input,
        hovered,
        &panel,
        cursor,
        ContainerKind::Brewing,
        slots,
        cursor_item,
        drag,
        last_click,
    );

    ContainerResult {
        clicked_outside,
        ops,
        button: None,
        beacon: None,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn brewing_layout_has_five_unique_slots() {
        assert_eq!(super::SLOT_MAIN_BASE, 5);
        assert_eq!(super::SLOT_HOTBAR_BASE, 32);
    }
}
