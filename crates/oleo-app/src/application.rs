use adw::prelude::*;
use gtk::{gdk, gio, glib};

use crate::smoke::SmokeConfig;
use crate::window::OleoWindow;

const LOG_DOMAIN: &str = "oleo";
const ICON_RESOURCE_PATH: &str = "/io/github/marcelo/Oleo/icons";

const ACCELERATORS: [(&str, &[&str]); 14] = [
    ("win.undo", &["<Control>z"]),
    ("win.redo", &["<Control><Shift>z"]),
    ("win.dry", &["<Control>d"]),
    ("win.new", &["<Control>n"]),
    ("win.open", &["<Control>o"]),
    ("win.save", &["<Control>s"]),
    ("win.save-as", &["<Control><Shift>s"]),
    ("win.export", &["<Control>e"]),
    ("win.zoom-in", &["<Control>plus", "<Control>equal"]),
    ("win.zoom-out", &["<Control>minus"]),
    ("win.zoom-fit", &["<Control>0"]),
    ("win.toggle-sidebar", &["F9"]),
    ("app.shortcuts", &["<Control>question"]),
    ("app.quit", &["<Control>q"]),
];

pub struct OleoApplication {
    app: adw::Application,
}

impl OleoApplication {
    pub fn new(app_id: &str) -> Self {
        let app = adw::Application::builder().application_id(app_id).build();
        app.connect_startup(|app| {
            register_icons();
            setup_actions(app);
            for (action, accels) in ACCELERATORS {
                app.set_accels_for_action(action, accels);
            }
        });
        app.connect_activate(|app| {
            let window = OleoWindow::new(app);
            if let Some(smoke) = SmokeConfig::from_env() {
                smoke.run(app, &window);
            }
            window.present();
        });
        Self { app }
    }

    pub fn run(&self) -> glib::ExitCode {
        self.app.run()
    }
}

fn register_icons() {
    if let Some(display) = gdk::Display::default() {
        gtk::IconTheme::for_display(&display).add_resource_path(ICON_RESOURCE_PATH);
    }
}

fn setup_actions(app: &adw::Application) {
    let pending = ["shortcuts", "about"].map(|name| {
        gio::ActionEntry::builder(name)
            .activate(move |_: &adw::Application, _, _| {
                glib::g_debug!(LOG_DOMAIN, "app.{name} is not wired yet");
            })
            .build()
    });
    let quit = gio::ActionEntry::builder("quit")
        .activate(|app: &adw::Application, _, _| app.quit())
        .build();
    app.add_action_entries(pending.into_iter().chain([quit]));
}
