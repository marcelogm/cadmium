use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gdk, glib};
use oleo_engine::brush::BrushShape;
use oleo_engine::canvas::primer_srgb8;
use oleo_engine::pigments::{MAX_SLOTS, Mixture, mixture_srgb8};
use oleo_engine::stroke::Tool;

use crate::LOG_DOMAIN;
use crate::state::{AppState, PigmentClick, SharedState, mixture_is_empty};
use crate::swatch::{Swatch, SwatchShape};
use crate::tools::UiTool;
use crate::window::OleoWindow;

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
const PIGMENTS_DESCRIPTION: &str =
    "Clique carrega o pincel limpo, de novo mergulha mais; Ctrl+clique mistura um pouco";
const LOADED_TOOLTIP: &str = "Esquerda: mistura na paleta · direita: ponta do pincel";
const RESERVE_TOOLTIP: &str = "Tinta na reserva do pincel";
const RELOAD_EACH_STROKE: &str = "Recarregar a cada traço";
const RELOAD_EACH_STROKE_HINT: &str = "Repõe a reserva com a mistura da paleta";
const CLEAN_EACH_STROKE: &str = "Limpar a cada traço";
const CLEAN_EACH_STROKE_HINT: &str = "Começa cada traço com o pincel limpo e recarregado";
const EYEDROPPER_HINT: &str = "Clique na tela para pegar a tinta que está nela. \
O botão direito do mouse e o botão inferior da caneta pegam a cor com qualquer ferramenta.";

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
    pub load_and_medium: bool,
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
        load_and_medium: false,
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
            load_and_medium: true,
            shape: true,
            stroke_loading: true,
            clean: true,
            hint: false,
        },
        Tool::Blender => VisibleControls {
            size: true,
            shape: true,
            ..nothing
        },
        Tool::Knife | Tool::Rag => VisibleControls {
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
    brush_is_clean: bool,
    palette_colour: [u8; 3],
    tip_colour: [u8; 3],
    reserve_level: f32,
    diameter_px: f32,
    load: f32,
    medium: f32,
    shape: BrushShape,
    reload_each_stroke: bool,
    clean_each_stroke: bool,
}

pub struct BrushPanel {
    root: gtk::Box,
    state: SharedState,
    window: glib::WeakRef<OleoWindow>,
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
    medium: (adw::ActionRow, gtk::Scale, gtk::Label),
    shape_row: adw::ComboRow,
    reload_row: adw::SwitchRow,
    clean_each_row: adw::SwitchRow,
    clean_button: gtk::Button,
    hint: gtk::Label,
    syncing: Cell<bool>,
}

impl BrushPanel {
    pub fn new(window: &OleoWindow, state: SharedState) -> Rc<BrushPanel> {
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
            .description(PIGMENTS_DESCRIPTION)
            .build();
        pigments_group.add(&pigment_card);

        let loaded_swatch = Swatch::new(
            [0, 0, 0],
            SwatchShape::RoundedRect,
            LOADED_SWATCH_SIZE.0,
            LOADED_SWATCH_SIZE.1,
        );
        loaded_swatch.set_tooltip_text(Some(LOADED_TOOLTIP));
        let reserve_bar = gtk::LevelBar::builder()
            .min_value(0.0)
            .max_value(1.0)
            .width_request(LOADED_SWATCH_SIZE.0)
            .tooltip_text(RESERVE_TOOLTIP)
            .build();
        reserve_bar.update_property(&[gtk::accessible::Property::Label(RESERVE_TOOLTIP)]);
        let loaded = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(LOADED_SPACING)
            .valign(gtk::Align::Center)
            .build();
        loaded.append(&loaded_swatch);
        loaded.append(&reserve_bar);
        let loaded_row = adw::ActionRow::builder()
            .title("No pincel")
            .subtitle_lines(2)
            .build();
        loaded_row.add_suffix(&loaded);
        let size = slider_row(
            "Tamanho",
            MIN_SIZE_PX.ln(),
            MAX_SIZE_PX.ln(),
            SIZE_STEP,
            SIZE_PAGE,
        );
        let load = slider_row("Carga", 0.0, 1.0, UNIT_STEP, UNIT_PAGE);
        let medium = slider_row("Médium", 0.0, MEDIUM_SLIDER_MAX, UNIT_STEP, UNIT_PAGE);
        let shape_row = adw::ComboRow::builder()
            .title("Formato")
            .model(&gtk::StringList::new(&SHAPES.map(|(_, label)| label)))
            .build();
        let reload_row = adw::SwitchRow::builder()
            .title(RELOAD_EACH_STROKE)
            .subtitle(RELOAD_EACH_STROKE_HINT)
            .build();
        let clean_each_row = adw::SwitchRow::builder()
            .title(CLEAN_EACH_STROKE)
            .subtitle(CLEAN_EACH_STROKE_HINT)
            .build();
        let brush_group = adw::PreferencesGroup::builder().title("Pincel").build();
        brush_group.add(&loaded_row);
        brush_group.add(&size.0);
        brush_group.add(&load.0);
        brush_group.add(&medium.0);
        brush_group.add(&shape_row);
        brush_group.add(&reload_row);
        brush_group.add(&clean_each_row);

        let clean_button = gtk::Button::builder()
            .label("Limpar pincel")
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
            medium,
            shape_row,
            reload_row,
            clean_each_row,
            clean_button,
            hint,
            syncing: Cell::new(false),
        });
        panel.connect_controls();
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
        self.loaded_swatch.set_colour(snapshot.palette_colour);
        self.loaded_swatch
            .set_right_half_colour(Some(snapshot.tip_colour));
        self.reserve_bar
            .set_value(f64::from(snapshot.reserve_level));
        let loaded_text = if snapshot.brush_is_clean {
            CLEAN_LABEL.to_string()
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
            if mixture_is_empty(mixture) {
                primer
            } else {
                mixture_srgb8(db, &slots, &mixture.fractions)
            }
        };
        let palette_colour = colour_of(&state.palette_mixture());
        let tip_colour = colour_of(&state.tip_mixture);
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
            brush_is_clean: state.brush_is_clean(),
            palette_colour,
            tip_colour,
            reserve_level: state.reserve_level(),
            diameter_px: session.brush.geometry.max_diameter_px,
            load: session.brush.load,
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
        self.pigments_group.set_visible(shown.pigments);
        self.brush_group
            .set_visible(shown.size || shown.loaded || shown.shape);
        self.brush_group.set_title(UiTool::Paint(tool).label());
        self.loaded_row.set_visible(shown.loaded);
        self.size.0.set_visible(shown.size);
        self.load.0.set_visible(shown.load_and_medium);
        self.medium.0.set_visible(shown.load_and_medium);
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
        assert!(brush.pigments && brush.loaded && brush.size && brush.load_and_medium);
        assert!(brush.shape && brush.stroke_loading && brush.clean && !brush.hint);
        let eyedropper = visible_controls(Tool::Eyedropper);
        assert!(eyedropper.hint);
        assert!(!eyedropper.pigments && !eyedropper.loaded && !eyedropper.size);
        assert!(!eyedropper.load_and_medium && !eyedropper.shape && !eyedropper.clean);
        assert!(!eyedropper.stroke_loading);
    }

    #[test]
    fn knife_and_rag_show_only_the_size() {
        for tool in [Tool::Knife, Tool::Rag] {
            let shown = visible_controls(tool);
            assert!(shown.size);
            assert!(!shown.pigments && !shown.loaded && !shown.load_and_medium);
            assert!(!shown.shape && !shown.stroke_loading && !shown.clean && !shown.hint);
        }
    }
}
