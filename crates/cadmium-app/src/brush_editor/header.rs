//! The editor's header: Cancel, the brush's name (the dialog title), the ⋯ menu and Save.

use gtk::gio;

/// The action group the ⋯ menu's items live in: `editor.save-copy`, `editor.export`,
/// `editor.delete`.
pub(super) const ACTION_GROUP: &str = "editor";

pub(super) struct Header {
    pub(super) bar: adw::HeaderBar,
    pub(super) cancel: gtk::Button,
    pub(super) save: gtk::Button,
}

pub(super) fn header() -> Header {
    let cancel = gtk::Button::with_label("Cancel");
    let save = gtk::Button::builder()
        .label("Save")
        .css_classes(["suggested-action"])
        .build();
    let more = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .tooltip_text("More")
        .menu_model(&more_menu())
        .build();
    let bar = adw::HeaderBar::builder()
        .show_start_title_buttons(false)
        .show_end_title_buttons(false)
        .build();
    bar.pack_start(&cancel);
    bar.pack_end(&save);
    bar.pack_end(&more);
    Header { bar, cancel, save }
}

fn more_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(Some("Save as Copy"), Some("editor.save-copy"));
    menu.append(Some("Export…"), Some("editor.export"));
    let removal = gio::Menu::new();
    removal.append(Some("Delete"), Some("editor.delete"));
    menu.append_section(None, &removal);
    menu
}
