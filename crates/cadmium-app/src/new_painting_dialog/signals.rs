use std::time::Duration;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};

use super::NewPaintingDialog;
use super::build::{ACTION_GROUP, PRESET_ACTION};
use super::format::PaintingFormat;
use super::selection::{PigmentSelection, counter_text};
use crate::LOG_DOMAIN;
use crate::presets::PIGMENT_PRESETS;

const COUNTER_FLASH: Duration = Duration::from_millis(700);

impl NewPaintingDialog {
    pub(super) fn connect_signals(&self, cancel_button: &gtk::Button) {
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
        self.connect_size_rows();
        widgets.primer_row.connect_selected_notify(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |row| dialog.show_primer_colour(row)
        ));
        self.connect_pigment_chips();
        self.insert_preset_actions();
    }

    fn connect_size_rows(&self) {
        let widgets = self.widgets();
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
    }

    fn connect_pigment_chips(&self) {
        for (id, chip) in &self.widgets().chips {
            let id = *id;
            chip.connect_toggled(glib::clone!(
                #[weak(rename_to = dialog)]
                self,
                move |chip| dialog.pigment_toggled(id, chip)
            ));
        }
    }

    fn insert_preset_actions(&self) {
        let preset = gio::ActionEntry::builder(PRESET_ACTION)
            .parameter_type(Some(glib::VariantTy::STRING))
            .activate(glib::clone!(
                #[weak(rename_to = dialog)]
                self,
                move |_: &gio::SimpleActionGroup, _, parameter| dialog
                    .apply_preset_named(parameter)
            ))
            .build();
        let actions = gio::SimpleActionGroup::new();
        actions.add_action_entries([preset]);
        self.insert_action_group(ACTION_GROUP, Some(&actions));
    }

    fn show_primer_colour(&self, row: &adw::ComboRow) {
        let widgets = self.widgets();
        if let Some(colour) = widgets.primer_colours.get(row.selected() as usize) {
            widgets.primer_swatch.set_colour(*colour);
        }
    }

    fn apply_preset_named(&self, parameter: Option<&glib::Variant>) {
        if let Some(name) = parameter.and_then(|value| value.str()) {
            self.apply_preset(name);
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

    pub(super) fn sync_pigment_chips(&self) {
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
