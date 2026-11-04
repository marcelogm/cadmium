use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use cadmium_engine::gpu::GpuContext;
use gtk::{gdk, gio, glib};

use crate::LOG_DOMAIN;
use crate::dialogs;
use crate::presets::starting_canvas_spec;
use crate::smoke::SmokeConfig;
use crate::state::{AppState, SharedState};
use crate::window::CadmiumWindow;

const ICON_RESOURCE_PATH: &str = "/io/github/marcelogm/Cadmium/icons";
const MISSING_GPU_HEADING: &str = "Nenhuma GPU Vulkan disponível";
const MISSING_GPU_BODY: &str = "O Cadmium precisa de uma GPU com Vulkan para simular a tinta.";
const OUT_OF_VIDEO_MEMORY_HEADING: &str = "Memória de vídeo insuficiente";

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

pub struct CadmiumApplication {
    app: adw::Application,
}

impl CadmiumApplication {
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
        app.connect_startup(start_up);
        app.connect_activate(move |app| activate(app, &smoke));
        Self { app }
    }

    pub fn run(&self) -> glib::ExitCode {
        self.app.run()
    }
}

fn start_up(app: &adw::Application) {
    register_icons();
    if let Some(app_id) = app.application_id() {
        gtk::Window::set_default_icon_name(&app_id);
    }
    setup_actions(app);
    for (action, accels) in ACCELERATORS {
        app.set_accels_for_action(action, accels);
    }
}

fn activate(app: &adw::Application, smoke: &RefCell<Option<SmokeConfig>>) {
    if let Some(window) = app.active_window() {
        window.present();
        return;
    }
    let Some(state) = start_state(app) else {
        return;
    };
    let window = CadmiumWindow::new(app, state);
    if let Some(smoke) = smoke.borrow_mut().take() {
        smoke.run(app, &window);
    }
    window.present();
}

fn start_state(app: &adw::Application) -> Option<SharedState> {
    let gpu = match GpuContext::new() {
        Ok(gpu) => Rc::new(gpu),
        Err(error) => {
            report_startup_failure(
                app,
                MISSING_GPU_HEADING,
                &format!("{MISSING_GPU_BODY}\n\n{error}"),
            );
            return None;
        }
    };
    match AppState::new(gpu, starting_canvas_spec()) {
        Ok(state) => Some(Rc::new(RefCell::new(state))),
        Err(error) => {
            report_startup_failure(app, OUT_OF_VIDEO_MEMORY_HEADING, &error.to_string());
            None
        }
    }
}

fn report_startup_failure(app: &adw::Application, heading: &str, body: &str) {
    glib::g_critical!(LOG_DOMAIN, "{heading}: {body}");
    let hold = app.hold();
    let alert = adw::AlertDialog::builder()
        .heading(heading)
        .body(body)
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
        .activate(|app: &adw::Application, _, _| present_shortcuts(app))
        .build();
    let about = gio::ActionEntry::builder("about")
        .activate(|app: &adw::Application, _, _| present_about(app))
        .build();
    let quit = gio::ActionEntry::builder("quit")
        .activate(|app: &adw::Application, _, _| close_every_window(app))
        .build();
    app.add_action_entries([shortcuts, about, quit]);
}

fn present_shortcuts(app: &adw::Application) {
    if !active_window_shows_a_dialog(app) {
        dialogs::shortcuts_dialog().present(app.active_window().as_ref());
    }
}

fn present_about(app: &adw::Application) {
    if !active_window_shows_a_dialog(app) {
        dialogs::about_dialog().present(app.active_window().as_ref());
    }
}

fn close_every_window(app: &adw::Application) {
    for window in app.windows() {
        window.close();
    }
}

fn active_window_shows_a_dialog(app: &adw::Application) -> bool {
    app.active_window()
        .and_downcast::<adw::ApplicationWindow>()
        .is_some_and(|window| window.visible_dialog().is_some())
}
