use std::cell::{Cell, OnceCell, RefCell};
use std::time::Duration;

use adw::prelude::*;
use adw::subclass::prelude::*;
use cadmium_engine::canvas::{CanvasSpec, MAX_CANVAS_SIZE, Primer, Weave, primer_srgb8};
use cadmium_engine::pigments::PigmentDb;
use gtk::{gio, glib};

use crate::LOG_DOMAIN;
use crate::presets::PIGMENT_PRESETS;
use crate::swatch::{Swatch, SwatchShape};

mod format;
mod imp;
mod selection;

use format::{MIN_SIDE, PaintingFormat, SIDE_STEP};
use selection::{PigmentSelection, counter_text};

const CONTENT_WIDTH: i32 = 560;
const COUNTER_FLASH: Duration = Duration::from_millis(700);
const CHIP_SWATCH_SIZE: i32 = 16;
const PRIMER_SWATCH_SIZE: (i32, i32) = (30, 16);
const CHIPS_PER_LINE: u32 = 3;
const CHIP_SPACING: u32 = 6;
const ACTION_GROUP: &str = "new-painting";
const PRESET_ACTION: &str = "preset";

pub const PRIMERS: [(Primer, &str); 2] = [
    (Primer::WhiteGesso, "Gesso branco"),
    (Primer::SiennaImprimatura, "Imprimatura siena"),
];

pub const WEAVES: [(Weave, &str); 3] = [
    (Weave::Linen, "Linho"),
    (Weave::Cotton, "Algodão"),
    (Weave::Smooth, "Lisa"),
];

struct Widgets {
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

type CreateHandler = Box<dyn Fn(&NewPaintingDialog, CanvasSpec)>;

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

    pub fn connect_create<F: Fn(&NewPaintingDialog, CanvasSpec) + 'static>(&self, handler: F) {
        self.imp()
            .create_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    pub fn canvas_spec(&self) -> CanvasSpec {
        let widgets = self.widgets();
        widgets.width_row.update();
        widgets.height_row.update();
        CanvasSpec {
            width: widgets.width_row.value() as u32,
            height: widgets.height_row.value() as u32,
            primer: PRIMERS
                .get(widgets.primer_row.selected() as usize)
                .map_or(Primer::WhiteGesso, |(primer, _)| *primer),
            weave: WEAVES
                .get(widgets.weave_row.selected() as usize)
                .map_or(Weave::Linen, |(weave, _)| *weave),
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

    #[expect(clippy::excessive_nesting, clippy::too_many_lines)]
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

    #[expect(clippy::too_many_lines)]
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
            handler(self, spec.clone());
        }
    }

    fn apply_format(&self) {
        if self.imp().syncing_size.get() {
            return;
        }
        let widgets = self.widgets();
        let format = PaintingFormat::ALL
            .get(widgets.format_row.selected() as usize)
            .copied()
            .unwrap_or(PaintingFormat::Custom);
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
