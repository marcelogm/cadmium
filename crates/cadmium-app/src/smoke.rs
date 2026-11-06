use std::cell::{Cell, RefCell};
use std::path::PathBuf;

use adw::prelude::*;
use gtk::glib;

use crate::dmabuf::ModifierPolicy;
use crate::dmabuf_selftest::{DmabufSelftest, SelftestProgress};
use crate::tools::UiTool;
use crate::window::CadmiumWindow;

mod brushes;
mod config;
mod editor;
mod steps;
#[cfg(test)]
mod tests;

use brushes::SmokeBrushes;
use steps::{FileWork, SmokeDialog, SmokeStep, quit};

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
    brushes: SmokeBrushes,
    dmabuf_selftest: Option<ModifierPolicy>,
}

impl SmokeConfig {
    pub fn run(self, app: &adw::Application, window: &CadmiumWindow) {
        if let Some(policy) = self.dmabuf_selftest {
            run_dmabuf_selftest(app, window, policy);
            return;
        }
        let steps = self.steps();
        let run = SmokeRun {
            app: app.downgrade(),
            expects_editor: steps.iter().any(SmokeStep::opens_editor),
            steps,
            drawn: Cell::new(0),
            next_step: Cell::new(0),
            earliest_end_frame: Cell::new(self.exit_after_frames),
            file_work: FileWork::default(),
        };
        window.add_tick_callback(move |window, _| run.tick(window));
    }
}

struct SmokeRun {
    app: glib::WeakRef<adw::Application>,
    steps: Vec<SmokeStep>,
    /// Whether a step opens the brush editor, which the screenshot must then show.
    expects_editor: bool,
    drawn: Cell<u32>,
    next_step: Cell<usize>,
    earliest_end_frame: Cell<u32>,
    file_work: FileWork,
}

impl SmokeRun {
    fn tick(&self, window: &CadmiumWindow) -> glib::ControlFlow {
        self.drawn.set(self.drawn.get() + 1);
        while let Some(step) = self.steps.get(self.next_step.get())
            && self.run_step(step, window)
        {
            self.next_step.set(self.next_step.get() + 1);
        }
        if self.next_step.get() < self.steps.len() {
            glib::ControlFlow::Continue
        } else {
            glib::ControlFlow::Break
        }
    }

    fn run_step(&self, step: &SmokeStep, window: &CadmiumWindow) -> bool {
        let (frame, end) = (self.drawn.get(), self.earliest_end_frame.get());
        if !step.is_ready(frame, end, self.expects_editor, window) {
            return false;
        }
        let frames_late = self
            .drawn
            .get()
            .saturating_sub(self.earliest_end_frame.get());
        if !step.run(&self.app, window, frames_late, &self.file_work) {
            return false;
        }
        if let Some(frames) = step.settle_frames() {
            self.earliest_end_frame
                .set(self.earliest_end_frame.get().max(self.drawn.get() + frames));
        }
        true
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
