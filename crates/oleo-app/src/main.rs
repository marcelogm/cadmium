mod application;
mod smoke;
mod tools;
mod window;

const APP_ID: &str = "io.github.marcelo.Oleo";

fn main() -> gtk::glib::ExitCode {
    gtk::gio::resources_register_include!("oleo.gresource").expect("resources");
    let app = application::OleoApplication::new(APP_ID);
    app.run()
}
