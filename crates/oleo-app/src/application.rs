use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use oleo_engine::gpu::GpuContext;

use crate::LOG_DOMAIN;
use crate::dialogs;
use crate::presets::starting_canvas_spec;
use crate::smoke::SmokeConfig;
use crate::state::AppState;
use crate::window::OleoWindow;

const ICON_RESOURCE_PATH: &str = "/io/github/marcelo/Oleo/icons";

pub const ACCELERATORS: [(&str, &[&str]); 14] = [
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
        let smoke = SmokeConfig::from_env();
        let flags = if smoke.is_some() {
            gio::ApplicationFlags::NON_UNIQUE
        } else {
            gio::ApplicationFlags::empty()
        };
        let app = adw::Application::builder()
            .application_id(app_id)
            .flags(flags)
            .build();
        let smoke = RefCell::new(smoke);
        app.connect_startup(|app| {
            register_icons();
            setup_actions(app);
            for (action, accels) in ACCELERATORS {
                app.set_accels_for_action(action, accels);
            }
        });
        app.connect_activate(move |app| {
            if let Some(window) = app.active_window() {
                window.present();
                return;
            }
            let gpu = match GpuContext::new() {
                Ok(gpu) => Rc::new(gpu),
                Err(error) => {
                    report_missing_gpu(app, &error);
                    return;
                }
            };
            let state = Rc::new(RefCell::new(AppState::new(gpu, starting_canvas_spec())));
            let window = OleoWindow::new(app, state);
            if let Some(smoke) = smoke.borrow_mut().take() {
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

fn report_missing_gpu(app: &adw::Application, error: &oleo_engine::gpu::GpuError) {
    glib::g_critical!(LOG_DOMAIN, "no usable GPU: {error}");
    let hold = app.hold();
    let alert = adw::AlertDialog::builder()
        .heading("Nenhuma GPU Vulkan disponível")
        .body(format!(
            "O Óleo precisa de uma GPU com Vulkan para simular a tinta.\n\n{error}"
        ))
        .default_response("close")
        .close_response("close")
        .build();
    alert.add_response("close", "Sair");
    alert.choose(
        None::<&gtk::Widget>,
        None::<&gio::Cancellable>,
        glib::clone!(
            #[weak]
            app,
            move |_| {
                drop(hold);
                app.quit();
            }
        ),
    );
}

fn register_icons() {
    if let Some(display) = gdk::Display::default() {
        gtk::IconTheme::for_display(&display).add_resource_path(ICON_RESOURCE_PATH);
    }
}

fn setup_actions(app: &adw::Application) {
    let shortcuts = gio::ActionEntry::builder("shortcuts")
        .activate(|app: &adw::Application, _, _| {
            if !active_window_shows_a_dialog(app) {
                dialogs::shortcuts_dialog().present(app.active_window().as_ref());
            }
        })
        .build();
    let about = gio::ActionEntry::builder("about")
        .activate(|app: &adw::Application, _, _| {
            if !active_window_shows_a_dialog(app) {
                dialogs::about_dialog().present(app.active_window().as_ref());
            }
        })
        .build();
    let quit = gio::ActionEntry::builder("quit")
        .activate(|app: &adw::Application, _, _| app.quit())
        .build();
    app.add_action_entries([shortcuts, about, quit]);
}

fn active_window_shows_a_dialog(app: &adw::Application) -> bool {
    app.active_window()
        .and_downcast::<adw::ApplicationWindow>()
        .is_some_and(|window| window.visible_dialog().is_some())
}
