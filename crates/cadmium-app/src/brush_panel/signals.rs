use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::glib;

use super::BrushPanel;
use super::presentation::{PLATE_GAP_KEY, slider_to_size};
use crate::LOG_DOMAIN;

impl BrushPanel {
    pub(super) fn connect_controls(self: &Rc<Self>) {
        let group = &self.tool_group;
        let panel = Rc::downgrade(self);
        group.size.scale.connect_value_changed(move |scale| {
            with_panel(&panel, |panel| panel.resize_brush(scale))
        });
        let panel = Rc::downgrade(self);
        group.load.scale.connect_value_changed(move |scale| {
            with_panel(&panel, |panel| panel.change_load(scale))
        });
        let panel = Rc::downgrade(self);
        group.plate_gap.scale.connect_value_changed(move |scale| {
            with_panel(&panel, |panel| panel.change_plate_gap(scale))
        });
        let panel = Rc::downgrade(self);
        group.medium.scale.connect_value_changed(move |scale| {
            with_panel(&panel, |panel| panel.change_medium(scale))
        });
        self.connect_rack();
        let panel = Rc::downgrade(self);
        group.reload_row.connect_active_notify(move |row| {
            with_panel(&panel, |panel| panel.change_reload_each_stroke(row))
        });
        let panel = Rc::downgrade(self);
        group.clean_each_row.connect_active_notify(move |row| {
            with_panel(&panel, |panel| panel.change_clean_each_stroke(row))
        });
    }

    pub(super) fn connect_refresh_on_map(self: &Rc<Self>) {
        let shown = Rc::downgrade(self);
        self.root.connect_map(move |_| refresh_if_alive(&shown));
    }

    fn resize_brush(self: &Rc<Self>, scale: &gtk::Scale) {
        let diameter = slider_to_size(scale.value());
        self.change_brush(|state| state.set_focused_size(diameter));
    }

    fn change_load(self: &Rc<Self>, scale: &gtk::Scale) {
        let load = scale.value() as f32;
        self.change_brush(|state| state.set_load(load));
    }

    fn change_plate_gap(self: &Rc<Self>, scale: &gtk::Scale) {
        let gap = scale.value() as f32;
        self.change_brush(|state| state.set_param(PLATE_GAP_KEY, gap));
    }

    fn change_medium(self: &Rc<Self>, scale: &gtk::Scale) {
        let medium = scale.value() as f32;
        self.change_brush(|state| state.set_medium(medium));
    }

    fn connect_rack(self: &Rc<Self>) {
        let panel = Rc::downgrade(self);
        self.rack
            .list
            .connect_row_selected(move |_, _| with_panel(&panel, |panel| panel.pick_brush()));
        let panel = Rc::downgrade(self);
        self.rack
            .list
            .connect_row_activated(move |_, row| with_panel(&panel, |panel| panel.edit_brush(row)));
    }

    fn pick_brush(self: &Rc<Self>) {
        let Some(index) = self.rack.picked() else {
            return;
        };
        self.change_brush(|state| {
            state.wear_library_brush(index);
        });
    }

    /// Double-click or Enter on a row: its brush is worn, if it wasn't yet, then edited.
    fn edit_brush(self: &Rc<Self>, row: &gtk::ListBoxRow) {
        if !row.is_selected() {
            self.rack.list.select_row(Some(row));
        }
        if let Err(error) = self.rack.list.activate_action("win.edit-brush", None) {
            glib::g_warning!(LOG_DOMAIN, "could not open the brush editor: {error}");
        }
    }

    fn change_reload_each_stroke(self: &Rc<Self>, row: &adw::SwitchRow) {
        let active = row.is_active();
        self.change_brush(|state| state.reload_each_stroke = active);
    }

    fn change_clean_each_stroke(self: &Rc<Self>, row: &adw::SwitchRow) {
        let active = row.is_active();
        self.change_brush(|state| state.clean_each_stroke = active);
    }
}

fn refresh_if_alive(panel: &Weak<BrushPanel>) {
    if let Some(panel) = panel.upgrade() {
        panel.refresh();
    }
}

fn with_panel(panel: &Weak<BrushPanel>, action: impl FnOnce(&Rc<BrushPanel>)) {
    if let Some(panel) = panel.upgrade()
        && !panel.syncing.get()
    {
        action(&panel);
    }
}
