use std::cell::{Cell, RefCell};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use cadmium_engine::stroke::StrokeSample;
use gtk::{glib, graphene, gsk};

use crate::LOG_DOMAIN;
use crate::dmabuf::ModifierPolicy;
use crate::dmabuf_selftest::{DmabufSelftest, SelftestProgress};
use crate::tools::UiTool;
use crate::window::CadmiumWindow;

mod config;
#[cfg(test)]
mod tests;

const TOOL_ACTION: &str = "win.tool";
const STROKE_PIGMENT: &str = "ultramarine_blue";
const STROKE_SAMPLES: usize = 60;
const STROKE_START: f64 = 0.15;
const STROKE_END: f64 = 0.85;
const STROKE_AMPLITUDE: f64 = 0.12;
const STROKE_WAVES: f64 = 1.5;
const STROKE_PRESSURE_START: f32 = 0.2;
const STROKE_PRESSURE_PEAK: f32 = 1.0;
const STROKE_PRESSURE_END: f32 = 0.4;
const STROKE_TILT_DEG: f32 = 25.0;
const STROKE_SAMPLE_INTERVAL_MS: u32 = 8;
const SCREENSHOT_RETRY_FRAMES: u32 = 120;
const SETTLE_FRAMES_AFTER_FILE_WORK: u32 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmokeDialog {
    NewPainting,
    About,
    Shortcuts,
}

impl std::str::FromStr for SmokeDialog {
    type Err = ();

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "new" => Ok(SmokeDialog::NewPainting),
            "about" => Ok(SmokeDialog::About),
            "shortcuts" => Ok(SmokeDialog::Shortcuts),
            _ => Err(()),
        }
    }
}

impl SmokeDialog {
    fn action(self) -> &'static str {
        match self {
            SmokeDialog::NewPainting => "win.new",
            SmokeDialog::About => "app.about",
            SmokeDialog::Shortcuts => "app.shortcuts",
        }
    }

    fn open(self, window: &CadmiumWindow) {
        if let Err(error) = WidgetExt::activate_action(window, self.action(), None) {
            glib::g_warning!(LOG_DOMAIN, "smoke could not open {self:?}: {error}");
        }
    }
}

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

#[derive(Clone, Debug, PartialEq)]
enum SmokeStep {
    ChooseTool(UiTool),
    OpenDialog(SmokeDialog),
    PaintStroke,
    Save(PathBuf),
    Open(PathBuf),
    Export(PathBuf),
    ShowPage(String),
    ShowTray,
    Screenshot(PathBuf),
    Quit,
}

impl SmokeStep {
    fn is_ready(&self, frame: u32, earliest_end_frame: u32, window: &CadmiumWindow) -> bool {
        match self {
            SmokeStep::ChooseTool(_)
            | SmokeStep::OpenDialog(_)
            | SmokeStep::ShowPage(_)
            | SmokeStep::ShowTray => true,
            SmokeStep::Save(_) | SmokeStep::Open(_) | SmokeStep::Export(_) => true,
            SmokeStep::PaintStroke => window.canvas().presented_frames() > 0,
            SmokeStep::Screenshot(_) | SmokeStep::Quit => frame >= earliest_end_frame,
        }
    }

    #[expect(clippy::excessive_nesting)]
    fn run(
        &self,
        app: &glib::WeakRef<adw::Application>,
        window: &CadmiumWindow,
        frames_late: u32,
        file_work: &FileWork,
    ) -> bool {
        match self {
            SmokeStep::ChooseTool(tool) => choose_tool(window, *tool),
            SmokeStep::OpenDialog(dialog) => dialog.open(window),
            SmokeStep::PaintStroke => paint_stroke(window),
            SmokeStep::Save(path) => {
                return file_work.advance(|| window.save_to(path.clone()), path);
            }
            SmokeStep::Open(path) => {
                return file_work.advance(|| window.open_path(path.clone()), path);
            }
            SmokeStep::Export(path) => {
                return file_work.advance(|| window.export_to(path.clone()), path);
            }
            SmokeStep::ShowPage(name) => window.show_sidebar_page(name),
            SmokeStep::ShowTray => window.show_tray(),
            SmokeStep::Screenshot(path) => match save_screenshot(window, path) {
                Err(ScreenshotError::NotRenderedYet) if frames_late < SCREENSHOT_RETRY_FRAMES => {
                    return false;
                }
                Err(error) => glib::g_warning!(
                    LOG_DOMAIN,
                    "smoke screenshot to {} failed: {error}",
                    path.display()
                ),
                Ok(()) => {}
            },
            SmokeStep::Quit => {
                if let Some(app) = app.upgrade() {
                    app.quit();
                }
            }
        }
        true
    }
}

#[derive(Default)]
struct FileWork {
    started: Cell<bool>,
    finished: Rc<Cell<bool>>,
}

