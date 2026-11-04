use std::cell::Cell;

use adw::prelude::*;
use gtk::glib;

use crate::window::OleoWindow;

const SMOKE_EXIT_AFTER_FRAMES: &str = "OLEO_SMOKE_EXIT_AFTER_FRAMES";

pub struct OleoApplication {
    app: adw::Application,
}

impl OleoApplication {
    pub fn new(app_id: &str) -> Self {
        let app = adw::Application::builder().application_id(app_id).build();
        app.connect_activate(|app| {
            let window = OleoWindow::new(app);
            if let Some(frames) = smoke_exit_after_frames() {
                quit_after_frames(app, &window, frames);
            }
            window.present();
        });
        Self { app }
    }

    pub fn run(&self) -> glib::ExitCode {
        self.app.run()
    }
}

fn smoke_exit_after_frames() -> Option<u32> {
    std::env::var(SMOKE_EXIT_AFTER_FRAMES).ok()?.parse().ok()
}

fn quit_after_frames(app: &adw::Application, window: &OleoWindow, frames: u32) {
    let app = app.downgrade();
    let drawn = Cell::new(0u32);
    window.add_tick_callback(move |_, _| {
        drawn.set(drawn.get() + 1);
        if drawn.get() < frames {
            return glib::ControlFlow::Continue;
        }
        if let Some(app) = app.upgrade() {
            app.quit();
        }
        glib::ControlFlow::Break
    });
}
