use std::cell::{Cell, RefCell};
use std::path::PathBuf;

use adw::prelude::*;
use gtk::glib;

use crate::dmabuf::ModifierPolicy;
use crate::dmabuf_selftest::{DmabufSelftest, SelftestProgress};
use crate::tools::UiTool;
use crate::window::CadmiumWindow;

mod config;
mod steps;
#[cfg(test)]
mod tests;

use steps::{FileWork, SmokeDialog, quit};

const SETTLE_FRAMES_AFTER_FILE_WORK: u32 = 10;

#[derive(Clone, Debug, PartialEq)]
pub struct SmokeConfig {
    exit_after_frames: u32,
    screenshot: Option<PathBuf>,
    dialog: Option<SmokeDialog>,
    tool: Option<UiTool>,
    stroke: bool,
    page: Option<String>,
    tray: bool,
    save: Option<PathBuf>,
    open: Option<PathBuf>,
    export: Option<PathBuf>,
    dmabuf_selftest: Option<ModifierPolicy>,
}

impl SmokeConfig {
    #[expect(clippy::excessive_nesting)]
    pub fn run(self, app: &adw::Application, window: &CadmiumWindow) {
        if let Some(mode) = self.dmabuf_selftest {
            run_dmabuf_selftest(app, window, mode);
            return;
        }
        let app = app.downgrade();
        let steps = self.steps();
        let drawn = Cell::new(0u32);
        let next_step = Cell::new(0usize);
        let earliest_end_frame = Cell::new(self.exit_after_frames);
        let file_work = FileWork::default();
        window.add_tick_callback(move |window, _| {
            drawn.set(drawn.get() + 1);
            while let Some(step) = steps.get(next_step.get()) {
                if !step.is_ready(drawn.get(), earliest_end_frame.get(), window) {
                    return glib::ControlFlow::Continue;
                }
                let frames_late = drawn.get().saturating_sub(earliest_end_frame.get());
                if !step.run(&app, window, frames_late, &file_work) {
                    return glib::ControlFlow::Continue;
                }
                if FileWork::is_file_step(step) {
                    earliest_end_frame.set(
                        earliest_end_frame
                            .get()
                            .max(drawn.get() + SETTLE_FRAMES_AFTER_FILE_WORK),
                    );
                }
                next_step.set(next_step.get() + 1);
            }
            glib::ControlFlow::Break
        });
    }
}

fn run_dmabuf_selftest(app: &adw::Application, window: &CadmiumWindow, policy: ModifierPolicy) {
    let app = app.downgrade();
    let selftest: RefCell<Option<DmabufSelftest>> = RefCell::new(None);
    window.add_tick_callback(move |window, _| {
        let mut selftest = selftest.borrow_mut();
        let progress = match selftest.as_mut() {
            Some(running) => running.tick(window),
            None => match DmabufSelftest::start(window, policy) {
                Ok(started) => {
                    *selftest = Some(started);
                    SelftestProgress::Running
                }
                Err(error) => SelftestProgress::Failed(error),
            },
        };
        match progress {
            SelftestProgress::Running => glib::ControlFlow::Continue,
            SelftestProgress::Passed(summary) => {
                println!("dmabuf selftest: ok {summary}");
                *selftest = None;
                quit(&app);
                glib::ControlFlow::Break
            }
            SelftestProgress::Failed(reason) => {
                println!("dmabuf selftest: failed {reason}");
                std::process::exit(1);
            }
        }
    });
}
