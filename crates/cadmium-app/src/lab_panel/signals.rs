use std::rc::{Rc, Weak};

use adw::prelude::*;
use cadmium_engine::params::ParamSpec;

use super::{LabPanel, ParamControl};
use crate::sliders::format_param;

impl LabPanel {
    pub(super) fn connect_controls(self: &Rc<Self>, reset: &gtk::Button) {
        let panel = Rc::downgrade(self);
        self.view_group.connect_active_name_notify(move |_| {
            with_panel(&panel, LabPanel::apply_view_choice);
        });
        let panel = Rc::downgrade(self);
        self.pigment_dropdown.connect_selected_notify(move |_| {
            with_panel(&panel, LabPanel::apply_view_choice);
        });
        for control in &self.params {
            self.connect_param(control);
        }
        let panel = Rc::downgrade(self);
        reset.connect_clicked(move |_| {
            with_panel(&panel, LabPanel::reset_to_defaults);
        });
    }

    fn connect_param(self: &Rc<Self>, control: &ParamControl) {
        let panel = Rc::downgrade(self);
        let spec = control.spec;
        let label = control.label.clone();
        control.scale.connect_value_changed(move |scale| {
            with_panel(&panel, |panel| panel.change_param(spec, &label, scale))
        });
    }

    fn change_param(&self, spec: &'static ParamSpec, label: &gtk::Label, scale: &gtk::Scale) {
        let value = scale.value() as f32;
        self.state.borrow_mut().set_param(spec.key, value);
        label.set_label(&format_param(spec, value));
    }

    fn reset_to_defaults(&self) {
        self.state.borrow_mut().reset_params();
        self.refresh();
    }

    pub(super) fn connect_pen_on_map(self: &Rc<Self>) {
        let shown = Rc::downgrade(self);
        self.root.connect_map(move |_| show_pen_if_alive(&shown));
    }
}

fn show_pen_if_alive(panel: &Weak<LabPanel>) {
    if let Some(panel) = panel.upgrade() {
        panel.show_pen();
    }
}

fn with_panel(panel: &Weak<LabPanel>, action: impl FnOnce(&LabPanel)) {
    if let Some(panel) = panel.upgrade()
        && !panel.syncing.get()
    {
        action(&panel);
    }
}
