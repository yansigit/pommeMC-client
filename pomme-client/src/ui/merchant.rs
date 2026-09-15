//! Villager and wandering-trader screen.
//!
//! The server owns offer validity and inventory changes. The client displays
//! the offer list, lets the player choose a trade, and uses the normal
//! container click path for the two payment slots and result slot.

use std::time::Instant;

use azalea_inventory::ItemStack;
use azalea_protocol::packets::game::c_merchant_offers::MerchantOffer;

use super::common::{FONT_SIZE, SLOT_SIZE, WHITE, hit_test, push_item_icon};
use super::container::{
    ContainerInput, ContainerResult, DragState, Panel, SlotCtx, push_backdrop, push_cursor_stack,
    resolve_gesture,
};
use crate::player::menu_click::ContainerKind;
use crate::renderer::pipelines::menu_overlay::MenuElement;

const PANEL_W: f32 = 276.0;
const PANEL_H: f32 = 166.0;
const OFFERS_X: f32 = 101.0;
const OFFERS_Y: f32 = 18.0;
const OFFER_W: f32 = 166.0;
const OFFER_H: f32 = 20.0;
const OFFER_GAP: f32 = 2.0;

/// The latest server-supplied merchant data for an open menu.
#[derive(Default)]
pub struct MerchantState {
    pub offers: Vec<MerchantOffer>,
    pub villager_level: u32,
    pub villager_xp: u32,
    pub show_progress: bool,
    pub can_restock: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn build_merchant(
    elements: &mut Vec<MenuElement>,
    screen_w: f32,
    screen_h: f32,
    cursor: (f32, f32),
    input: &ContainerInput,
    slots: &[ItemStack],
    title: &str,
    state: &MerchantState,
    cursor_item: &ItemStack,
    drag: &mut Option<DragState>,
    last_click: &mut Option<(u16, Instant)>,
    gs: f32,
) -> ContainerResult {
    let panel = push_backdrop(elements, screen_w, screen_h, gs, PANEL_W, PANEL_H);
    elements.push(MenuElement::Rect {
        x: panel.ox,
        y: panel.oy,
        w: panel.w,
        h: panel.h,
        corner_radius: 2.0 * panel.scale,
        color: [0.78, 0.78, 0.78, 1.0],
    });
    elements.push(MenuElement::Rect {
        x: panel.ox + 2.0 * panel.scale,
        y: panel.oy + 2.0 * panel.scale,
        w: panel.w - 4.0 * panel.scale,
        h: panel.h - 4.0 * panel.scale,
        corner_radius: 1.0 * panel.scale,
        color: [0.20, 0.20, 0.20, 1.0],
    });
    panel.label(elements, 8.0, 6.0, title);
    let restock = if state.can_restock { " +R" } else { "" };
    let progress = if state.show_progress {
        format!(
            "Level {} XP {}{}",
            state.villager_level, state.villager_xp, restock
        )
    } else {
        format!("Level {}{}", state.villager_level, restock)
    };
    panel.label(elements, OFFERS_X, 6.0, &progress);
    panel.label(elements, 8.0, 72.0, "Inventory");

    let mut selected = None;
    for (index, offer) in state.offers.iter().enumerate() {
        let y = OFFERS_Y + index as f32 * (OFFER_H + OFFER_GAP);
        if y + OFFER_H > 80.0 {
            break;
        }
        let x = OFFERS_X;
        let enabled = !offer.out_of_stock;
        let hovered = hit_test(
            cursor,
            [
                panel.ox + x * panel.scale,
                panel.oy + y * panel.scale,
                OFFER_W * panel.scale,
                OFFER_H * panel.scale,
            ],
        );
        elements.push(MenuElement::Rect {
            x: panel.ox + x * panel.scale,
            y: panel.oy + y * panel.scale,
            w: OFFER_W * panel.scale,
            h: OFFER_H * panel.scale,
            corner_radius: 1.0 * panel.scale,
            color: if hovered && enabled {
                [0.38, 0.38, 0.38, 1.0]
            } else if enabled {
                [0.28, 0.28, 0.28, 1.0]
            } else {
                [0.18, 0.18, 0.18, 1.0]
            },
        });
        let payment_a = ItemStack::new(offer.base_cost_a.item, offer.base_cost_a.count);
        draw_offer_item(elements, &panel, &payment_a, x + 4.0, y + 2.0, enabled);
        if let Some(cost_b) = &offer.cost_b {
            let payment_b = ItemStack::new(cost_b.item, cost_b.count);
            draw_offer_item(elements, &panel, &payment_b, x + 24.0, y + 2.0, enabled);
        }
        draw_offer_item(elements, &panel, &offer.result, x + 72.0, y + 2.0, enabled);
        elements.push(MenuElement::TextFlat {
            x: panel.ox + (x + 48.0) * panel.scale,
            y: panel.oy + (y + 6.0) * panel.scale,
            text: "➜".into(),
            scale: FONT_SIZE * panel.scale,
            color: if enabled {
                WHITE
            } else {
                [0.45, 0.45, 0.45, 1.0]
            },
        });
        if hovered && input.left_pressed && enabled {
            selected = Some(index as u32);
        }
    }

    if state.offers.is_empty() {
        panel.label(elements, OFFERS_X + 12.0, 38.0, "No offers");
    }
    let mut ctx = SlotCtx::new(
        elements,
        &panel,
        cursor,
        ContainerKind::Merchant,
        slots,
        cursor_item,
        drag,
    );
    ctx.player_rows(slots, 3, 30, 84.0);
    let item = |num: u16| slots.get(num as usize).unwrap_or(&ItemStack::Empty);
    ctx.slot(8.0, 36.0, item(0), None, 0);
    ctx.slot(28.0, 36.0, item(1), None, 1);
    ctx.slot(76.0, 36.0, item(2), None, 2);
    let (hovered, shown_cursor) = ctx.finish(cursor_item);
    push_cursor_stack(elements, cursor, panel.scale, &shown_cursor);
    let (ops, clicked_outside) = resolve_gesture(
        input,
        hovered,
        &panel,
        cursor,
        ContainerKind::Merchant,
        slots,
        cursor_item,
        drag,
        last_click,
    );

    ContainerResult {
        clicked_outside,
        ops,
        button: selected,
        beacon: None,
    }
}

fn draw_offer_item(
    elements: &mut Vec<MenuElement>,
    panel: &Panel,
    item: &ItemStack,
    x: f32,
    y: f32,
    enabled: bool,
) {
    if let ItemStack::Present(data) = item {
        let mut tinted = data.clone();
        if !enabled {
            tinted.count = 0;
        }
        push_item_icon(
            elements,
            panel.ox + x * panel.scale,
            panel.oy + y * panel.scale,
            SLOT_SIZE * panel.scale,
            panel.scale,
            &tinted,
        );
    }
}

#[cfg(test)]
mod tests {
    use azalea_inventory::ItemStack;
    use azalea_protocol::packets::game::c_merchant_offers::{
        DataComponentExactPredicate, ItemCost, MerchantOffer,
    };
    use azalea_registry::builtin::ItemKind;

