use std::rc::{Rc, Weak};

use adw::prelude::*;

use super::BrushPanel;
use super::presentation::{PLATE_GAP_KEY, SHAPES, slider_to_size};

impl BrushPanel {
    pub(super) fn connect_controls(self: &Rc<Self>) {
        let panel = Rc::downgrade(self);
        self.tool_group.size.1.connect_value_changed(move |scale| {
            with_panel(&panel, |panel| panel.resize_brush(scale))
        });
        let panel = Rc::downgrade(self);
        self.tool_group.load.1.connect_value_changed(move |scale| {
            with_panel(&panel, |panel| panel.change_load(scale))
        });
        let panel = Rc::downgrade(self);
        self.tool_group
            .plate_gap
            .1
            .connect_value_changed(move |scale| {
                with_panel(&panel, |panel| panel.change_plate_gap(scale))
            });
        let panel = Rc::downgrade(self);
        self.tool_group
            .medium
            .1
            .connect_value_changed(move |scale| {
                with_panel(&panel, |panel| panel.change_medium(scale))
            });
        let panel = Rc::downgrade(self);
        self.tool_group
            .shape_row
            .connect_selected_notify(move |row| {
                with_panel(&panel, |panel| panel.change_shape(row))
            });
        let panel = Rc::downgrade(self);
        self.tool_group
            .reload_row
            .connect_active_notify(move |row| {
                with_panel(&panel, |panel| panel.change_reload_each_stroke(row))
            });
        let panel = Rc::downgrade(self);
        self.tool_group
            .clean_each_row
            .connect_active_notify(move |row| {
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

    fn change_shape(self: &Rc<Self>, row: &adw::ComboRow) {
        if let Some((shape, _)) = SHAPES.get(row.selected() as usize) {
            self.change_brush(|state| state.set_brush_shape(*shape));
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
