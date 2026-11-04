use std::cell::{Cell, OnceCell, RefCell};
use std::time::Duration;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};
use oleo_engine::canvas::{CanvasSpec, MAX_CANVAS_SIZE, Primer, Weave, primer_reflectance};
use oleo_engine::pigments::{MAX_SLOTS, PigmentDb};
use oleo_engine::spectral::{
    bands_to_linear_srgb, compress_gamut, linear_to_srgb8, saunderson_display,
};

use crate::swatch::{Swatch, SwatchShape};

pub const MIN_SIDE: u32 = 64;
pub const SIDE_STEP: u32 = 64;
const CONTENT_WIDTH: i32 = 560;
const COUNTER_FLASH: Duration = Duration::from_millis(700);
const CHIP_SWATCH_SIZE: i32 = 16;
const PRIMER_SWATCH_SIZE: (i32, i32) = (30, 16);
const CHIPS_PER_LINE: u32 = 3;
const CHIP_SPACING: u32 = 6;
const STARTING_SIZE: (u32, u32) = (2048, 1536);
const ACTION_GROUP: &str = "new-painting";
const LOG_DOMAIN: &str = "oleo";
const PRESET_ACTION: &str = "preset";

pub const PRIMARIES: &[&str] = &[
    "titanium_white",
    "cadmium_yellow",
    "cadmium_red",
    "quinacridone_magenta",
    "ultramarine_blue",
    "phthalo_blue",
    "burnt_umber",
    "ivory_black",
];
pub const ZORN: &[&str] = &[
    "titanium_white",
    "yellow_ochre",
    "cadmium_red",
    "ivory_black",
];
pub const EARTHS: &[&str] = &[
    "titanium_white",
    "yellow_ochre",
    "burnt_sienna",
    "burnt_umber",
    "ivory_black",
    "ultramarine_blue",
];

pub struct PigmentPreset {
    pub name: &'static str,
    pub label: &'static str,
    pub pigments: &'static [&'static str],
}

pub const PIGMENT_PRESETS: [PigmentPreset; 3] = [
    PigmentPreset {
        name: "zorn",
        label: "Paleta limitada (Zorn)",
        pigments: ZORN,
    },
    PigmentPreset {
        name: "primaries",
        label: "Primárias",
        pigments: PRIMARIES,
    },
    PigmentPreset {
        name: "earths",
        label: "Terras",
        pigments: EARTHS,
    },
];

pub const PRIMERS: [(Primer, &str); 2] = [
    (Primer::WhiteGesso, "Gesso branco"),
    (Primer::SiennaImprimatura, "Imprimatura siena"),
];

pub const WEAVES: [(Weave, &str); 3] = [
    (Weave::Linen, "Linho"),
    (Weave::Cotton, "Algodão"),
    (Weave::Smooth, "Lisa"),
];

pub fn starting_canvas_spec() -> CanvasSpec {
    CanvasSpec {
        width: STARTING_SIZE.0,
        height: STARTING_SIZE.1,
        primer: Primer::WhiteGesso,
        weave: Weave::Linen,
        pigment_slots: PRIMARIES.iter().map(|id| id.to_string()).collect(),
    }
}