    use super::{MerchantState, build_merchant};
    use crate::ui::container::ContainerInput;

    fn offer(out_of_stock: bool) -> MerchantOffer {
        MerchantOffer {
            base_cost_a: ItemCost {
                item: ItemKind::Emerald,
                count: 1,
                components: DataComponentExactPredicate { expected: vec![] },
            },
            result: ItemStack::new(ItemKind::Bread, 1),
            cost_b: None,
            out_of_stock,
            uses: 0,
            max_uses: 1,
            xp: 1,
            special_price_diff: 0,
            price_multiplier: 0.05,
            demand: 0,
        }
    }

    #[test]
    fn empty_merchant_starts_without_offers_or_progress() {
        let state = MerchantState::default();
        assert!(state.offers.is_empty());
        assert_eq!(state.villager_level, 0);
        assert_eq!(state.villager_xp, 0);
        assert!(!state.show_progress);
    }

    #[test]
    fn clicking_available_offer_returns_trade_index() {
        let state = MerchantState {
            offers: vec![offer(false)],
            ..MerchantState::default()
        };
        let input = ContainerInput {
            left_pressed: true,
            right_pressed: false,
            middle_pressed: false,
            left_held: false,
            right_held: false,
            shift: false,
            hotbar_swap: None,
            swap_offhand: false,
            throw: false,
            throw_all: false,
        };
        let mut elements = Vec::new();
        let mut drag = None;
        let mut last_click = None;
        let result = build_merchant(
            &mut elements,
            400.0,
            300.0,
            (170.0, 90.0),
            &input,
            &vec![ItemStack::Empty; 39],
            "Villager",
            &state,
            &ItemStack::Empty,
            &mut drag,
            &mut last_click,
            1.0,
        );
        assert_eq!(result.button, Some(0));
    }

    #[test]
    fn clicking_out_of_stock_offer_does_not_select_it() {
        let state = MerchantState {
            offers: vec![offer(true)],
            ..MerchantState::default()
        };
        let input = ContainerInput {
            left_pressed: true,
            right_pressed: false,
            middle_pressed: false,
            left_held: false,
            right_held: false,
            shift: false,
            hotbar_swap: None,
            swap_offhand: false,
            throw: false,
            throw_all: false,
        };
        let mut elements = Vec::new();
        let mut drag = None;
        let mut last_click = None;
        let result = build_merchant(
            &mut elements,
            400.0,
            300.0,
            (170.0, 90.0),
            &input,
            &vec![ItemStack::Empty; 39],
            "Villager",
            &state,
            &ItemStack::Empty,
            &mut drag,
            &mut last_click,
            1.0,
        );
        assert_eq!(result.button, None);
    }
}
