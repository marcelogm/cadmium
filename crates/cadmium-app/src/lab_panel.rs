use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use cadmium_engine::params::ParamSpec;
use cadmium_engine::render::ViewMode;

use crate::side_panel::panel_root;
use crate::sliders::format_param;
use crate::state::SharedState;

mod build;
mod presentation;
mod signals;
mod snapshot;

use presentation::{pen_texts, view_from_name, view_name};
use snapshot::{SlotInfo, Snapshot};

struct ParamControl {
    spec: &'static ParamSpec,
    scale: gtk::Scale,
    label: gtk::Label,
}

pub struct LabPanel {
    root: gtk::Box,
    state: SharedState,
    view_group: adw::ToggleGroup,
    pigment_row: adw::ActionRow,
    pigment_dropdown: gtk::DropDown,
    params: Vec<ParamControl>,
    pen_labels: [gtk::Label; 4],
    shown_pen: RefCell<Option<[String; 4]>>,
    pigments_group: adw::PreferencesGroup,
    pigment_rows: RefCell<Vec<adw::ActionRow>>,
    shown_slots: RefCell<Option<Vec<String>>>,
    syncing: Cell<bool>,
}

impl LabPanel {
    pub fn new(state: SharedState) -> Rc<LabPanel> {
        let root = panel_root();
        let view = build::view_controls();
        root.append(&view.group);
        let params = build::param_groups(&root);
        let reset = build::reset_button();
        root.append(&reset);
        let (pen, pen_labels) = build::pen_group();
        root.append(&pen);
        let pigments_group = build::pigments_group();
        root.append(&pigments_group);
        root.append(&build::provisional_notice());
        let panel = Rc::new(LabPanel {
            root,
            state,
            view_group: view.toggles,
            pigment_row: view.pigment_row,
            pigment_dropdown: view.pigment_dropdown,
            params,
            pen_labels,
            shown_pen: RefCell::new(None),
            pigments_group,
            pigment_rows: RefCell::new(Vec::new()),
            shown_slots: RefCell::new(None),
            syncing: Cell::new(false),
        });
        panel.connect_controls(&reset);
        panel.connect_pen_on_map();
        panel.refresh();
        panel
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub fn refresh(&self) {
        let snapshot = Snapshot::of(
            &self.state.borrow(),
            self.params.iter().map(|control| control.spec.key),
        );
        self.syncing.set(true);
        if self.shown_slots.borrow().as_ref() != Some(&snapshot.slots) {
            self.rebuild_slots(&snapshot.infos);
            self.shown_slots.replace(Some(snapshot.slots));
        }
        self.show_view_mode(snapshot.mode);
        self.show_param_values(snapshot.values);
        self.syncing.set(false);
        self.show_pen();
    }

    fn show_view_mode(&self, mode: ViewMode) {
        self.view_group.set_active_name(Some(view_name(mode)));
        if let ViewMode::Pigment(slot) = mode {
            self.pigment_dropdown.set_selected(u32::from(slot));
        }
        self.pigment_row
            .set_sensitive(matches!(mode, ViewMode::Pigment(_)));
    }

    fn show_param_values(&self, values: Vec<f32>) {
        for (control, value) in self.params.iter().zip(values) {
            control.scale.set_value(f64::from(value));
            control.label.set_label(&format_param(control.spec, value));
        }
    }

    fn rebuild_slots(&self, infos: &[SlotInfo]) {
        let names: Vec<&str> = infos.iter().map(|info| info.name).collect();
        self.pigment_dropdown
            .set_model(Some(&gtk::StringList::new(&names)));
        for row in self.pigment_rows.take() {
            self.pigments_group.remove(&row);
        }
        let rows = infos
            .iter()
            .map(|info| {
                let row = build::slot_row(info);
                self.pigments_group.add(&row);
                row
            })
            .collect();
        self.pigment_rows.replace(rows);
    }

    fn apply_view_choice(&self) {
        let Some(name) = self.view_group.active_name() else {
            return;
        };
        let slot = u8::try_from(self.pigment_dropdown.selected()).unwrap_or_default();
        let Some(mode) = view_from_name(&name, slot) else {
            return;
        };
        self.pigment_row
            .set_sensitive(matches!(mode, ViewMode::Pigment(_)));
        self.state.borrow_mut().set_view_mode(mode);
    }

    pub fn show_pen(&self) {
        if !self.root.is_mapped() {
            return;
        }
        let Ok(state) = self.state.try_borrow() else {
            return;
        };
        let texts = pen_texts(&state.pen);
        drop(state);
        if self.shown_pen.borrow().as_ref() != Some(&texts) {
            self.show_pen_texts(&texts);
            self.shown_pen.replace(Some(texts));
        }
    }

    fn show_pen_texts(&self, texts: &[String; 4]) {
        for (label, text) in self.pen_labels.iter().zip(texts) {
            label.set_label(text);
        }
    }
}