pub fn primer_srgb8(db: &PigmentDb, primer: Primer) -> [u8; 3] {
    let shown = primer_reflectance(db, primer).map(saunderson_display);
    compress_gamut(bands_to_linear_srgb(&shown)).map(linear_to_srgb8)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintingFormat {
    Landscape4x3,
    Portrait3x4,
    Square,
    Landscape16x9,
    Custom,
}

impl PaintingFormat {
    pub const ALL: [PaintingFormat; 5] = [
        PaintingFormat::Landscape4x3,
        PaintingFormat::Portrait3x4,
        PaintingFormat::Square,
        PaintingFormat::Landscape16x9,
        PaintingFormat::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PaintingFormat::Landscape4x3 => "Paisagem 4:3",
            PaintingFormat::Portrait3x4 => "Retrato 3:4",
            PaintingFormat::Square => "Quadrado",
            PaintingFormat::Landscape16x9 => "Paisagem 16:9",
            PaintingFormat::Custom => "Personalizado",
        }
    }

    fn aspect(self) -> Option<(u32, u32)> {
        match self {
            PaintingFormat::Landscape4x3 => Some((4, 3)),
            PaintingFormat::Portrait3x4 => Some((3, 4)),
            PaintingFormat::Square => Some((1, 1)),
            PaintingFormat::Landscape16x9 => Some((16, 9)),
            PaintingFormat::Custom => None,
        }
    }

    pub fn size_keeping_long_side(self, width: u32, height: u32) -> Option<(u32, u32)> {
        let (aspect_width, aspect_height) = self.aspect()?;
        let long = width.max(height).clamp(MIN_SIDE, MAX_CANVAS_SIZE);
        let short = |long_part: u32, short_part: u32| {
            ((long as f64 * short_part as f64 / long_part as f64).round() as u32)
                .clamp(MIN_SIDE, MAX_CANVAS_SIZE)
        };
        Some(if aspect_width >= aspect_height {
            (long, short(aspect_width, aspect_height))
        } else {
            (short(aspect_height, aspect_width), long)
        })
    }

    pub fn matching(width: u32, height: u32) -> PaintingFormat {
        PaintingFormat::ALL
            .into_iter()
            .find(|format| format.size_keeping_long_side(width, height) == Some((width, height)))
            .unwrap_or(PaintingFormat::Custom)
    }

    fn index(self) -> u32 {
        PaintingFormat::ALL
            .iter()
            .position(|format| *format == self)
            .unwrap_or_default() as u32
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PigmentSelection {
    available: Vec<&'static str>,
    chosen: Vec<&'static str>,
}

impl PigmentSelection {
    pub fn new(db: &PigmentDb, initial: &[String]) -> Self {
        let mut selection = PigmentSelection {
            available: db.all().iter().map(|pigment| pigment.id).collect(),
            chosen: Vec::new(),
        };
        for id in initial {
            selection.set(id, true);
        }
        selection
    }

    pub fn contains(&self, id: &str) -> bool {
        self.chosen.contains(&id)
    }

    pub fn count(&self) -> usize {
        self.chosen.len()
    }

    pub fn set(&mut self, id: &str, chosen: bool) -> bool {
        let Some(&id) = self.available.iter().find(|known| **known == id) else {
            return !chosen;
        };
        match (chosen, self.contains(id)) {
            (true, true) | (false, false) => true,
            (false, true) => {
                self.chosen.retain(|kept| *kept != id);
                true
            }
            (true, false) if self.count() < MAX_SLOTS => {
                self.chosen.push(id);
                true
            }
            (true, false) => false,
        }
    }

    pub fn apply_preset(&mut self, pigments: &[&str]) {
        self.chosen.clear();
        for id in pigments {
            self.set(id, true);
        }
    }

    pub fn slots(&self) -> Vec<String> {
        self.available
            .iter()
            .filter(|id| self.contains(id))
            .map(|id| id.to_string())
            .collect()
    }
}

pub fn counter_text(count: usize) -> String {
    format!("{count} de {MAX_SLOTS}")
}

pub struct Widgets {
    format_row: adw::ComboRow,
    width_row: adw::SpinRow,
    height_row: adw::SpinRow,
    primer_row: adw::ComboRow,
    primer_swatch: Swatch,
    weave_row: adw::ComboRow,
    counter: gtk::Label,
    chips: Vec<(&'static str, gtk::ToggleButton)>,
    create_button: gtk::Button,
    primer_colours: Vec<[u8; 3]>,
}

type CreateHandler = Box<dyn Fn(CanvasSpec)>;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct NewPaintingDialog {
        pub widgets: OnceCell<Widgets>,
        pub selection: RefCell<Option<PigmentSelection>>,
        pub syncing_size: Cell<bool>,
        pub counter_flash: RefCell<Option<glib::SourceId>>,
        pub create_handlers: RefCell<Vec<CreateHandler>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NewPaintingDialog {
        const NAME: &'static str = "OleoNewPaintingDialog";
        type Type = super::NewPaintingDialog;
        type ParentType = adw::Dialog;
    }

    impl ObjectImpl for NewPaintingDialog {
        fn dispose(&self) {
            if let Some(source) = self.counter_flash.take() {
                source.remove();
            }
            self.create_handlers.borrow_mut().clear();
        }
    }

    impl WidgetImpl for NewPaintingDialog {}
    impl AdwDialogImpl for NewPaintingDialog {}
}

glib::wrapper! {
    pub struct NewPaintingDialog(ObjectSubclass<imp::NewPaintingDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

impl NewPaintingDialog {
    pub fn new(db: &PigmentDb, initial: &CanvasSpec) -> Self {
        let dialog: NewPaintingDialog = glib::Object::builder()
            .property("title", "Nova pintura")
            .property("content-width", CONTENT_WIDTH)
            .build();
        dialog
            .imp()
            .selection
            .replace(Some(PigmentSelection::new(db, &initial.pigment_slots)));
        dialog.build(db, initial);
        dialog
    }

    pub fn connect_create<F: Fn(CanvasSpec) + 'static>(&self, handler: F) {
        self.imp()
            .create_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    pub fn canvas_spec(&self) -> CanvasSpec {
        let widgets = self.widgets();
        CanvasSpec {
            width: widgets.width_row.value() as u32,
            height: widgets.height_row.value() as u32,
            primer: PRIMERS[widgets.primer_row.selected() as usize].0,
            weave: WEAVES[widgets.weave_row.selected() as usize].0,
            pigment_slots: self.with_selection(PigmentSelection::slots),
        }
    }

    fn widgets(&self) -> &Widgets {
        self.imp()
            .widgets
            .get()
            .expect("new painting dialog widgets are built in new")
    }

    fn with_selection<T>(&self, read: impl FnOnce(&PigmentSelection) -> T) -> T {
        read(
            self.imp()
                .selection
                .borrow()
                .as_ref()
                .expect("new painting dialog selection is set in new"),
        )
    }

    fn with_selection_mut<T>(&self, change: impl FnOnce(&mut PigmentSelection) -> T) -> T {
        change(
            self.imp()
                .selection
                .borrow_mut()
                .as_mut()
                .expect("new painting dialog selection is set in new"),
        )
    }

    fn build(&self, db: &PigmentDb, initial: &CanvasSpec) {
        let cancel_button = gtk::Button::with_label("Cancelar");
        let create_button = gtk::Button::builder()
            .label("Criar")
            .css_classes(["suggested-action"])
            .build();
        let header = adw::HeaderBar::builder()
            .show_start_title_buttons(false)
            .show_end_title_buttons(false)
            .build();
        header.pack_start(&cancel_button);
        header.pack_end(&create_button);

        let page = adw::PreferencesPage::new();
        let (format_row, width_row, height_row) = size_rows(initial);
        let size_group = adw::PreferencesGroup::builder().title("Tamanho").build();
        size_group.add(&format_row);
        size_group.add(&width_row);
        size_group.add(&height_row);
        page.add(&size_group);

        let primer_colours: Vec<[u8; 3]> = PRIMERS
            .iter()
            .map(|(primer, _)| primer_srgb8(db, *primer))
            .collect();
        let primer_index = PRIMERS
            .iter()
            .position(|(primer, _)| *primer == initial.primer)
            .unwrap_or_default();
        let primer_row = combo_row("Base", PRIMERS.map(|(_, label)| label), primer_index);
        let primer_swatch = Swatch::new(
            primer_colours[primer_index],
            SwatchShape::RoundedRect,
            PRIMER_SWATCH_SIZE.0,
            PRIMER_SWATCH_SIZE.1,
        );
        primer_row.add_prefix(&primer_swatch);
        let weave_index = WEAVES
            .iter()
            .position(|(weave, _)| *weave == initial.weave)
            .unwrap_or_default();
        let weave_row = combo_row(
            "Textura da tela",
            WEAVES.map(|(_, label)| label),
            weave_index,
        );
        let ground_group = adw::PreferencesGroup::builder().title("Fundo").build();
        ground_group.add(&primer_row);
        ground_group.add(&weave_row);
        page.add(&ground_group);

        let counter = gtk::Label::builder()
            .css_classes(["numeric"])
            .valign(gtk::Align::Center)
            .build();
        let presets_button = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .tooltip_text("Predefinições")
            .menu_model(&presets_menu())
            .valign(gtk::Align::Center)
            .css_classes(["flat"])
            .build();
        let pigments_suffix = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        pigments_suffix.append(&counter);
        pigments_suffix.append(&presets_button);
        let chips_box = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .homogeneous(true)
            .min_children_per_line(CHIPS_PER_LINE)
            .max_children_per_line(CHIPS_PER_LINE)
            .column_spacing(CHIP_SPACING)
            .row_spacing(CHIP_SPACING)
            .build();
        let chips: Vec<_> = db
            .all()
            .iter()
            .map(|pigment| {
                let chip = pigment_chip(
                    pigment.name,
                    pigment.colour_index,
                    db.masstone_srgb8(pigment.id),
                );
                chips_box.append(&chip);
                if let Some(child) = chip.parent() {
                    child.set_focusable(false);
                }
                (pigment.id, chip)
            })
            .collect();
        let pigments_group = adw::PreferencesGroup::builder()
            .title("Pigmentos")
            .header_suffix(&pigments_suffix)
            .build();
        pigments_group.add(&chips_box);
        page.add(&pigments_group);

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&page));
        self.set_child(Some(&toolbar_view));
        self.set_default_widget(Some(&create_button));

        let widgets = Widgets {
            format_row,
            width_row,
            height_row,
            primer_row,
            primer_swatch,
            weave_row,
            counter,
            chips,
            create_button,
            primer_colours,
        };
        if self.imp().widgets.set(widgets).is_err() {
            unreachable!("new painting dialog is built once");
        }
        self.sync_pigment_chips();
        self.connect_signals(&cancel_button);
    }

    fn connect_signals(&self, cancel_button: &gtk::Button) {
        let widgets = self.widgets();
        cancel_button.connect_clicked(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |_| {
                dialog.close();
            }
        ));
        widgets.create_button.connect_clicked(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |_| dialog.create()
        ));
        widgets.format_row.connect_selected_notify(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |_| dialog.apply_format()
        ));
        for row in [&widgets.width_row, &widgets.height_row] {
            row.connect_value_notify(glib::clone!(
                #[weak(rename_to = dialog)]
                self,
                move |_| dialog.size_edited()
            ));
        }
        widgets.primer_row.connect_selected_notify(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |row| {
                let widgets = dialog.widgets();
                if let Some(colour) = widgets.primer_colours.get(row.selected() as usize) {
                    widgets.primer_swatch.set_colour(*colour);
                }
            }
        ));
        for (id, chip) in &widgets.chips {
            let id = *id;
            chip.connect_toggled(glib::clone!(
                #[weak(rename_to = dialog)]
                self,
                move |chip| dialog.pigment_toggled(id, chip)
            ));
        }
        let preset = gio::ActionEntry::builder(PRESET_ACTION)
            .parameter_type(Some(glib::VariantTy::STRING))
            .activate(glib::clone!(
                #[weak(rename_to = dialog)]
                self,
                move |_: &gio::SimpleActionGroup, _, parameter| {
                    if let Some(name) = parameter.and_then(|value| value.str()) {
                        dialog.apply_preset(name);
                    }
                }
            ))
            .build();
        let actions = gio::SimpleActionGroup::new();
        actions.add_action_entries([preset]);
        self.insert_action_group(ACTION_GROUP, Some(&actions));
    }

    fn create(&self) {
        let spec = self.canvas_spec();
        for handler in self.imp().create_handlers.borrow().iter() {
            handler(spec.clone());
        }
        self.close();
    }

    fn apply_format(&self) {
        if self.imp().syncing_size.get() {
            return;
        }
        let widgets = self.widgets();
        let format = PaintingFormat::ALL[widgets.format_row.selected() as usize];
        let width = widgets.width_row.value() as u32;
        let height = widgets.height_row.value() as u32;
        if let Some((width, height)) = format.size_keeping_long_side(width, height) {
            self.imp().syncing_size.set(true);
            widgets.width_row.set_value(width as f64);
            widgets.height_row.set_value(height as f64);
            self.imp().syncing_size.set(false);
        }
    }

    fn size_edited(&self) {
        if self.imp().syncing_size.get() {
            return;
        }
        self.imp().syncing_size.set(true);
        self.widgets()
            .format_row
            .set_selected(PaintingFormat::Custom.index());
        self.imp().syncing_size.set(false);
    }

    fn pigment_toggled(&self, id: &str, chip: &gtk::ToggleButton) {
        let accepted = self.with_selection_mut(|selection| selection.set(id, chip.is_active()));
        if !accepted {
            chip.set_active(false);
            self.flash_counter();
        }
        self.update_counter();
    }

    fn apply_preset(&self, name: &str) {
        let Some(preset) = PIGMENT_PRESETS.iter().find(|preset| preset.name == name) else {
            glib::g_warning!(LOG_DOMAIN, "unknown pigment preset {name:?}");
            return;
        };
        self.with_selection_mut(|selection| selection.apply_preset(preset.pigments));
        self.sync_pigment_chips();
    }

    fn sync_pigment_chips(&self) {
        let widgets = self.widgets();
        for (id, chip) in &widgets.chips {
            chip.set_active(self.with_selection(|selection| selection.contains(id)));
        }
        self.update_counter();
    }

    fn update_counter(&self) {
        let count = self.with_selection(PigmentSelection::count);
        let widgets = self.widgets();
        widgets.counter.set_label(&counter_text(count));
        widgets.create_button.set_sensitive(count > 0);
    }

    fn flash_counter(&self) {
        let counter = &self.widgets().counter;
        counter.add_css_class("error");
        if let Some(source) = self.imp().counter_flash.take() {
            source.remove();
        }
        let source = glib::timeout_add_local_once(
            COUNTER_FLASH,
            glib::clone!(
                #[weak(rename_to = dialog)]
                self,
                move || {
                    dialog.imp().counter_flash.take();
                    dialog.widgets().counter.remove_css_class("error");
                }
            ),
        );
        self.imp().counter_flash.replace(Some(source));
    }
}

