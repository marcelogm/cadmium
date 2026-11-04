use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use cadmium_engine::brush::BrushShape;
use cadmium_engine::canvas::primer_srgb8;
use cadmium_engine::engine::PaintEngine;
use cadmium_engine::params::{ParamSpec, SimParams};
use cadmium_engine::pigments::{MAX_SLOTS, Mixture, mixture_srgb8};
use cadmium_engine::stroke::Tool;
use gtk::{gdk, glib};

use crate::LOG_DOMAIN;
use crate::lab_panel::{format_param, param_row};
use crate::state::{AppState, PigmentClick, SharedState};
use crate::swatch::{Swatch, SwatchShape};
use crate::tools::{KNIFE_HANDLING, UiTool};
use crate::window::CadmiumWindow;

const PANEL_MARGIN: i32 = 12;
const GROUP_SPACING: i32 = 18;
const PIGMENT_SWATCH_SIZE: i32 = 26;
const PIGMENT_SPACING: u32 = 4;
const PIGMENT_BOX_MARGIN: i32 = 10;
const PIGMENTS_PER_LINE: u32 = MAX_SLOTS as u32;
const MIN_PIGMENTS_PER_LINE: u32 = 4;
const LOADED_SWATCH_SIZE: (i32, i32) = (140, 20);
const LOADED_SPACING: i32 = 4;
const ADD_LIST_SWATCH_SIZE: i32 = 18;
const ADD_LIST_SPACING: i32 = 10;
const ADD_LIST_ROW_MARGIN: i32 = 6;
pub const SCALE_WIDTH: i32 = 140;
const VALUE_LABEL_CHARS: i32 = 6;
const MIN_SIZE_PX: f64 = 4.0;
const MAX_SIZE_PX: f64 = 500.0;
const SIZE_STEP: f64 = 0.01;
const SIZE_PAGE: f64 = 0.1;
const UNIT_STEP: f64 = 0.01;
const UNIT_PAGE: f64 = 0.1;
const MEDIUM_SLIDER_MAX: f64 = 0.9;
const PERCENT: f64 = 100.0;
const MIN_LISTED_SHARE: f32 = 0.01;
const CLEAN_LABEL: &str = "Limpo";
const PLATE_GAP_KEY: &str = "knife_gap_um";
const PLATE_GAP_TITLE: &str = "Espessura da placa";
const PLATE_GAP_UNIT: &str = "µm";
const RELOAD_EACH_STROKE: &str = "Recarregar a cada traço";
const CLEAN_EACH_STROKE: &str = "Limpar a cada traço";
const EYEDROPPER_HINT: &str = "Clique na tela para pegar a tinta que está nela. \
O botão direito do mouse e o botão inferior da caneta pegam a cor com qualquer ferramenta.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolWording {
    pub pigments: &'static str,
    pub loaded_title: &'static str,
    pub loaded_tooltip: &'static str,
    pub level_tooltip: &'static str,
    pub clean: &'static str,
    pub clean_button: &'static str,
    pub size_subtitle: Option<&'static str>,
    pub reload_hint: &'static str,
    pub clean_each_hint: &'static str,
}

const BRUSH_WORDING: ToolWording = ToolWording {
    pigments: "Clique carrega o pincel limpo, de novo mergulha mais; Ctrl+clique mistura um pouco",
    loaded_title: "No pincel",
    loaded_tooltip: "Esquerda: mistura na paleta · direita: ponta do pincel",
    level_tooltip: "Tinta na reserva do pincel",
    clean: CLEAN_LABEL,
    clean_button: "Limpar pincel",
    size_subtitle: None,
    reload_hint: "Repõe a reserva com a mistura da paleta",
    clean_each_hint: "Começa cada traço com o pincel limpo e recarregado",
};

const KNIFE_WORDING: ToolWording = ToolWording {
    pigments: "Clique carrega a espátula limpa, de novo pega mais; Ctrl+clique mistura um pouco",
    loaded_title: "Na espátula",
    loaded_tooltip: "Esquerda: mistura na paleta · direita: tinta na espátula",
    level_tooltip: "Tinta na espátula",
    clean: "Limpa",
    clean_button: "Limpar espátula",
    size_subtitle: Some("Comprimento da lâmina"),
    reload_hint: "Repõe a tinta da espátula até a carga",
    clean_each_hint: "Começa cada traço com a espátula limpa e recarregada",
};

