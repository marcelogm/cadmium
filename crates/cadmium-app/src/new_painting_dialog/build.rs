use adw::prelude::*;
use adw::subclass::prelude::*;
use cadmium_engine::canvas::{CanvasSpec, MAX_CANVAS_SIZE, primer_srgb8};
use cadmium_engine::pigments::PigmentDb;
use gtk::gio;

use super::format::{MIN_SIDE, PaintingFormat, SIDE_STEP};
use super::{NewPaintingDialog, PRIMERS, WEAVES, Widgets};
use crate::presets::PIGMENT_PRESETS;
use crate::swatch::{Swatch, SwatchShape};

const CHIP_SWATCH_SIZE: i32 = 16;
const PRIMER_SWATCH_SIZE: (i32, i32) = (30, 16);
const CHIPS_PER_LINE: u32 = 3;
const CHIP_SPACING: u32 = 6;
pub(super) const ACTION_GROUP: &str = "new-painting";
pub(super) const PRESET_ACTION: &str = "preset";

impl NewPaintingDialog {
    pub(super) fn build(&self, db: &PigmentDb, initial: &CanvasSpec) {
        let (header, cancel_button, create_button) = header_bar();
        let page = adw::PreferencesPage::new();
        let (format_row, width_row, height_row) = size_section(&page, initial);
        let ground = ground_section(&page, db, initial);
        let (counter, chips) = pigments_section(&page, db);
        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&page));
        self.set_child(Some(&toolbar_view));
        self.set_default_widget(Some(&create_button));
        let widgets = Widgets {
            format_row,
            width_row,
            height_row,
            primer_row: ground.primer_row,
            primer_swatch: ground.primer_swatch,
            weave_row: ground.weave_row,
            counter,
            chips,
            create_button,
            primer_colours: ground.primer_colours,
        };
        if self.imp().widgets.set(widgets).is_err() {
            unreachable!("new painting dialog is built once");
        }
        self.sync_pigment_chips();
        self.connect_signals(&cancel_button);
    }
}

struct GroundRows {
    primer_row: adw::ComboRow,
    primer_swatch: Swatch,
    weave_row: adw::ComboRow,
    primer_colours: Vec<[u8; 3]>,
}

fn header_bar() -> (adw::HeaderBar, gtk::Button, gtk::Button) {
    let cancel_button = gtk::Button::with_label("Cancel");
    let create_button = gtk::Button::builder()
        .label("Create")
        .css_classes(["suggested-action"])
        .build();
    let header = adw::HeaderBar::builder()
        .show_start_title_buttons(false)
        .show_end_title_buttons(false)
        .build();
    header.pack_start(&cancel_button);
    header.pack_end(&create_button);
    (header, cancel_button, create_button)
}

fn size_section(
    page: &adw::PreferencesPage,
    initial: &CanvasSpec,
) -> (adw::ComboRow, adw::SpinRow, adw::SpinRow) {
    let (format_row, width_row, height_row) = size_rows(initial);
    let size_group = adw::PreferencesGroup::builder().title("Size").build();
    size_group.add(&format_row);
    size_group.add(&width_row);
    size_group.add(&height_row);
    page.add(&size_group);
    (format_row, width_row, height_row)
}

fn ground_section(page: &adw::PreferencesPage, db: &PigmentDb, initial: &CanvasSpec) -> GroundRows {
    let (primer_row, primer_swatch, primer_colours) = primer_combo(db, initial);
    let weave_row = weave_combo(initial);
    let ground_group = adw::PreferencesGroup::builder().title("Ground").build();
    ground_group.add(&primer_row);
    ground_group.add(&weave_row);
    page.add(&ground_group);
    GroundRows {
        primer_row,
        primer_swatch,
        weave_row,
        primer_colours,
    }
}

fn primer_combo(db: &PigmentDb, initial: &CanvasSpec) -> (adw::ComboRow, Swatch, Vec<[u8; 3]>) {
    let primer_colours: Vec<[u8; 3]> = PRIMERS
        .iter()
        .map(|(primer, _)| primer_srgb8(db, *primer))
        .collect();
    let primer_index = PRIMERS
        .iter()
        .position(|(primer, _)| *primer == initial.primer)
        .unwrap_or_default();
    let primer_row = combo_row("Primer", PRIMERS.map(|(_, label)| label), primer_index);
    let primer_swatch = Swatch::new(
        primer_colours[primer_index],
        SwatchShape::RoundedRect,
        PRIMER_SWATCH_SIZE.0,
        PRIMER_SWATCH_SIZE.1,
    );
    primer_row.add_prefix(&primer_swatch);
    (primer_row, primer_swatch, primer_colours)
}

fn weave_combo(initial: &CanvasSpec) -> adw::ComboRow {
    let weave_index = WEAVES
        .iter()
        .position(|(weave, _)| *weave == initial.weave)
        .unwrap_or_default();
    combo_row(
        "Canvas Texture",
        WEAVES.map(|(_, label)| label),
        weave_index,
    )
}

fn pigments_section(
    page: &adw::PreferencesPage,
    db: &PigmentDb,
) -> (gtk::Label, Vec<(&'static str, gtk::ToggleButton)>) {
    let counter = gtk::Label::builder()
        .css_classes(["numeric"])
        .valign(gtk::Align::Center)
        .build();
    let presets_button = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .tooltip_text("Presets")
        .menu_model(&presets_menu())
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .build();
    let pigments_suffix = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    pigments_suffix.append(&counter);
    pigments_suffix.append(&presets_button);
    let (chips_box, chips) = pigment_chips(db);
    let pigments_group = adw::PreferencesGroup::builder()
        .title("Pigments")
        .header_suffix(&pigments_suffix)
        .build();
    pigments_group.add(&chips_box);
    page.add(&pigments_group);
    (counter, chips)
}

fn pigment_chips(db: &PigmentDb) -> (gtk::FlowBox, Vec<(&'static str, gtk::ToggleButton)>) {
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
    (chips_box, chips)
}

fn size_rows(initial: &CanvasSpec) -> (adw::ComboRow, adw::SpinRow, adw::SpinRow) {
    let format = PaintingFormat::matching(initial.width, initial.height);
    let format_row = combo_row(
        "Format",
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
        side_row("Width", initial.width),
        side_row("Height", initial.height),
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
