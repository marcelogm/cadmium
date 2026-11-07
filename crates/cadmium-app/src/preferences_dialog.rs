use adw::prelude::*;
use gtk::glib;

use crate::LOG_DOMAIN;
use crate::settings::{PenButtonAction, Settings};
use crate::state::SharedState;

mod pressure;

const SAVE_FAILED: &str =
    "Couldn't save the preferences; they'll be back to how they were next time";

/// The app's Preferences dialog; each change applies and is saved at once.
pub fn preferences_dialog(state: &SharedState) -> adw::PreferencesDialog {
    let dialog = adw::PreferencesDialog::builder()
        .title("Preferences")
        .build();
    let page = adw::PreferencesPage::new();
    let group = adw::PreferencesGroup::builder()
        .title("Pen Buttons")
        .description("What the pen's side buttons do on the canvas")
        .build();
    let settings = state.borrow().settings.get();
    group.add(&button_row(
        state,
        &dialog,
        ("Lower Button", "Nearer the tip"),
        settings.lower_pen_button,
        |settings, action| settings.lower_pen_button = action,
    ));
    group.add(&button_row(
        state,
        &dialog,
        ("Upper Button", "Further from the tip"),
        settings.upper_pen_button,
        |settings, action| settings.upper_pen_button = action,
    ));
    page.add(&group);
    page.add(&pressure::pressure_group(
        settings.pressure_curve,
        glib::clone!(
            #[strong]
            state,
            #[weak_allow_none]
            dialog,
            move |curve| {
                save(&state, dialog.as_ref(), |settings| {
                    settings.pressure_curve = curve;
                });
            }
        ),
    ));
    dialog.add(&page);
    dialog
}

/// Applies `change` to the settings and saves them, even if the dialog is gone; a failed save
/// is told in a toast when it's still there.
fn save(
    state: &SharedState,
    dialog: Option<&adw::PreferencesDialog>,
    change: impl FnOnce(&mut Settings),
) {
    if let Err(error) = state.borrow_mut().settings.update(change) {
        glib::g_warning!(LOG_DOMAIN, "couldn't save the settings: {error}");
        if let Some(dialog) = dialog {
            dialog.add_toast(adw::Toast::new(SAVE_FAILED));
        }
    }
}

/// A row choosing the action of one pen button; `bind` stores the choice in the settings.
fn button_row(
    state: &SharedState,
    dialog: &adw::PreferencesDialog,
    (title, subtitle): (&str, &str),
    current: PenButtonAction,
    bind: fn(&mut Settings, PenButtonAction),
) -> adw::ComboRow {
    let labels: Vec<&str> = PenButtonAction::ALL
        .iter()
        .map(|action| action.label())
        .collect();
    let row = adw::ComboRow::builder()
        .title(title)
        .subtitle(subtitle)
        .model(&gtk::StringList::new(&labels))
        .build();
    row.set_selected(current.menu_index());
    row.connect_selected_notify(glib::clone!(
        #[strong]
        state,
        #[weak_allow_none]
        dialog,
        move |row| {
            let Some(action) = PenButtonAction::at_menu_index(row.selected()) else {
                return;
            };
            save(&state, dialog.as_ref(), |settings| bind(settings, action));
        }
    ));
    row
}
