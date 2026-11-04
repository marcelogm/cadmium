mod application;
mod dialogs;
mod new_painting_dialog;
mod presets;
mod smoke;
mod swatch;
mod tools;
mod window;

const APP_ID: &str = "io.github.marcelo.Oleo";
pub const LOG_DOMAIN: &str = "oleo";

fn main() -> gtk::glib::ExitCode {
    gtk::gio::resources_register_include!("oleo.gresource").expect("resources");
    let app = application::OleoApplication::new(APP_ID);
    app.run()
}
