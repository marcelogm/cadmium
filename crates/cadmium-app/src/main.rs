mod application;
mod brush_panel;
mod canvas_widget;
mod dialogs;
mod dmabuf;
mod dmabuf_selftest;
mod files;
mod input;
mod lab_panel;
mod new_painting_dialog;
mod presenter;
mod presets;
mod smoke;
mod state;
mod swatch;
mod tools;
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
