//! Shared modern screens for the remaining vanilla container menus.
//!
//! These screens intentionally keep recipe/output behavior
//! server-authoritative. They still expose the exact slot contracts and the
//! same click/drag path as the other container screens, so players can use
//! these menus with vanilla servers even when a custom resource pack is
//! unavailable.

use std::time::Instant;

use azalea_inventory::ItemStack;

use super::container::{
    ContainerInput, ContainerResult, DragState, SlotCtx, push_backdrop, push_cursor_stack,
    resolve_gesture,
};
use crate::player::menu_click::ContainerKind;
use crate::renderer::pipelines::menu_overlay::MenuElement;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SpecializedContainer {
    Grindstone,
    Lectern,
    Loom,
    Smithing,
    Cartography,
    Stonecutter,
}

impl SpecializedContainer {
    pub fn click_kind(self) -> ContainerKind {
        match self {
            Self::Grindstone => ContainerKind::Grindstone,
            Self::Lectern => ContainerKind::Lectern,
            Self::Loom => ContainerKind::Loom,
            Self::Smithing => ContainerKind::Smithing,
            Self::Cartography => ContainerKind::Cartography,
            Self::Stonecutter => ContainerKind::Stonecutter,
        }
    }

    fn player_slot_start(self) -> u16 {
        self.click_kind().inv_start() as u16
    }

    fn hotbar_slot_start(self) -> u16 {
        (self.click_kind().slot_count() - 9) as u16
    }

    fn slots(self) -> &'static [(f32, f32)] {
        match self {
            Self::Grindstone => &[(27.0, 47.0), (76.0, 47.0), (134.0, 47.0)],
            Self::Lectern => &[(80.0, 47.0)],
            Self::Loom => &[(20.0, 47.0), (50.0, 47.0), (80.0, 47.0), (134.0, 47.0)],
            Self::Smithing => &[(20.0, 47.0), (50.0, 47.0), (80.0, 47.0), (134.0, 47.0)],
            Self::Cartography => &[(20.0, 47.0), (50.0, 47.0), (134.0, 47.0)],
            Self::Stonecutter => &[(20.0, 47.0), (134.0, 47.0)],
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Grindstone => "Grindstone",
            Self::Lectern => "Lectern",
            Self::Loom => "Loom",
            Self::Smithing => "Smithing Table",
            Self::Cartography => "Cartography Table",
            Self::Stonecutter => "Stonecutter",
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn build_specialized(
    elements: &mut Vec<MenuElement>,
    screen_w: f32,
    screen_h: f32,
    cursor: (f32, f32),
    input: &ContainerInput,
    slots: &[ItemStack],
    title: &str,
    kind: SpecializedContainer,
    cursor_item: &ItemStack,
    drag: &mut Option<DragState>,
    last_click: &mut Option<(u16, Instant)>,
    gs: f32,
) -> ContainerResult {
    let panel = push_backdrop(elements, screen_w, screen_h, gs, 176.0, 166.0);
    panel.label(
        elements,
        8.0,
        6.0,
        if title.is_empty() {
            kind.label()
        } else {
            title
        },
    );
    panel.label(elements, 8.0, 72.0, "Inventory");

    let click_kind = kind.click_kind();
    let mut ctx = SlotCtx::new(
        elements,
        &panel,
        cursor,
        click_kind,
        slots,
        cursor_item,
        drag,
    );
    ctx.player_rows(
        slots,
        kind.player_slot_start(),
        kind.hotbar_slot_start(),
        84.0,
    );
    for (slot, &(x, y)) in kind.slots().iter().enumerate() {
        let item = slots.get(slot).unwrap_or(&ItemStack::Empty);
        ctx.slot(x, y, item, None, slot as u16);
    }
    let (hovered, shown_cursor) = ctx.finish(cursor_item);
    push_cursor_stack(elements, cursor, panel.scale, &shown_cursor);
    let (ops, clicked_outside) = resolve_gesture(
        input,
        hovered,
        &panel,
        cursor,
        click_kind,
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
    use super::SpecializedContainer;

    #[test]
    fn specialized_menu_slot_contracts_match_vanilla() {
        assert_eq!(
            SpecializedContainer::Grindstone.click_kind().slot_count(),
            39
        );
        assert_eq!(SpecializedContainer::Lectern.click_kind().slot_count(), 37);
        assert_eq!(SpecializedContainer::Loom.click_kind().slot_count(), 40);
        assert_eq!(SpecializedContainer::Smithing.click_kind().inv_start(), 4);
        assert_eq!(
            SpecializedContainer::Cartography.click_kind().inv_start(),
            3
        );
        assert_eq!(
            SpecializedContainer::Stonecutter.click_kind().inv_start(),
            2
        );
    }
}
