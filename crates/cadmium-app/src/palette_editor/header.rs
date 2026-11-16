//! The dialog's header: its title and the Circle | Mixing switch.

use adw::prelude::*;

use super::PaletteEditor;
use super::presentation::{mode_name, modes};
use super::snapshot::Snapshot;
use crate::colour_wheel::COLOUR_CIRCLE;
use crate::state::PaletteMode;

pub(super) const TITLE: &str = "Palette";

pub(super) struct Header {
    pub(super) bar: adw::HeaderBar,
    pub(super) modes: adw::ToggleGroup,
}

pub(super) fn header() -> Header {
    let toggles = adw::ToggleGroup::new();
    for (mode, label) in modes(COLOUR_CIRCLE) {
        toggles.add(
            adw::Toggle::builder()
                .name(mode_name(*mode))
                .label(*label)
                .build(),
        );
    }
    // A single mode needs no switch.
    toggles.set_visible(COLOUR_CIRCLE);
    let title = gtk::Label::builder()
        .label(TITLE)
        .css_classes(["heading"])
        .build();
    let bar = adw::HeaderBar::builder().title_widget(&toggles).build();
    bar.pack_start(&title);
    Header {
        bar,
        modes: toggles,
    }
}

impl PaletteEditor {
    /// Shows the page of the dialog's mode, and the pick only where there is a sheet.
    pub(super) fn show_mode(&self, snapshot: &Snapshot) {
        let name = mode_name(snapshot.edit.mode);
        if self.modes.active_name().as_deref() != Some(name) {
            self.modes.set_active_name(Some(name));
        }
        self.stack.set_visible_child_name(name);
        let mixing = snapshot.edit.mode == PaletteMode::Mixing;
        self.column.pick.set_visible(mixing);
    }
}
