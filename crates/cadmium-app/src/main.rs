#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod application;
mod atomic_file;
mod brush_editor;
mod brush_files;
mod brush_library;
mod brush_panel;
mod canvas_widget;
mod cursor_outline;
mod dialogs;
#[cfg_attr(not(target_os = "linux"), path = "dmabuf_off.rs")]
mod dmabuf;
#[cfg(all(target_os = "linux", any(debug_assertions, feature = "smoke")))]
mod dmabuf_selftest;
mod files;
mod handlers;
mod ink;
mod input;
mod lab_panel;
mod new_painting_dialog;
mod preferences_dialog;
mod presenter;
mod presets;
mod provisional_overlay;
mod settings;
mod side_panel;
mod sliders;
#[cfg_attr(
    not(all(target_os = "linux", any(debug_assertions, feature = "smoke"))),
    path = "smoke_off.rs"
)]
mod smoke;
mod state;
mod surface_scale;
mod swatch;
#[cfg(test)]
mod test_dir;
mod thumbnails;
mod tools;
mod tray_view;
mod view;
mod window;
mod writes;

const APP_ID: &str = "io.github.marcelogm.Cadmium";
pub const LOG_DOMAIN: &str = "cadmium";

fn main() -> gtk::glib::ExitCode {
    gtk::gio::resources_register_include!("cadmium.gresource").expect("resources");
    let app = application::CadmiumApplication::new(APP_ID);
    app.run()
}
