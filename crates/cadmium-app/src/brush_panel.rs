use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use cadmium_studio::BrushStudio;
use gtk::glib;

use crate::LOG_DOMAIN;
use crate::side_panel::panel_root;
use crate::state::{AppState, PigmentClick, SharedState};
use crate::swatch::Swatch;
use crate::thumbnails::{ThumbnailCache, ThumbnailKey};
use crate::tools::panel::PanelTool;
use crate::window::CadmiumWindow;

mod build;
mod piles;
mod presentation;
mod rack;
mod signals;
mod snapshot;
mod thumbnail_job;

use build::{PigmentsGroup, ToolGroup};
use presentation::{
    RackEntry, describe_recipe, percent_text, pile_is_highlighted, plate_gap_text, size_to_slider,
    slider_needs_sync, tool_group_description, tool_hint,
};
use snapshot::Snapshot;
use thumbnail_job::ThumbnailJob;

pub struct BrushPanel {
    root: gtk::Box,
    state: SharedState,
    window: glib::WeakRef<CadmiumWindow>,
    pigments: PigmentsGroup,
    tool_group: ToolGroup,
    rack: rack::Rack,
    clean_button: gtk::Button,
    hint: gtk::Label,
    pigment_swatches: RefCell<Vec<Swatch>>,
    shown_slots: RefCell<Option<Vec<String>>>,
    shown_brushes: RefCell<Option<ShownBrushes>>,
    syncing: Cell<bool>,
    thumbnails: RefCell<ThumbnailCache>,
    studio: RefCell<Option<BrushStudio>>,
    thumbnail_job: Cell<ThumbnailJob>,
}

/// The entries and thumbnail keys the rack shows, in rack order.
type ShownBrushes = (Vec<RackEntry>, Vec<ThumbnailKey>);

