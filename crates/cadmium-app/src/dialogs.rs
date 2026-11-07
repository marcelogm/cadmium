use adw::prelude::*;
use gtk::glib;

use crate::application::ACCELERATORS;
use crate::input::nav_drag::NavMode;
use crate::tools::UiTool;

const APPLICATION_ICON: &str = "io.github.marcelogm.Cadmium";
const PIGMENT_DATA_CREDIT: &str =
    "Provisional spectra, hand-tuned from each pigment's known appearance; not measurements";

/// The hold-and-drag gestures, with the keys held for each and the mode they start.
const NAVIGATION_GESTURES: [(&str, &str, NavMode); 3] = [
    ("Move Canvas", "space", NavMode::Pan),
    ("Zoom Canvas", "<Control>", NavMode::Zoom),
    ("Rotate Canvas", "<Shift>", NavMode::Rotate),
];

pub const ACTION_SHORTCUTS: [(&str, &[(&str, &str)]); 3] = [
    (
        "Painting",
        &[
            ("Undo", "win.undo"),
            ("Redo", "win.redo"),
            ("Dry", "win.dry"),
        ],
    ),
    (
        "File",
        &[
            ("New Painting", "win.new"),
            ("Open", "win.open"),
            ("Save", "win.save"),
            ("Save As", "win.save-as"),
            ("Export PNG", "win.export"),
            ("Quit", "app.quit"),
        ],
    ),
    (
        "View",
        &[
            ("Zoom In", "win.zoom-in"),
            ("Zoom Out", "win.zoom-out"),
            ("Fit to Window", "win.zoom-fit"),
            ("Rotate Left", "win.rotate-left"),
            ("Rotate Right", "win.rotate-right"),
            ("Show or Hide Sidebar", "win.toggle-sidebar"),
            ("Show or Hide Tray", "win.toggle-tray"),
            ("Preferences", "win.preferences"),
            ("Keyboard Shortcuts", "app.shortcuts"),
        ],
    ),
];

pub fn action_accelerator(action: &str) -> Option<String> {
    ACCELERATORS
        .iter()
        .find(|(name, _)| *name == action)
        .map(|(_, accels)| accels.join(" "))
}