fn size_rows(initial: &CanvasSpec) -> (adw::ComboRow, adw::SpinRow, adw::SpinRow) {
    let format = PaintingFormat::matching(initial.width, initial.height);
    let format_row = combo_row(
        "Formato",
        PaintingFormat::ALL.map(PaintingFormat::label),
        format.index() as usize,
    );
    let side_row = |title: &str, value: u32| {
        let row =
            adw::SpinRow::with_range(MIN_SIDE as f64, MAX_CANVAS_SIZE as f64, SIDE_STEP as f64);
        row.set_title(title);
        row.set_digits(0);
        row.set_value(value as f64);
        row
    };
    (
        format_row,
        side_row("Largura", initial.width),
        side_row("Altura", initial.height),
    )
}

fn combo_row<const N: usize>(title: &str, labels: [&str; N], selected: usize) -> adw::ComboRow {
    adw::ComboRow::builder()
        .title(title)
        .model(&gtk::StringList::new(&labels))
        .selected(selected as u32)
        .build()
}

fn presets_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    for preset in &PIGMENT_PRESETS {
        let item = gio::MenuItem::new(Some(preset.label), None);
        item.set_action_and_target_value(
            Some(&format!("{ACTION_GROUP}.{PRESET_ACTION}")),
            Some(&preset.name.to_variant()),
        );
        menu.append_item(&item);
    }
    menu
}

