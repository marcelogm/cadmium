use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use cadmium_engine::pigments::MAX_SLOTS;
use cadmium_engine::stroke::Tool;
use gtk::{gdk, glib};

use crate::LOG_DOMAIN;
use crate::sliders::{SliderRow, param_row, slider_row};
use crate::state::{AppState, PigmentClick, SharedState};
use crate::swatch::{Swatch, SwatchShape};
use crate::window::CadmiumWindow;

mod presentation;
mod snapshot;

use presentation::{
    EYEDROPPER_HINT, MAX_SIZE_PX, MIN_SIZE_PX, PLATE_GAP_KEY, PanelTool, SHAPES, SIZE_STEP,
    describe_recipe, percent_text, pigment_click, pile_is_highlighted, plate_gap_spec,
    plate_gap_text, size_to_slider, slider_needs_sync, slider_to_size, tool_group_description,
    tool_hint, tool_wording, visible_controls,
};
use snapshot::{PigmentChoice, Snapshot};

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
const SIZE_PAGE: f64 = 0.1;
const UNIT_STEP: f64 = 0.01;
const UNIT_PAGE: f64 = 0.1;
const MEDIUM_SLIDER_MAX: f64 = 0.9;
const PLATE_GAP_TITLE: &str = "Espessura da placa";
const RELOAD_EACH_STROKE: &str = "Recarregar a cada traço";
const CLEAN_EACH_STROKE: &str = "Limpar a cada traço";

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
    size: SliderRow,
    load: SliderRow,
    plate_gap: SliderRow,
    medium: SliderRow,
    shape_row: adw::ComboRow,
    reload_row: adw::SwitchRow,
    clean_each_row: adw::SwitchRow,
    clean_button: gtk::Button,
    hint: gtk::Label,
    syncing: Cell<bool>,
}

impl BrushPanel {
    #[expect(clippy::excessive_nesting, clippy::too_many_lines)]
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
            .description(tool_wording(PanelTool::Paint(Tool::Brush)).pigments)
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
            tool_wording(PanelTool::Paint(Tool::Brush)).size_title,
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

    #[expect(clippy::too_many_lines)]
    pub fn refresh(self: &Rc<Self>) {
        let snapshot = Snapshot::of(&self.state.borrow());
        self.syncing.set(true);
        if self.shown_slots.borrow().as_ref() != Some(&snapshot.slots) {
            self.rebuild_pigments(&snapshot);
        }
        for (slot, swatch) in self.pigment_swatches.borrow().iter().enumerate() {
            swatch.set_selected(pile_is_highlighted(
                snapshot.tool,
                slot,
                snapshot.tube_slot,
                &snapshot.recipe,
            ));
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
        self.show_controls_for(
            snapshot.tool,
            tool_group_description(snapshot.tool, &snapshot.slot_names, snapshot.tube_slot),
        );
        self.syncing.set(false);
    }

    #[expect(clippy::excessive_nesting)]
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
                    panel.click_pile(slot, click);
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

    #[expect(clippy::excessive_nesting, clippy::too_many_lines)]
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
                panel.add_pile(&id);
            }
        });
        button
    }

    fn show_controls_for(&self, tool: PanelTool, group_description: Option<&str>) {
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
        self.size.0.set_title(wording.size_title);
        self.size
            .1
            .update_property(&[gtk::accessible::Property::Label(wording.size_title)]);
        self.size
            .0
            .set_subtitle(wording.size_subtitle.unwrap_or_default());
        self.reload_row.set_subtitle(wording.reload_hint);
        self.clean_each_row.set_subtitle(wording.clean_each_hint);
        self.clean_button.set_label(wording.clean_button);
        self.hint.set_label(tool_hint(tool));
        self.pigments_group.set_visible(shown.pigments);
        self.brush_group.set_visible(shown.tool_group());
        self.brush_group.set_title(tool.label());
        self.brush_group.set_description(group_description);
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

    #[expect(clippy::excessive_nesting, clippy::too_many_lines)]
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

    fn click_pile(self: &Rc<Self>, slot: usize, click: PigmentClick) {
        self.change_brush(|state| state.click_pile(slot, click));
    }

    fn add_pile(self: &Rc<Self>, id: &str) {
        let added = self.state.borrow_mut().add_pile(id);
        match added {
            Ok(_) => self.after_brush_change(),
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
