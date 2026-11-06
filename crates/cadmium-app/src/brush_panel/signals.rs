use std::rc::{Rc, Weak};

use adw::prelude::*;

use super::BrushPanel;
use super::presentation::{PLATE_GAP_KEY, slider_to_size};

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
        let panel = Rc::downgrade(self);
        group
            .brush_row
            .connect_selected_notify(move |row| with_panel(&panel, |panel| panel.pick_brush(row)));
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
        self.change_brush(|state| state.set_brush_size(diameter));
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

    fn pick_brush(self: &Rc<Self>, row: &adw::ComboRow) {
        let picked = self
            .state
            .borrow()
            .brushes
            .get(row.selected() as usize)
            .cloned();
        let Some(brush) = picked else {
            return;
        };
        self.change_brush(|state| state.wear_brush(&brush));
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