impl FileWork {
    #[expect(clippy::excessive_nesting)]
    fn advance<F: Future<Output = bool> + 'static>(
        &self,
        start: impl FnOnce() -> F,
        path: &Path,
    ) -> bool {
        if !self.started.replace(true) {
            let finished = self.finished.clone();
            let work = start();
            let path = path.to_path_buf();
            glib::spawn_future_local(async move {
                if !work.await {
                    glib::g_warning!(LOG_DOMAIN, "smoke file step on {} failed", path.display());
                }
                finished.set(true);
            });
        }
        if !self.finished.replace(false) {
            return false;
        }
        self.started.set(false);
        true
    }

    fn is_file_step(step: &SmokeStep) -> bool {
        matches!(
            step,
            SmokeStep::Save(_) | SmokeStep::Open(_) | SmokeStep::Export(_)
        )
    }
}

#[derive(Debug)]
enum ScreenshotError {
    NotRenderedYet,
    NoRenderer,
    Save(glib::BoolError),
}

impl std::fmt::Display for ScreenshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScreenshotError::NotRenderedYet => write!(f, "window produced no render node"),
            ScreenshotError::NoRenderer => write!(f, "window has no renderer"),
            ScreenshotError::Save(error) => write!(f, "{error}"),
        }
    }
}

impl SmokeConfig {
    fn steps(&self) -> Vec<SmokeStep> {
        let mut steps = Vec::new();
        steps.extend(self.tool.map(SmokeStep::ChooseTool));
        steps.extend(self.dialog.map(SmokeStep::OpenDialog));
        if self.stroke {
            steps.push(SmokeStep::PaintStroke);
        }
        steps.extend(self.save.clone().map(SmokeStep::Save));
        steps.extend(self.open.clone().map(SmokeStep::Open));
        steps.extend(self.export.clone().map(SmokeStep::Export));
        steps.extend(self.page.clone().map(SmokeStep::ShowPage));
        if self.tray {
            steps.push(SmokeStep::ShowTray);
        }
        steps.extend(self.screenshot.clone().map(SmokeStep::Screenshot));
        steps.push(SmokeStep::Quit);
        steps
    }

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

fn smoke_stroke(width: u32, height: u32) -> Vec<StrokeSample> {
    let (width, height) = (f64::from(width), f64::from(height));
    let last = (STROKE_SAMPLES - 1) as f64;
    (0..STROKE_SAMPLES)
        .map(|index| {
            let t = index as f64 / last;
            let along = STROKE_START + (STROKE_END - STROKE_START) * t;
            let wave = (t * STROKE_WAVES * std::f64::consts::TAU).sin();
            let t = t as f32;
            let pressure = if t <= 0.5 {
                STROKE_PRESSURE_START + (STROKE_PRESSURE_PEAK - STROKE_PRESSURE_START) * t * 2.0
            } else {
                STROKE_PRESSURE_PEAK
                    + (STROKE_PRESSURE_END - STROKE_PRESSURE_PEAK) * (t - 0.5) * 2.0
            };
            StrokeSample {
                x: width * along,
                y: height * (0.5 + STROKE_AMPLITUDE * wave),
                pressure,
                tilt_x_deg: STROKE_TILT_DEG,
                tilt_y_deg: STROKE_TILT_DEG,
                time_ms: index as u32 * STROKE_SAMPLE_INTERVAL_MS,
            }
        })
        .collect()
}

fn paint_stroke(window: &CadmiumWindow) {
    let canvas = window.canvas();
    let state = canvas.state();
    let mut state = state.borrow_mut();
    let spec = &state.session.canvas.spec;
    let Some(slot) = spec
        .pigment_slots
        .iter()
        .position(|id| id == STROKE_PIGMENT)
    else {
        glib::g_warning!(
            LOG_DOMAIN,
            "smoke stroke needs {STROKE_PIGMENT} on the palette"
        );
        return;
    };
    let samples = smoke_stroke(spec.width, spec.height);
    let gpu = state.gpu.clone();
    state.session.brush.recipe = vec![(slot, 1.0)];
    state.session.reload_brush(&gpu);
    let tool = state.session.brush.tool;
    state.begin_stroke(tool, samples[0]);
    state.add_samples(&samples[1..]);
    state.release();
    drop(state);
    canvas.queue_present();
    window.refresh_panels();
}

fn choose_tool(window: &CadmiumWindow, tool: UiTool) {
    if let Err(error) =
        WidgetExt::activate_action(window, TOOL_ACTION, Some(&tool.name().to_variant()))
    {
        glib::g_warning!(LOG_DOMAIN, "smoke could not choose {tool:?}: {error}");
    }
}

#[expect(clippy::excessive_nesting)]
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
                if let Some(app) = app.upgrade() {
                    app.quit();
                }
                glib::ControlFlow::Break
            }
            SelftestProgress::Failed(reason) => {
                println!("dmabuf selftest: failed {reason}");
                std::process::exit(1);
            }
        }
    });
}

fn save_screenshot(window: &CadmiumWindow, path: &Path) -> Result<(), ScreenshotError> {
    let width = window.width() as f32;
    let height = window.height() as f32;
    let paintable = gtk::WidgetPaintable::new(Some(window));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, width as f64, height as f64);
    let node = snapshot.to_node().ok_or(ScreenshotError::NotRenderedYet)?;
    let renderer = window.renderer().ok_or(ScreenshotError::NoRenderer)?;
    let viewport = graphene::Rect::new(0.0, 0.0, width, height);
    let texture = gsk::prelude::GskRendererExt::render_texture(&renderer, &node, Some(&viewport));
    texture.save_to_png(path).map_err(ScreenshotError::Save)
}