pub fn tool_accelerator(tool: UiTool) -> String {
    tool.profile()
        .keys
        .iter()
        .filter_map(|key| key.name())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn about_dialog() -> adw::AboutDialog {
    let about = adw::AboutDialog::builder()
        .application_name("Cadmium")
        .application_icon(APPLICATION_ICON)
        .version(env!("CARGO_PKG_VERSION"))
        .comments("Oil painting simulator with spectral pigments")
        .developer_name("Marcelo")
        .license_type(gtk::License::Unknown)
        .build();
    about.add_credit_section(Some("Pigment Data"), &[PIGMENT_DATA_CREDIT]);
    about
}

pub fn shortcuts_dialog() -> adw::ShortcutsDialog {
    let dialog = adw::ShortcutsDialog::new();
    dialog.add(tool_shortcuts());
    for (title, entries) in ACTION_SHORTCUTS {
        dialog.add(action_shortcuts(title, entries));
    }
    dialog.add(navigation_shortcuts());
    dialog
}

/// The Canvas Navigation section of the shortcuts dialog.
fn navigation_shortcuts() -> adw::ShortcutsSection {
    let section = adw::ShortcutsSection::new(Some("Canvas Navigation"));
    for (label, keys, _) in NAVIGATION_GESTURES {
        let item = adw::ShortcutsItem::new(label, keys);
        item.set_subtitle("Hold and drag");
        section.add(item);
    }
    section
}

fn tool_shortcuts() -> adw::ShortcutsSection {
    let tools = adw::ShortcutsSection::new(Some("Tools"));
    for tool in UiTool::ALL {
        let item = adw::ShortcutsItem::new(tool.label(), &tool_accelerator(tool));
        if let Some(handling) = tool.handling() {
            item.set_subtitle(handling);
        }
        tools.add(item);
    }
    tools
}

fn action_shortcuts(title: &str, entries: &[(&str, &str)]) -> adw::ShortcutsSection {
    let section = adw::ShortcutsSection::new(Some(title));
    for (label, action) in entries {
        if let Some(accelerator) = action_accelerator(action) {
            section.add(adw::ShortcutsItem::new(label, &accelerator));
        }
    }
    section
}

const DISCARD_RESPONSE: &str = "discard";
const CANCEL_RESPONSE: &str = "cancel";
const DISCARD_PAINTING_HEADING: &str = "Discard the Current Painting?";
const DISCARD_UNSAVED_HEADING: &str = "Discard Unsaved Changes?";
const DISCARD_BRUSH_HEADING: &str = "Discard Changes?";

fn discard_alert(heading: &str) -> adw::AlertDialog {
    let alert = adw::AlertDialog::builder()
        .heading(heading)
        .body("Unsaved changes will be lost.")
        .default_response(CANCEL_RESPONSE)
        .close_response(CANCEL_RESPONSE)
        .build();
    alert.add_responses(&[(CANCEL_RESPONSE, "Cancel"), (DISCARD_RESPONSE, "Discard")]);
    alert.set_response_appearance(DISCARD_RESPONSE, adw::ResponseAppearance::Destructive);
    alert
}

pub fn confirm_discard(parent: &impl IsA<gtk::Widget>, on_discard: impl Fn() + 'static) {
    ask_to_discard(DISCARD_PAINTING_HEADING, parent, on_discard);
}

/// Asks before the brush editor drops its changes; runs `on_discard` if the painter agrees.
/// Returns the alert, so the editor can close it if it is closed meanwhile.
pub fn confirm_discard_brush_changes(
    parent: &impl IsA<gtk::Widget>,
    on_discard: impl Fn() + 'static,
) -> adw::AlertDialog {
    ask_to_discard(DISCARD_BRUSH_HEADING, parent, on_discard)
}

fn ask_to_discard(
    heading: &str,
    parent: &impl IsA<gtk::Widget>,
    on_discard: impl Fn() + 'static,
) -> adw::AlertDialog {
    let alert = discard_alert(heading);
    alert.clone().choose(
        Some(parent),
        None::<&gtk::gio::Cancellable>,
        move |response: glib::GString| {
            if response == DISCARD_RESPONSE {
                on_discard();
            }
        },
    );
    alert
}

pub async fn painting_discard_confirmed(parent: &impl IsA<gtk::Widget>) -> bool {
    discard_alert(DISCARD_PAINTING_HEADING)
        .choose_future(Some(parent))
        .await
        == DISCARD_RESPONSE
}

pub async fn unsaved_changes_discard_confirmed(parent: &impl IsA<gtk::Widget>) -> bool {
    discard_alert(DISCARD_UNSAVED_HEADING)
        .choose_future(Some(parent))
        .await
        == DISCARD_RESPONSE
}

const REPLACE_RESPONSE: &str = "replace";
const WAIT_RESPONSE: &str = "wait";
const QUIT_ANYWAY_RESPONSE: &str = "quit-anyway";
const STILL_WRITING_HEADING: &str = "Still Saving";
const STILL_WRITING_BODY: &str =
    "A file is taking a while to save. Quitting now may leave it incomplete.";

pub async fn overwrite_confirmed(parent: &impl IsA<gtk::Widget>, file_name: &str) -> bool {
    let alert = adw::AlertDialog::builder()
        .heading(format!("Replace {file_name}?"))
        .body("A file with this name already exists. Replacing it will overwrite its contents.")
        .default_response(CANCEL_RESPONSE)
        .close_response(CANCEL_RESPONSE)
        .build();
    alert.add_responses(&[(CANCEL_RESPONSE, "Cancel"), (REPLACE_RESPONSE, "Replace")]);
    alert.set_response_appearance(REPLACE_RESPONSE, adw::ResponseAppearance::Destructive);
    alert.choose_future(Some(parent)).await == REPLACE_RESPONSE
}

const DELETE_RESPONSE: &str = "delete";

pub async fn delete_brush_confirmed(parent: &impl IsA<gtk::Widget>, name: &str) -> bool {
    deletion_confirmed(delete_brush_alert(name), parent).await
}

/// Presents `alert` (from `delete_brush_alert`) on `parent`; true when the painter deletes.
pub async fn deletion_confirmed(alert: adw::AlertDialog, parent: &impl IsA<gtk::Widget>) -> bool {
    alert.choose_future(Some(parent)).await == DELETE_RESPONSE
}

/// "Delete <name>?", not yet presented.
pub fn delete_brush_alert(name: &str) -> adw::AlertDialog {
    let alert = adw::AlertDialog::builder()
        .heading(format!("Delete {name}?"))
        .body("The brush is removed from your brush library. This can't be undone.")
        .default_response(CANCEL_RESPONSE)
        .close_response(CANCEL_RESPONSE)
        .build();
    alert.add_responses(&[(CANCEL_RESPONSE, "Cancel"), (DELETE_RESPONSE, "Delete")]);
    alert.set_response_appearance(DELETE_RESPONSE, adw::ResponseAppearance::Destructive);
    alert
}

const CLOSE_RESPONSE: &str = "close";
const GPU_LOST_HEADING: &str = "Painting Lost After a GPU Reset";

pub fn announce_gpu_lost(parent: &impl IsA<gtk::Widget>, on_close: impl Fn() + 'static) {
    let alert = adw::AlertDialog::builder()
        .heading(GPU_LOST_HEADING)
        .default_response(CLOSE_RESPONSE)
        .close_response(CLOSE_RESPONSE)
        .build();
    alert.add_responses(&[(CLOSE_RESPONSE, "Close")]);
    alert.choose(
        Some(parent),
        None::<&gtk::gio::Cancellable>,
        move |_: glib::GString| on_close(),
    );
}

pub fn ask_to_quit_while_writing(
    parent: &impl IsA<gtk::Widget>,
    on_quit: impl Fn() + 'static,
) -> adw::AlertDialog {
    let alert = adw::AlertDialog::builder()
        .heading(STILL_WRITING_HEADING)
        .body(STILL_WRITING_BODY)
        .default_response(WAIT_RESPONSE)
        .close_response(WAIT_RESPONSE)
        .build();
    alert.add_responses(&[
        (WAIT_RESPONSE, "Wait"),
        (QUIT_ANYWAY_RESPONSE, "Quit Anyway"),
    ]);
    alert.set_response_appearance(QUIT_ANYWAY_RESPONSE, adw::ResponseAppearance::Destructive);
    alert.clone().choose(
        Some(parent),
        None::<&gtk::gio::Cancellable>,
        move |response: glib::GString| {
            if response == QUIT_ANYWAY_RESPONSE {
                on_quit();
            }
        },
    );
    alert
}

#[cfg(test)]
mod tests;
