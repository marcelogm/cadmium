use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::glib;
use oleo_engine::engine::PaintEngine;
use oleo_engine::params::{ParamGroup, ParamSpec, SimParams};
use oleo_engine::render::ViewMode;

use crate::brush_panel::slider_row;
use crate::state::{PenReadout, SharedState};
use crate::swatch::{Swatch, SwatchShape};

const PANEL_MARGIN: i32 = 12;
const GROUP_SPACING: i32 = 18;
const VIEW_SPACING: i32 = 12;
const PARAM_STEPS: f64 = 100.0;
const PARAM_PAGES: f64 = 10.0;
const WHOLE_NUMBER_RANGE: f32 = 20.0;
const PIGMENT_SWATCH_SIZE: (i32, i32) = (28, 20);
const PIGMENT_SWATCH_SPACING: i32 = 4;
const TINT_WHITE_PARTS: f32 = 1.0;
const PROVISIONAL_NOTICE: &str =
    "Os dados espectrais são provisórios (ajustados à mão, não medidos).";

const VIEW_COLOR: &str = "color";
const VIEW_HEIGHT: &str = "height";
const VIEW_WETNESS: &str = "wetness";
const VIEW_PIGMENT: &str = "pigment";

const VIEW_TOGGLES: [(&str, &str); 4] = [
    (VIEW_COLOR, "Cor"),
    (VIEW_HEIGHT, "Altura"),
    (VIEW_WETNESS, "Umidade"),
    (VIEW_PIGMENT, "Pigmento"),
];

pub const PARAM_GROUPS: [(ParamGroup, &str); 5] = [
    (ParamGroup::Transfer, "Transferência"),
    (ParamGroup::Dynamics, "Dinâmica"),
    (ParamGroup::Tools, "Ferramentas"),
    (ParamGroup::Footprint, "Pegada"),
    (ParamGroup::Render, "Render"),
];

pub fn view_name(mode: ViewMode) -> &'static str {
    match mode {
        ViewMode::Color => VIEW_COLOR,
        ViewMode::Height => VIEW_HEIGHT,
        ViewMode::Wetness => VIEW_WETNESS,
        ViewMode::Pigment(_) => VIEW_PIGMENT,
    }
}

pub fn view_from_name(name: &str, pigment_slot: u8) -> Option<ViewMode> {
    match name {
        VIEW_COLOR => Some(ViewMode::Color),
        VIEW_HEIGHT => Some(ViewMode::Height),
        VIEW_WETNESS => Some(ViewMode::Wetness),
        VIEW_PIGMENT => Some(ViewMode::Pigment(pigment_slot)),
        _ => None,
    }
}