pub fn tool_wording(tool: Tool) -> ToolWording {
    match tool {
        Tool::Knife => KNIFE_WORDING,
        _ => BRUSH_WORDING,
    }
}

pub fn tool_hint(tool: Tool) -> &'static str {
    match tool {
        Tool::Knife => KNIFE_HANDLING,
        _ => EYEDROPPER_HINT,
    }
}

fn plate_gap_spec() -> &'static ParamSpec {
    SimParams::specs()
        .iter()
        .find(|spec| spec.key == PLATE_GAP_KEY)
        .expect("the knife plate gap is a simulation parameter")
}

pub fn plate_gap_text(gap_um: f32) -> String {
    format!(
        "{} {PLATE_GAP_UNIT}",
        format_param(plate_gap_spec(), gap_um)
    )
}

pub const SHAPES: [(BrushShape, &str); 3] = [
    (BrushShape::Round, "Redondo"),
    (BrushShape::Flat, "Chato"),
    (BrushShape::Fan, "Leque"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisibleControls {
    pub pigments: bool,
    pub loaded: bool,
    pub size: bool,
    pub load: bool,
    pub medium: bool,
    pub plate_gap: bool,
    pub shape: bool,
    pub stroke_loading: bool,
    pub clean: bool,
    pub hint: bool,
}

pub fn visible_controls(tool: Tool) -> VisibleControls {
    let nothing = VisibleControls {
        pigments: false,
        loaded: false,
        size: false,
        load: false,
        medium: false,
        plate_gap: false,
        shape: false,
        stroke_loading: false,
        clean: false,
        hint: false,
    };
    match tool {
        Tool::Brush => VisibleControls {
            pigments: true,
            loaded: true,
            size: true,
            load: true,
            medium: true,
            plate_gap: false,
            shape: true,
            stroke_loading: true,
            clean: true,
            hint: false,
        },
        Tool::Knife => VisibleControls {
            pigments: true,
            loaded: true,
            size: true,
            load: true,
            plate_gap: true,
            stroke_loading: true,
            clean: true,
            hint: true,
            ..nothing
        },
        Tool::Blender => VisibleControls {
            size: true,
            shape: true,
            ..nothing
        },
        Tool::Rag => VisibleControls {
            size: true,
            ..nothing
        },
        Tool::Eyedropper => VisibleControls {
            hint: true,
            ..nothing
        },
    }
}

pub fn pigment_click(modifiers: gdk::ModifierType) -> PigmentClick {
    if modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
        PigmentClick::Mix
    } else if modifiers.contains(gdk::ModifierType::SHIFT_MASK) {
        PigmentClick::CleanAndLoad
    } else {
        PigmentClick::Choose
    }
}

pub fn slider_needs_sync(slider_value: f64, diameter_px: f32) -> bool {
    slider_to_size(slider_value) != diameter_px
}

pub fn describe_recipe(names: &[&str], recipe: &[(usize, f32)]) -> String {
    let mut shares: Vec<(&str, f32)> = recipe
        .iter()
        .filter(|(_, parts)| *parts > 0.0)
        .filter_map(|(slot, parts)| names.get(*slot).map(|name| (*name, *parts)))
        .collect();
    let total: f32 = shares.iter().map(|(_, parts)| parts).sum();
    if total <= 0.0 {
        return CLEAN_LABEL.to_string();
    }
    if let [(name, _)] = shares.as_slice() {
        return name.to_string();
    }
    shares.sort_by(|a, b| b.1.total_cmp(&a.1));
    shares
        .iter()
        .map(|(name, parts)| (name, parts / total))
        .filter(|(_, share)| *share >= MIN_LISTED_SHARE)
        .map(|(name, share)| format!("{name} {:.0}%", f64::from(share) * PERCENT))
        .collect::<Vec<_>>()
        .join(" · ")
}

pub fn size_to_slider(diameter_px: f32) -> f64 {
    f64::from(diameter_px).clamp(MIN_SIZE_PX, MAX_SIZE_PX).ln()
}

pub fn slider_to_size(value: f64) -> f32 {
    value.exp().clamp(MIN_SIZE_PX, MAX_SIZE_PX).round() as f32
}

fn percent_text(value: f64) -> String {
    format!("{:.0}%", value * PERCENT)
}

pub fn value_label() -> gtk::Label {
    gtk::Label::builder()
        .width_chars(VALUE_LABEL_CHARS)
        .xalign(1.0)
        .css_classes(["numeric", "dim-label"])
        .build()
}

pub fn slider_row(
    title: &str,
    lower: f64,
    upper: f64,
    step: f64,
    page: f64,
) -> (adw::ActionRow, gtk::Scale, gtk::Label) {
    let adjustment = gtk::Adjustment::new(lower, lower, upper, step, page, 0.0);
    let scale = gtk::Scale::builder()
        .adjustment(&adjustment)
        .draw_value(false)
        .width_request(SCALE_WIDTH)
        .valign(gtk::Align::Center)
        .build();
    scale.update_property(&[gtk::accessible::Property::Label(title)]);
    let label = value_label();
    let row = adw::ActionRow::builder().title(title).build();
    row.add_suffix(&scale);
    row.add_suffix(&label);
    (row, scale, label)
}

struct PigmentChoice {
    id: &'static str,
    name: &'static str,
    colour_index: &'static str,
    colour: [u8; 3],
}

struct Snapshot {
    tool: Tool,
    slots: Vec<String>,
    slot_names: Vec<&'static str>,
    slot_colours: Vec<[u8; 3]>,
    tooltips: Vec<String>,
    addable: Vec<PigmentChoice>,
    recipe: Vec<(usize, f32)>,
    held_is_clean: bool,
    palette_colour: [u8; 3],
    held_colour: [u8; 3],
    held_level: f32,
    diameter_px: f32,
    load: f32,
    plate_gap: f32,
    medium: f32,
    shape: BrushShape,
    reload_each_stroke: bool,
    clean_each_stroke: bool,
}

pub struct BrushPanel {
    root: gtk::Box,
    state: SharedState,
    window: glib::WeakRef<CadmiumWindow>,
    pigments_group: adw::PreferencesGroup,
    pigment_box: gtk::FlowBox,
    pigment_swatches: RefCell<Vec<Swatch>>,
    shown_slots: RefCell<Option<Vec<String>>>,
    brush_group: adw::PreferencesGroup,
    loaded_row: adw::ActionRow,
    loaded_swatch: Swatch,
    reserve_bar: gtk::LevelBar,
    size: (adw::ActionRow, gtk::Scale, gtk::Label),
    load: (adw::ActionRow, gtk::Scale, gtk::Label),
    plate_gap: (adw::ActionRow, gtk::Scale, gtk::Label),
    medium: (adw::ActionRow, gtk::Scale, gtk::Label),
    shape_row: adw::ComboRow,
    reload_row: adw::SwitchRow,
    clean_each_row: adw::SwitchRow,
    clean_button: gtk::Button,
    hint: gtk::Label,
    syncing: Cell<bool>,
}

impl BrushPanel {
    pub fn new(window: &CadmiumWindow, state: SharedState) -> Rc<BrushPanel> {
        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(GROUP_SPACING)
            .margin_top(PANEL_MARGIN)
            .margin_bottom(PANEL_MARGIN)
            .margin_start(PANEL_MARGIN)
            .margin_end(PANEL_MARGIN)
            .build();

        let pigment_box = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .homogeneous(true)
            .max_children_per_line(PIGMENTS_PER_LINE)
            .min_children_per_line(MIN_PIGMENTS_PER_LINE)
            .row_spacing(PIGMENT_SPACING)
            .column_spacing(PIGMENT_SPACING)
            .margin_top(PIGMENT_BOX_MARGIN)
            .margin_bottom(PIGMENT_BOX_MARGIN)
            .margin_start(PIGMENT_BOX_MARGIN)
            .margin_end(PIGMENT_BOX_MARGIN)
            .build();
        let pigment_card = gtk::Box::builder().css_classes(["card"]).build();
        pigment_card.append(&pigment_box);
        let pigments_group = adw::PreferencesGroup::builder()
            .title("Pigmentos")
            .description(BRUSH_WORDING.pigments)
            .build();
        pigments_group.add(&pigment_card);

        let loaded_swatch = Swatch::new(
            [0, 0, 0],
            SwatchShape::RoundedRect,
            LOADED_SWATCH_SIZE.0,
            LOADED_SWATCH_SIZE.1,
        );
        let reserve_bar = gtk::LevelBar::builder()
            .min_value(0.0)
            .max_value(1.0)
            .width_request(LOADED_SWATCH_SIZE.0)
            .build();
        let loaded = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(LOADED_SPACING)
            .valign(gtk::Align::Center)
            .build();
        loaded.append(&loaded_swatch);
        loaded.append(&reserve_bar);
        let loaded_row = adw::ActionRow::builder().subtitle_lines(2).build();
        loaded_row.add_suffix(&loaded);
        let size = slider_row(
            "Tamanho",
            MIN_SIZE_PX.ln(),
            MAX_SIZE_PX.ln(),
            SIZE_STEP,
            SIZE_PAGE,
        );
        let load = slider_row("Carga", 0.0, 1.0, UNIT_STEP, UNIT_PAGE);
        let plate_gap = param_row(plate_gap_spec());
        plate_gap.0.set_title(PLATE_GAP_TITLE);
        plate_gap
            .1
            .update_property(&[gtk::accessible::Property::Label(PLATE_GAP_TITLE)]);
        let medium = slider_row("Médium", 0.0, MEDIUM_SLIDER_MAX, UNIT_STEP, UNIT_PAGE);
        let shape_row = adw::ComboRow::builder()
            .title("Formato")
            .model(&gtk::StringList::new(&SHAPES.map(|(_, label)| label)))
            .build();
        let reload_row = adw::SwitchRow::builder().title(RELOAD_EACH_STROKE).build();
        let clean_each_row = adw::SwitchRow::builder().title(CLEAN_EACH_STROKE).build();
        let brush_group = adw::PreferencesGroup::builder().title("Pincel").build();
        brush_group.add(&loaded_row);
        brush_group.add(&size.0);
        brush_group.add(&load.0);
        brush_group.add(&plate_gap.0);
        brush_group.add(&medium.0);
        brush_group.add(&shape_row);
        brush_group.add(&reload_row);
        brush_group.add(&clean_each_row);

        let clean_button = gtk::Button::builder()
            .action_name("win.clean-brush")
            .halign(gtk::Align::Center)
            .css_classes(["pill"])
            .build();
        let hint = gtk::Label::builder()
            .label(EYEDROPPER_HINT)
            .wrap(true)
            .xalign(0.0)
            .css_classes(["dim-label"])
            .build();

        root.append(&pigments_group);
        root.append(&brush_group);
        root.append(&clean_button);
        root.append(&hint);

        let panel = Rc::new(BrushPanel {
            root,
            state,
            window: window.downgrade(),
            pigments_group,
            pigment_box,
            pigment_swatches: RefCell::new(Vec::new()),
            shown_slots: RefCell::new(None),
            brush_group,
            loaded_row,
            loaded_swatch,
            reserve_bar,
            size,
            load,
            plate_gap,
            medium,
            shape_row,
            reload_row,
            clean_each_row,
            clean_button,
            hint,
            syncing: Cell::new(false),
        });
        panel.connect_controls();
        let shown = Rc::downgrade(&panel);
        panel.root.connect_map(move |_| {
            if let Some(panel) = shown.upgrade() {
                panel.refresh();
            }
        });
        panel.refresh();
        panel
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub fn refresh(self: &Rc<Self>) {
        let snapshot = self.snapshot();
        self.syncing.set(true);
        if self.shown_slots.borrow().as_ref() != Some(&snapshot.slots) {
            self.rebuild_pigments(&snapshot);
        }
        for (slot, swatch) in self.pigment_swatches.borrow().iter().enumerate() {
            swatch.set_selected(
                snapshot
                    .recipe
                    .iter()
                    .any(|(chosen, parts)| *chosen == slot && *parts > 0.0),
            );
        }
        let wording = tool_wording(snapshot.tool);
        self.loaded_swatch.set_colour(snapshot.palette_colour);
        self.loaded_swatch
            .set_right_half_colour(Some(snapshot.held_colour));
        self.reserve_bar.set_value(f64::from(snapshot.held_level));
        let loaded_text = if snapshot.held_is_clean {
            wording.clean.to_string()
        } else {
            describe_recipe(&snapshot.slot_names, &snapshot.recipe)
        };
        self.loaded_row.set_subtitle(&loaded_text);
        if slider_needs_sync(self.size.1.value(), snapshot.diameter_px) {
            self.size.1.set_value(size_to_slider(snapshot.diameter_px));
        }
        self.size
            .2
            .set_label(&format!("{:.0} px", snapshot.diameter_px));
        self.load.1.set_value(f64::from(snapshot.load));
        self.load
            .2
            .set_label(&percent_text(f64::from(snapshot.load)));
        self.plate_gap.1.set_value(f64::from(snapshot.plate_gap));
        self.plate_gap
            .2
            .set_label(&plate_gap_text(snapshot.plate_gap));
        self.medium.1.set_value(f64::from(snapshot.medium));
        self.medium
            .2
            .set_label(&percent_text(f64::from(snapshot.medium)));
        let shape_index = SHAPES
            .iter()
            .position(|(shape, _)| *shape == snapshot.shape)
            .unwrap_or_default();
        self.shape_row.set_selected(shape_index as u32);
        self.reload_row.set_active(snapshot.reload_each_stroke);
        self.clean_each_row.set_active(snapshot.clean_each_stroke);
        self.show_controls_for(snapshot.tool);
        self.syncing.set(false);
    }

    fn snapshot(&self) -> Snapshot {
        let state = self.state.borrow();
        let session = &state.session;
        let db = &session.db;
        let slots = session.canvas.spec.pigment_slots.clone();
        let pigments: Vec<_> = slots.iter().filter_map(|id| db.get(id)).collect();
        let tool = match state.ui_tool {
            UiTool::Paint(tool) => tool,
            UiTool::Move => session.brush.tool,
        };
        let primer = primer_srgb8(db, session.canvas.spec.primer);
        let colour_of = |mixture: &Mixture| {
            if mixture.is_empty() {
                primer
            } else {
                mixture_srgb8(db, &slots, &mixture.fractions)
            }
        };
        let palette_colour = colour_of(&state.palette_mixture());
        let (held_colour, held_level, held_is_clean) = if tool == Tool::Knife {
            (
                colour_of(&state.knife_mixture),
                state.knife_level(),
                state.knife_is_clean(),
            )
        } else {
            (
                colour_of(&state.tip_mixture),
                state.reserve_level(),
                state.brush_is_clean(),
            )
        };
        Snapshot {
            tool,
            slot_names: pigments.iter().map(|pigment| pigment.name).collect(),
            slot_colours: slots.iter().map(|id| db.masstone_srgb8(id)).collect(),
            tooltips: pigments
                .iter()
                .map(|pigment| format!("{} ({})", pigment.name, pigment.colour_index))
                .collect(),
            addable: db
                .all()
                .iter()
                .filter(|pigment| !slots.iter().any(|id| id == pigment.id))
                .map(|pigment| PigmentChoice {
                    id: pigment.id,
                    name: pigment.name,
                    colour_index: pigment.colour_index,
                    colour: db.masstone_srgb8(pigment.id),
                })
                .collect(),
            slots,
            recipe: session.brush.recipe.clone(),
            held_is_clean,
            palette_colour,
            held_colour,
            held_level,
            diameter_px: session.brush.geometry.max_diameter_px,
            load: session.brush.load,
            plate_gap: session.engine.params().get(PLATE_GAP_KEY),
            medium: session.brush.medium,
            shape: session.brush.shape,
            reload_each_stroke: state.reload_each_stroke,
            clean_each_stroke: state.clean_each_stroke,
        }
    }

    fn rebuild_pigments(self: &Rc<Self>, snapshot: &Snapshot) {
        self.pigment_box.remove_all();
        let mut swatches = Vec::with_capacity(snapshot.slots.len());
        for (slot, colour) in snapshot.slot_colours.iter().enumerate() {
            let swatch = Swatch::new(
                *colour,
                SwatchShape::Circle,
                PIGMENT_SWATCH_SIZE,
                PIGMENT_SWATCH_SIZE,
            );
            let tooltip = snapshot.tooltips.get(slot).map(String::as_str);
            let button = gtk::Button::builder()
                .child(&swatch)
                .halign(gtk::Align::Center)
                .valign(gtk::Align::Center)
                .css_classes(["flat", "circular", "pigment-swatch"])
                .build();
            button.set_tooltip_text(tooltip);
            if let Some(name) = tooltip {
                button.update_property(&[gtk::accessible::Property::Label(name)]);
            }
            button.set_cursor_from_name(Some("pointer"));
            let panel = Rc::downgrade(self);
            button.connect_clicked(move |button| {
                let click = pigment_click(current_modifiers(button));
                if let Some(panel) = panel.upgrade() {
                    panel.choose_pigment(slot, click);
                }
            });
            self.pigment_box.append(&button);
            swatches.push(swatch);
        }
        if snapshot.slots.len() < MAX_SLOTS && !snapshot.addable.is_empty() {
            self.pigment_box
                .append(&self.add_pigment_button(&snapshot.addable));
        }
        self.pigment_swatches.replace(swatches);
        self.shown_slots.replace(Some(snapshot.slots.clone()));
    }

    fn add_pigment_button(self: &Rc<Self>, addable: &[PigmentChoice]) -> gtk::MenuButton {
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["navigation-sidebar"])
            .build();
        for choice in addable {
            let line = gtk::Box::builder()
                .spacing(ADD_LIST_SPACING)
                .margin_top(ADD_LIST_ROW_MARGIN)
                .margin_bottom(ADD_LIST_ROW_MARGIN)
                .build();
            line.append(&Swatch::new(
                choice.colour,
                SwatchShape::Circle,
                ADD_LIST_SWATCH_SIZE,
                ADD_LIST_SWATCH_SIZE,
            ));
            line.append(&gtk::Label::new(Some(choice.name)));
            line.append(
                &gtk::Label::builder()
                    .label(choice.colour_index)
                    .hexpand(true)
                    .xalign(1.0)
                    .css_classes(["dim-label", "caption"])
                    .build(),
            );
            let row = gtk::ListBoxRow::builder()
                .child(&line)
                .name(choice.id)
                .build();
            list.append(&row);
        }
        let popover = gtk::Popover::builder().child(&list).build();
        let open_popover = popover.downgrade();
        let button = gtk::MenuButton::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text("Adicionar pigmento")
            .popover(&popover)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .css_classes(["circular", "flat"])
            .build();
        let panel = Rc::downgrade(self);
        list.connect_row_activated(move |_, row| {
            if let Some(popover) = open_popover.upgrade() {
                popover.popdown();
            }
            let id = row.widget_name();
            if let Some(panel) = panel.upgrade() {
                panel.add_pigment(&id);
            }
        });
        button
    }

    fn show_controls_for(&self, tool: Tool) {
        let shown = visible_controls(tool);
        let wording = tool_wording(tool);
        self.pigments_group.set_description(Some(wording.pigments));
        self.loaded_row.set_title(wording.loaded_title);
        self.loaded_swatch
            .set_tooltip_text(Some(wording.loaded_tooltip));
        self.reserve_bar
            .set_tooltip_text(Some(wording.level_tooltip));
        self.reserve_bar
            .update_property(&[gtk::accessible::Property::Label(wording.level_tooltip)]);
        self.size
            .0
            .set_subtitle(wording.size_subtitle.unwrap_or_default());
        self.reload_row.set_subtitle(wording.reload_hint);
        self.clean_each_row.set_subtitle(wording.clean_each_hint);
        self.clean_button.set_label(wording.clean_button);
        self.hint.set_label(tool_hint(tool));
        self.pigments_group.set_visible(shown.pigments);
        self.brush_group
            .set_visible(shown.size || shown.loaded || shown.shape);
        self.brush_group.set_title(UiTool::Paint(tool).label());
        self.loaded_row.set_visible(shown.loaded);
        self.size.0.set_visible(shown.size);
        self.load.0.set_visible(shown.load);
        self.medium.0.set_visible(shown.medium);
        self.plate_gap.0.set_visible(shown.plate_gap);
        self.shape_row.set_visible(shown.shape);
        self.reload_row.set_visible(shown.stroke_loading);
        self.clean_each_row.set_visible(shown.stroke_loading);
        self.clean_button.set_visible(shown.clean);
        self.hint.set_visible(shown.hint);
    }

    fn connect_controls(self: &Rc<Self>) {
        let panel = Rc::downgrade(self);
        self.size.1.connect_value_changed(move |scale| {
            with_panel(&panel, |panel| {
                let diameter = slider_to_size(scale.value());
                panel.change_brush(|state| state.set_brush_size(diameter));
            });
        });
        let panel = Rc::downgrade(self);
        self.load.1.connect_value_changed(move |scale| {
            with_panel(&panel, |panel| {
                let load = scale.value() as f32;
                panel.change_brush(|state| state.set_load(load));
            });
        });
        let panel = Rc::downgrade(self);
        self.plate_gap.1.connect_value_changed(move |scale| {
            with_panel(&panel, |panel| {
                let gap = scale.value() as f32;
                panel.change_brush(|state| state.set_param(PLATE_GAP_KEY, gap));
            });
        });
        let panel = Rc::downgrade(self);
        self.medium.1.connect_value_changed(move |scale| {
            with_panel(&panel, |panel| {
                let medium = scale.value() as f32;
                panel.change_brush(|state| state.set_medium(medium));
            });
        });
        let panel = Rc::downgrade(self);
        self.shape_row.connect_selected_notify(move |row| {
            with_panel(&panel, |panel| {
                if let Some((shape, _)) = SHAPES.get(row.selected() as usize) {
                    panel.change_brush(|state| state.set_brush_shape(*shape));
                }
            });
        });
        let panel = Rc::downgrade(self);
        self.reload_row.connect_active_notify(move |row| {
            with_panel(&panel, |panel| {
                let active = row.is_active();
                panel.change_brush(|state| state.reload_each_stroke = active);
            });
        });
        let panel = Rc::downgrade(self);
        self.clean_each_row.connect_active_notify(move |row| {
            with_panel(&panel, |panel| {
                let active = row.is_active();
                panel.change_brush(|state| state.clean_each_stroke = active);
            });
        });
    }

    fn change_brush(self: &Rc<Self>, change: impl FnOnce(&mut AppState)) {
        change(&mut self.state.borrow_mut());
        self.after_brush_change();
    }

    fn choose_pigment(self: &Rc<Self>, slot: usize, click: PigmentClick) {
        self.change_brush(|state| state.choose_pigment(slot, click));
    }

    fn add_pigment(self: &Rc<Self>, id: &str) {
        let added = self.state.borrow_mut().add_pigment(id);
        match added {
            Ok(slot) => self.choose_pigment(slot, PigmentClick::CleanAndLoad),
            Err(reason) => glib::g_warning!(LOG_DOMAIN, "could not add {id:?}: {reason}"),
        }
    }

    fn after_brush_change(self: &Rc<Self>) {
        match self.window.upgrade() {
            Some(window) => {
                window.canvas().queue_draw();
                window.refresh_panels();
            }
            None => self.refresh(),
        }
    }
}

fn current_modifiers(widget: &impl IsA<gtk::Widget>) -> gdk::ModifierType {
    widget
        .display()
        .default_seat()
        .and_then(|seat| seat.keyboard())
        .map(|keyboard| keyboard.modifier_state())
        .unwrap_or_else(gdk::ModifierType::empty)
}

fn with_panel(panel: &Weak<BrushPanel>, action: impl FnOnce(&Rc<BrushPanel>)) {
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
    fn modifiers_choose_how_a_swatch_click_loads_the_brush() {
        assert_eq!(
            pigment_click(gdk::ModifierType::empty()),
            PigmentClick::Choose
        );
        assert_eq!(
            pigment_click(gdk::ModifierType::CONTROL_MASK),
            PigmentClick::Mix
        );
        assert_eq!(
            pigment_click(gdk::ModifierType::SHIFT_MASK),
            PigmentClick::CleanAndLoad
        );
        assert_eq!(
            pigment_click(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK),
            PigmentClick::Mix
        );
    }

    #[test]
    fn size_slider_is_left_alone_while_its_value_rounds_to_the_brush_size() {
        let dragged = size_to_slider(40.0) + SIZE_STEP / 10.0;
        assert!(!slider_needs_sync(dragged, 40.0));
        assert!(slider_needs_sync(size_to_slider(80.0), 40.0));
    }

    #[test]
    fn recipe_description_names_a_single_pigment() {
        assert_eq!(describe_recipe(&["Branco", "Azul"], &[(1, 1.0)]), "Azul");
    }

    #[test]
    fn recipe_description_lists_shares_largest_first() {
        assert_eq!(
            describe_recipe(&["Branco", "Azul", "Amarelo"], &[(1, 1.0), (2, 3.0)]),
            "Amarelo 75% · Azul 25%"
        );
    }

    #[test]
    fn empty_recipe_reads_as_a_clean_brush() {
        assert_eq!(describe_recipe(&["Branco"], &[]), CLEAN_LABEL);
        assert_eq!(describe_recipe(&["Branco"], &[(0, 0.0)]), CLEAN_LABEL);
    }

    #[test]
    fn size_slider_is_logarithmic_and_clamped() {
        assert_eq!(slider_to_size(size_to_slider(40.0)), 40.0);
        assert_eq!(slider_to_size(size_to_slider(1.0)), MIN_SIZE_PX as f32);
        assert_eq!(slider_to_size(size_to_slider(9000.0)), MAX_SIZE_PX as f32);
        let middle = slider_to_size((size_to_slider(4.0) + size_to_slider(500.0)) / 2.0);
        assert!(middle > 40.0 && middle < 50.0, "{middle}");
    }

    #[test]
    fn brush_shows_every_control_and_eyedropper_only_the_hint() {
        let brush = visible_controls(Tool::Brush);
        assert!(brush.pigments && brush.loaded && brush.size && brush.load && brush.medium);
        assert!(brush.shape && brush.stroke_loading && brush.clean && !brush.hint);
        assert!(!brush.plate_gap);
        let eyedropper = visible_controls(Tool::Eyedropper);
        assert!(eyedropper.hint);
        assert!(!eyedropper.pigments && !eyedropper.loaded && !eyedropper.size);
        assert!(!eyedropper.load && !eyedropper.medium && !eyedropper.plate_gap);
        assert!(!eyedropper.shape && !eyedropper.clean);
        assert!(!eyedropper.stroke_loading);
    }

    #[test]
    fn knife_loads_from_the_palette_and_sets_its_plate_but_has_no_medium_or_shape() {
        let knife = visible_controls(Tool::Knife);
        assert!(knife.pigments && knife.loaded && knife.size && knife.load);
        assert!(knife.plate_gap && knife.stroke_loading && knife.clean && knife.hint);
        assert!(!knife.medium && !knife.shape);
    }

    #[test]
    fn rag_shows_only_the_size() {
        let shown = visible_controls(Tool::Rag);
        assert!(shown.size);
        assert!(!shown.pigments && !shown.loaded && !shown.load && !shown.medium);
        assert!(!shown.plate_gap && !shown.shape && !shown.stroke_loading);
        assert!(!shown.clean && !shown.hint);
    }

    #[test]
    fn knife_panel_speaks_of_the_knife_and_how_to_hold_it() {
        let knife = tool_wording(Tool::Knife);
        assert_eq!(knife.clean_button, "Limpar espátula");
        assert_eq!(knife.loaded_title, "Na espátula");
        assert_eq!(tool_hint(Tool::Knife), KNIFE_HANDLING);
        assert_eq!(tool_wording(Tool::Brush).clean_button, "Limpar pincel");
        assert_eq!(tool_hint(Tool::Eyedropper), EYEDROPPER_HINT);
    }

    #[test]
    fn plate_gap_control_drives_the_knife_gap_parameter() {
        assert_eq!(plate_gap_spec().key, PLATE_GAP_KEY);
        assert_eq!(plate_gap_text(80.0), "80 µm");
    }
}