impl BrushPanel {
    pub fn new(window: &CadmiumWindow, state: SharedState) -> Rc<BrushPanel> {
        let root = panel_root();
        let pigments = build::pigments_group();
        let tool_group = build::tool_group();
        let rack = rack::rack();
        let clean_button = build::clean_button();
        let hint = build::hint_label();
        root.append(&pigments.group);
        root.append(&rack.group);
        root.append(&tool_group.group);
        root.append(&clean_button);
        root.append(&hint);
        let panel = Rc::new(BrushPanel {
            root,
            state,
            window: window.downgrade(),
            pigments,
            tool_group,
            rack,
            clean_button,
            hint,
            pigment_swatches: RefCell::new(Vec::new()),
            shown_slots: RefCell::new(None),
            shown_brushes: RefCell::new(None),
            syncing: Cell::new(false),
            thumbnails: RefCell::new(ThumbnailCache::default()),
            studio: RefCell::new(None),
            thumbnail_job: Cell::new(ThumbnailJob::Idle),
        });
        panel.connect_controls();
        panel.connect_refresh_on_map();
        panel.refresh();
        panel
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub fn refresh(self: &Rc<Self>) {
        self.release_studio_while_editing();
        let snapshot = Snapshot::of(&self.state.borrow());
        self.syncing.set(true);
        if self.shown_slots.borrow().as_ref() != Some(&snapshot.slots) {
            self.rebuild_pigments(&snapshot);
        }
        self.highlight_piles(&snapshot);
        self.show_held_paint(&snapshot);
        self.show_sliders(&snapshot);
        self.show_rack(&snapshot);
        self.show_stroke_options(&snapshot);
        self.show_controls_for(
            snapshot.tool,
            tool_group_description(snapshot.tool, &snapshot.slot_names, snapshot.tube_slot),
        );
        self.syncing.set(false);
    }

    fn highlight_piles(&self, snapshot: &Snapshot) {
        for (slot, swatch) in self.pigment_swatches.borrow().iter().enumerate() {
            swatch.set_selected(pile_is_highlighted(
                snapshot.tool,
                slot,
                snapshot.tube_slot,
                &snapshot.recipe,
            ));
        }
    }

    fn show_held_paint(&self, snapshot: &Snapshot) {
        let wording = snapshot.tool.wording();
        let group = &self.tool_group;
        group.loaded_swatch.set_colour(snapshot.palette_colour);
        group
            .loaded_swatch
            .set_right_half_colour(Some(snapshot.held_colour));
        group.reserve_bar.set_value(f64::from(snapshot.held_level));
        let loaded_text = if snapshot.held_is_clean {
            wording.clean.to_string()
        } else {
            describe_recipe(&snapshot.slot_names, &snapshot.held_recipe)
        };
        group.loaded_row.set_subtitle(&loaded_text);
    }

    fn show_sliders(&self, snapshot: &Snapshot) {
        let group = &self.tool_group;
        if slider_needs_sync(group.size.scale.value(), snapshot.diameter_px) {
            group
                .size
                .scale
                .set_value(size_to_slider(snapshot.diameter_px));
        }
        group
            .size
            .value_label
            .set_label(&format!("{:.0} px", snapshot.diameter_px));
        group.load.scale.set_value(f64::from(snapshot.load));
        group
            .load
            .value_label
            .set_label(&percent_text(f64::from(snapshot.load)));
        group
            .plate_gap
            .scale
            .set_value(f64::from(snapshot.plate_gap));
        group
            .plate_gap
            .value_label
            .set_label(&plate_gap_text(snapshot.plate_gap));
        group.medium.scale.set_value(f64::from(snapshot.medium));
        group
            .medium
            .value_label
            .set_label(&percent_text(f64::from(snapshot.medium)));
    }

    fn show_rack(self: &Rc<Self>, snapshot: &Snapshot) {
        let unchanged = self
            .shown_brushes
            .borrow()
            .as_ref()
            .is_some_and(|(entries, keys)| {
                *entries == snapshot.brush_entries && *keys == snapshot.brush_keys
            });
        if !unchanged {
            let thumbnails = self.cached_thumbnails(&snapshot.brush_keys);
            self.rack.fill(&snapshot.brush_entries, &thumbnails);
            let shown = (snapshot.brush_entries.clone(), snapshot.brush_keys.clone());
            self.shown_brushes.replace(Some(shown));
        }
        self.rack.select(snapshot.brush_index);
        if let Some(window) = self.window.upgrade() {
            window.set_brush_deletable(snapshot.brush_is_user);
        }
        self.schedule_thumbnails(&snapshot.brush_keys);
    }

    fn show_stroke_options(&self, snapshot: &Snapshot) {
        let group = &self.tool_group;
        group.reload_row.set_active(snapshot.reload_each_stroke);
        group.clean_each_row.set_active(snapshot.clean_each_stroke);
    }

    fn show_controls_for(&self, tool: PanelTool, group_description: Option<&str>) {
        self.show_wording_for(tool);
        self.show_visible_controls_for(tool, group_description);
    }

    fn show_wording_for(&self, tool: PanelTool) {
        let wording = tool.wording();
        let group = &self.tool_group;
        self.pigments.group.set_description(Some(wording.pigments));
        group.loaded_row.set_title(wording.loaded_title);
        group
            .loaded_swatch
            .set_tooltip_text(Some(wording.loaded_tooltip));
        group
            .reserve_bar
            .set_tooltip_text(Some(wording.level_tooltip));
        group
            .reserve_bar
            .update_property(&[gtk::accessible::Property::Label(wording.level_tooltip)]);
        group.size.row.set_title(wording.size_title);
        group
            .size
            .scale
            .update_property(&[gtk::accessible::Property::Label(wording.size_title)]);
        group
            .size
            .row
            .set_subtitle(wording.size_subtitle.unwrap_or_default());
        group.reload_row.set_subtitle(wording.reload_hint);
        group.clean_each_row.set_subtitle(wording.clean_each_hint);
        self.clean_button.set_label(wording.clean_button);
        self.hint.set_label(tool_hint(tool));
    }

    fn show_visible_controls_for(&self, tool: PanelTool, group_description: Option<&str>) {
        let shown = tool.controls();
        let group = &self.tool_group;
        self.pigments.group.set_visible(shown.pigments);
        group.group.set_visible(shown.tool_group());
        group.group.set_title(tool.label());
        group.group.set_description(group_description);
        group.loaded_row.set_visible(shown.loaded);
        group.size.row.set_visible(shown.size);
        group.load.row.set_visible(shown.load);
        group.medium.row.set_visible(shown.medium);
        group.plate_gap.row.set_visible(shown.plate_gap);
        self.rack.group.set_visible(shown.shape);
        group.reload_row.set_visible(shown.stroke_loading);
        group.clean_each_row.set_visible(shown.stroke_loading);
        self.clean_button.set_visible(shown.clean);
        self.hint.set_visible(shown.hint);
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
