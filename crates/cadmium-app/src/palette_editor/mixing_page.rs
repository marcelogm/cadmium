//! The Mixing page: the sheet's tools, undo and redo, Clear Sheet, and the sheet.

use adw::prelude::*;

use super::PaletteEditor;
use super::sheet_view::SheetView;
use super::snapshot::Snapshot;
use crate::state::SharedState;
use crate::tools::{UiTool, tool_toggle};

const PAGE_MARGIN: i32 = 18;
const PAGE_SPACING: i32 = 12;
const BAR_SPACING: i32 = 6;
const CLEAR_SHEET: &str = "Clear Sheet";
const UNDO_TOOLTIP: &str = "Undo on the Sheet";
const REDO_TOOLTIP: &str = "Redo on the Sheet";

pub(super) struct MixingPage {
    pub(super) root: gtk::Box,
    pub(super) tools: adw::ToggleGroup,
    pub(super) clear: gtk::Button,
    pub(super) sheet: SheetView,
}

pub(super) fn mixing_page(state: SharedState) -> MixingPage {
    let tools = adw::ToggleGroup::new();
    for tool in UiTool::SHEET {
        tools.add(tool_toggle(tool));
    }
    let clear = gtk::Button::builder()
        .label(CLEAR_SHEET)
        .hexpand(true)
        .halign(gtk::Align::End)
        .build();
    let bar = gtk::Box::builder().spacing(BAR_SPACING).build();
    bar.append(&tools);
    bar.append(&history_button(
        "edit-undo-symbolic",
        UNDO_TOOLTIP,
        "win.undo",
    ));
    bar.append(&history_button(
        "edit-redo-symbolic",
        REDO_TOOLTIP,
        "win.redo",
    ));
    bar.append(&clear);
    let sheet = SheetView::new(state);
    let root = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(PAGE_SPACING)
        .margin_top(PAGE_MARGIN)
        .margin_bottom(PAGE_MARGIN)
        .margin_start(PAGE_MARGIN)
        .margin_end(PAGE_MARGIN)
        .build();
    root.append(&bar);
    root.append(&sheet);
    MixingPage {
        root,
        tools,
        clear,
        sheet,
    }
}

fn history_button(icon: &str, tooltip: &str, action: &str) -> gtk::Button {
    gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tooltip)
        .action_name(action)
        .css_classes(["flat"])
        .build()
}

impl PaletteEditor {
    pub(super) fn show_sheet_tool(&self, snapshot: &Snapshot) {
        let name = snapshot.sheet_tool.name();
        let tools = &self.mixing.tools;
        if tools.active_name().as_deref() != Some(name) {
            tools.set_active_name(Some(name));
        }
    }

    /// Picks the sheet's tool as its toggle does.
    pub(super) fn choose_sheet_tool(&self, tool: UiTool) {
        self.mixing.tools.set_active_name(Some(tool.name()));
    }
}