fn pigment_chip(name: &str, colour_index: &str, colour: [u8; 3]) -> gtk::ToggleButton {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, CHIP_SPACING as i32);
    content.append(&Swatch::new(
        colour,
        SwatchShape::Circle,
        CHIP_SWATCH_SIZE,
        CHIP_SWATCH_SIZE,
    ));
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 0);
    labels.append(
        &gtk::Label::builder()
            .label(name)
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .css_classes(["pigment-name"])
            .build(),
    );
    labels.append(
        &gtk::Label::builder()
            .label(colour_index)
            .xalign(0.0)
            .css_classes(["dim-label", "caption"])
            .build(),
    );
    content.append(&labels);
    gtk::ToggleButton::builder()
        .child(&content)
        .tooltip_text(format!("{name} ({colour_index})"))
        .css_classes(["card", "pigment-chip"])
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn landscape_format_keeps_the_long_side_of_a_portrait_size() {
        assert_eq!(
            PaintingFormat::Landscape4x3.size_keeping_long_side(2304, 3072),
            Some((3072, 2304))
        );
    }

    #[test]
    fn portrait_format_puts_the_long_side_vertically() {
        assert_eq!(
            PaintingFormat::Portrait3x4.size_keeping_long_side(2048, 1536),
            Some((1536, 2048))
        );
    }

    #[test]
    fn square_and_widescreen_formats_derive_the_short_side() {
        assert_eq!(
            PaintingFormat::Square.size_keeping_long_side(2048, 1536),
            Some((2048, 2048))
        );
        assert_eq!(
            PaintingFormat::Landscape16x9.size_keeping_long_side(1536, 2048),
            Some((2048, 1152))
        );
    }

    #[test]
    fn custom_format_leaves_the_size_alone() {
        assert_eq!(
            PaintingFormat::Custom.size_keeping_long_side(1000, 700),
            None
        );
    }

    #[test]
    fn short_side_never_drops_below_the_minimum() {
        assert_eq!(
            PaintingFormat::Landscape16x9.size_keeping_long_side(64, 64),
            Some((64, 64))
        );
    }

    #[test]
    fn size_with_a_known_aspect_is_recognised_as_that_format() {
        assert_eq!(
            PaintingFormat::matching(2048, 1536),
            PaintingFormat::Landscape4x3
        );
        assert_eq!(
            PaintingFormat::matching(1536, 2048),
            PaintingFormat::Portrait3x4
        );
        assert_eq!(PaintingFormat::matching(1024, 1024), PaintingFormat::Square);
        assert_eq!(PaintingFormat::matching(1000, 700), PaintingFormat::Custom);
    }

    #[test]
    fn starting_painting_is_landscape_with_the_primaries() {
        let spec = starting_canvas_spec();
        assert_eq!(
            PaintingFormat::matching(spec.width, spec.height),
            PaintingFormat::Landscape4x3
        );
        assert!(spec.validate(&PigmentDb::builtin()).is_ok());
        assert_eq!(spec.pigment_slots.len(), MAX_SLOTS);
    }

    #[test]
    fn ninth_pigment_is_refused() {
        let db = PigmentDb::builtin();
        let mut selection = PigmentSelection::new(&db, &starting_canvas_spec().pigment_slots);
        assert!(!selection.set("zinc_white", true));
        assert!(!selection.contains("zinc_white"));
        assert_eq!(selection.count(), MAX_SLOTS);
    }

    #[test]
    fn rechoosing_a_pigment_in_a_full_selection_is_accepted() {
        let db = PigmentDb::builtin();
        let mut selection = PigmentSelection::new(&db, &starting_canvas_spec().pigment_slots);
        assert!(selection.set("titanium_white", true));
        assert_eq!(selection.count(), MAX_SLOTS);
    }

    #[test]
    fn dropping_a_pigment_frees_a_slot() {
        let db = PigmentDb::builtin();
        let mut selection = PigmentSelection::new(&db, &starting_canvas_spec().pigment_slots);
        assert!(selection.set("phthalo_blue", false));
        assert!(selection.set("zinc_white", true));
        assert_eq!(selection.count(), MAX_SLOTS);
    }

    #[test]
    fn unknown_pigment_cannot_be_chosen() {
        let db = PigmentDb::builtin();
        let mut selection = PigmentSelection::new(&db, &[]);
        assert!(!selection.set("mars_violet", true));
        assert_eq!(selection.count(), 0);
    }

    #[test]
    fn slots_follow_the_pigment_database_order() {
        let db = PigmentDb::builtin();
        let mut selection = PigmentSelection::new(&db, &[]);
        selection.set("ivory_black", true);
        selection.set("titanium_white", true);
        assert_eq!(selection.slots(), ["titanium_white", "ivory_black"]);
    }

    #[test]
    fn every_preset_yields_a_valid_canvas_spec() {
        let db = PigmentDb::builtin();
        for preset in &PIGMENT_PRESETS {
            let mut selection = PigmentSelection::new(&db, &[]);
            selection.apply_preset(preset.pigments);
            assert_eq!(selection.count(), preset.pigments.len(), "{}", preset.name);
            let spec = CanvasSpec {
                pigment_slots: selection.slots(),
                ..starting_canvas_spec()
            };
            assert!(spec.validate(&db).is_ok(), "{}", preset.name);
        }
    }

    #[test]
    fn applying_a_preset_replaces_the_selection() {
        let db = PigmentDb::builtin();
        let mut selection = PigmentSelection::new(&db, &starting_canvas_spec().pigment_slots);
        selection.apply_preset(ZORN);
        assert_eq!(
            selection.slots(),
            [
                "titanium_white",
                "yellow_ochre",
                "cadmium_red",
                "ivory_black"
            ]
        );
    }

    #[test]
    fn counter_reads_n_of_eight() {
        assert_eq!(counter_text(6), "6 de 8");
    }

    #[test]
    fn gesso_swatch_is_near_white_and_imprimatura_is_warm() {
        let db = PigmentDb::builtin();
        let gesso = primer_srgb8(&db, Primer::WhiteGesso);
        assert!(gesso.iter().all(|channel| *channel > 200), "{gesso:?}");
        let [red, green, blue] = primer_srgb8(&db, Primer::SiennaImprimatura);
        assert!(red > green && green > blue, "{:?}", [red, green, blue]);
        assert!(red < gesso[0] || blue < gesso[2]);
    }
}