pub fn format_param(spec: &ParamSpec, value: f32) -> String {
    if spec.max - spec.min >= WHOLE_NUMBER_RANGE {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

pub fn pen_texts(pen: &PenReadout) -> [String; 4] {
    [
        format!("{:.2}", pen.pressure),
        format!("{:.0}° / {:.0}°", pen.tilt_x, pen.tilt_y),
        pen.samples_last_frame.to_string(),
        format!("{:.0} Hz", pen.rate_hz),
    ]
}

struct ParamControl {
    spec: &'static ParamSpec,
    scale: gtk::Scale,
    label: gtk::Label,
}

struct SlotInfo {
    name: &'static str,
    subtitle: String,
    masstone: [u8; 3],
    tint: [u8; 3],
    glaze_over_white: [u8; 3],
    glaze_over_black: [u8; 3],
}

pub struct LabPanel {
    root: gtk::Box,
    state: SharedState,
    view_group: adw::ToggleGroup,
    pigment_row: adw::ActionRow,
    pigment_dropdown: gtk::DropDown,
    params: Vec<ParamControl>,
    pen_labels: [gtk::Label; 4],
    pigments_group: adw::PreferencesGroup,
    pigment_rows: RefCell<Vec<adw::ActionRow>>,
    shown_slots: RefCell<Option<Vec<String>>>,
    syncing: Cell<bool>,
}

impl LabPanel {
    pub fn new(state: SharedState) -> Rc<LabPanel> {
        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(GROUP_SPACING)
            .margin_top(PANEL_MARGIN)
            .margin_bottom(PANEL_MARGIN)
            .margin_start(PANEL_MARGIN)
            .margin_end(PANEL_MARGIN)
            .build();

        let view_group = adw::ToggleGroup::builder().build();
        for (name, label) in VIEW_TOGGLES {
            view_group.add(adw::Toggle::builder().name(name).label(label).build());
        }
        let pigment_dropdown = gtk::DropDown::builder().valign(gtk::Align::Center).build();
        let pigment_row = adw::ActionRow::builder().title("Pigmento").build();
        pigment_row.add_suffix(&pigment_dropdown);
        let pigment_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        pigment_list.append(&pigment_row);
        let view_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(VIEW_SPACING)
            .build();
        view_box.append(&view_group);
        view_box.append(&pigment_list);
        let view = adw::PreferencesGroup::builder().title("Visualizar").build();
        view.add(&view_box);
        root.append(&view);

        let mut params = Vec::new();
        for (group, title) in PARAM_GROUPS {
            let preferences = adw::PreferencesGroup::builder().title(title).build();
            for spec in SimParams::specs().iter().filter(|spec| spec.group == group) {
                let range = f64::from(spec.max - spec.min);
                let (row, scale, label) = slider_row(
                    spec.label,
                    f64::from(spec.min),
                    f64::from(spec.max),
                    range / PARAM_STEPS,
                    range / PARAM_PAGES,
                );
                preferences.add(&row);
                params.push(ParamControl { spec, scale, label });
            }
            root.append(&preferences);
        }
        let reset = gtk::Button::builder()
            .label("Restaurar padrões")
            .halign(gtk::Align::Center)
            .css_classes(["pill"])
            .build();
        root.append(&reset);

        let pen = adw::PreferencesGroup::builder().title("Caneta").build();
        let pen_labels = ["Pressão", "Inclinação", "Amostras por quadro", "Taxa"].map(|title| {
            let label = gtk::Label::builder()
                .css_classes(["numeric", "dim-label"])
                .build();
            let row = adw::ActionRow::builder().title(title).build();
            row.add_suffix(&label);
            pen.add(&row);
            label
        });
        root.append(&pen);

        let pigments_group = adw::PreferencesGroup::builder()
            .title("Pigmentos")
            .description("Massa, tinta 1:1 com branco, veladura sobre branco e preto")
            .build();
        root.append(&pigments_group);
        root.append(
            &gtk::Label::builder()
                .label(PROVISIONAL_NOTICE)
                .wrap(true)
                .xalign(0.0)
                .css_classes(["dim-label", "caption"])
                .build(),
        );

        let panel = Rc::new(LabPanel {
            root,
            state,
            view_group,
            pigment_row,
            pigment_dropdown,
            params,
            pen_labels,
            pigments_group,
            pigment_rows: RefCell::new(Vec::new()),
            shown_slots: RefCell::new(None),
            syncing: Cell::new(false),
        });
        panel.connect_controls(&reset);
        panel.follow_pen();
        panel.refresh();
        panel
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub fn refresh(&self) {
        let (slots, infos, mode, values) = {
            let state = self.state.borrow();
            let session = &state.session;
            let db = &session.db;
            let slots = session.canvas.spec.pigment_slots.clone();
            let infos: Vec<SlotInfo> = slots
                .iter()
                .filter_map(|id| db.get(id))
                .map(|pigment| SlotInfo {
                    name: pigment.name,
                    subtitle: format!("{} · {}", pigment.colour_index, pigment.provenance),
                    masstone: db.masstone_srgb8(pigment.id),
                    tint: db.tint_srgb8(pigment.id, TINT_WHITE_PARTS),
                    glaze_over_white: db.glaze_srgb8(pigment.id, false),
                    glaze_over_black: db.glaze_srgb8(pigment.id, true),
                })
                .collect();
            let params = session.engine.params();
            let values: Vec<f32> = self
                .params
                .iter()
                .map(|control| params.get(control.spec.key))
                .collect();
            (slots, infos, session.view_mode, values)
        };
        self.syncing.set(true);
        if self.shown_slots.borrow().as_ref() != Some(&slots) {
            self.rebuild_slots(&infos);
            self.shown_slots.replace(Some(slots));
        }
        self.view_group.set_active_name(Some(view_name(mode)));
        if let ViewMode::Pigment(slot) = mode {
            self.pigment_dropdown.set_selected(u32::from(slot));
        }
        self.pigment_row
            .set_sensitive(matches!(mode, ViewMode::Pigment(_)));
        for (control, value) in self.params.iter().zip(values) {
            control.scale.set_value(f64::from(value));
            control.label.set_label(&format_param(control.spec, value));
        }
        self.syncing.set(false);
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
                let row = adw::ActionRow::builder()
                    .title(info.name)
                    .subtitle(&info.subtitle)
                    .subtitle_lines(2)
                    .build();
                let swatches = gtk::Box::builder()
                    .spacing(PIGMENT_SWATCH_SPACING)
                    .valign(gtk::Align::Center)
                    .build();
                let glaze = lab_swatch(info.glaze_over_white, "Veladura sobre branco e preto");
                glaze.set_right_half_colour(Some(info.glaze_over_black));
                swatches.append(&lab_swatch(info.masstone, "Massa"));
                swatches.append(&lab_swatch(info.tint, "Tinta 1:1 com branco"));
                swatches.append(&glaze);
                row.add_suffix(&swatches);
                self.pigments_group.add(&row);
                row
            })
            .collect();
        self.pigment_rows.replace(rows);
    }

    fn connect_controls(self: &Rc<Self>, reset: &gtk::Button) {
        let panel = Rc::downgrade(self);
        self.view_group.connect_active_name_notify(move |_| {
            with_panel(&panel, LabPanel::apply_view_choice);
        });
        let panel = Rc::downgrade(self);
        self.pigment_dropdown.connect_selected_notify(move |_| {
            with_panel(&panel, LabPanel::apply_view_choice);
        });
        for control in &self.params {
            let panel = Rc::downgrade(self);
            let spec = control.spec;
            let label = control.label.clone();
            control.scale.connect_value_changed(move |scale| {
                with_panel(&panel, |panel| {
                    let value = scale.value() as f32;
                    panel.state.borrow_mut().set_param(spec.key, value);
                    label.set_label(&format_param(spec, value));
                });
            });
        }
        let panel = Rc::downgrade(self);
        reset.connect_clicked(move |_| {
            with_panel(&panel, |panel| {
                panel.state.borrow_mut().reset_params();
                panel.refresh();
            });
        });
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

    fn follow_pen(self: &Rc<Self>) {
        let panel = Rc::downgrade(self);
        let shown = RefCell::new(None::<[String; 4]>);
        self.root.add_tick_callback(move |root, _| {
            let Some(panel) = panel.upgrade() else {
                return glib::ControlFlow::Break;
            };
            if !root.is_mapped() {
                return glib::ControlFlow::Continue;
            }
            let Ok(state) = panel.state.try_borrow() else {
                return glib::ControlFlow::Continue;
            };
            let texts = pen_texts(&state.pen);
            drop(state);
            if shown.borrow().as_ref() != Some(&texts) {
                for (label, text) in panel.pen_labels.iter().zip(&texts) {
                    label.set_label(text);
                }
                shown.replace(Some(texts));
            }
            glib::ControlFlow::Continue
        });
    }
}

fn lab_swatch(colour: [u8; 3], tooltip: &str) -> Swatch {
    let swatch = Swatch::new(
        colour,
        SwatchShape::RoundedRect,
        PIGMENT_SWATCH_SIZE.0,
        PIGMENT_SWATCH_SIZE.1,
    );
    swatch.set_tooltip_text(Some(tooltip));
    swatch
}

fn with_panel(panel: &Weak<LabPanel>, action: impl FnOnce(&LabPanel)) {
    if let Some(panel) = panel.upgrade()
        && !panel.syncing.get()
    {
        action(&panel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_view_mode_round_trips_through_its_toggle_name() {
        for mode in [
            ViewMode::Color,
            ViewMode::Height,
            ViewMode::Wetness,
            ViewMode::Pigment(3),
        ] {
            assert_eq!(view_from_name(view_name(mode), 3), Some(mode));
        }
        assert_eq!(view_from_name("ultraviolet", 0), None);
    }

    #[test]
    fn every_param_group_has_a_panel_title() {
        for spec in SimParams::specs() {
            assert!(
                PARAM_GROUPS.iter().any(|(group, _)| *group == spec.group),
                "{}",
                spec.key
            );
        }
    }

    #[test]
    fn wide_ranges_show_whole_numbers_and_narrow_ones_two_decimals() {
        let count = SimParams::specs()
            .iter()
            .find(|spec| spec.key == "bristle_count")
            .unwrap();
        let fraction = SimParams::specs()
            .iter()
            .find(|spec| spec.key == "transfer_fraction")
            .unwrap();
        assert_eq!(format_param(count, 24.0), "24");
        assert_eq!(format_param(fraction, 0.1), "0.10");
    }

    #[test]
    fn pen_readout_reads_as_pressure_tilt_samples_and_rate() {
        let pen = PenReadout {
            pressure: 0.5,
            tilt_x: 12.4,
            tilt_y: -30.0,
            samples_last_frame: 7,
            rate_hz: 199.6,
        };
        assert_eq!(pen_texts(&pen), ["0.50", "12° / -30°", "7", "200 Hz"]);
    }
}
