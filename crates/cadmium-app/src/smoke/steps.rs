use std::cell::Cell;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use cadmium_engine::stroke::StrokeSample;
use gtk::{glib, graphene, gsk};

use crate::LOG_DOMAIN;
use crate::tools::UiTool;
use crate::window::CadmiumWindow;

use super::brushes::BrushStep;
use super::{SETTLE_FRAMES_AFTER_FILE_WORK, SmokeConfig, editor};

const TOOL_ACTION: &str = "win.tool";
const STROKE_PIGMENT: &str = "ultramarine_blue";
pub(super) const STROKE_SAMPLES: usize = 60;
const STROKE_START: f64 = 0.15;
const STROKE_END: f64 = 0.85;
const STROKE_AMPLITUDE: f64 = 0.12;
const STROKE_WAVES: f64 = 1.5;
pub(super) const STROKE_PRESSURE_START: f32 = 0.2;
const STROKE_PRESSURE_PEAK: f32 = 1.0;
pub(super) const STROKE_PRESSURE_END: f32 = 0.4;
const STROKE_TILT_DEG: f32 = 25.0;
const STROKE_SAMPLE_INTERVAL_MS: u32 = 8;
const SCREENSHOT_RETRY_FRAMES: u32 = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SmokeDialog {
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
pub(super) enum SmokeStep {
    ChooseTool(UiTool),
    OpenDialog(SmokeDialog),
    PaintStroke,
    Save(PathBuf),
    Open(PathBuf),
    Export(PathBuf),
    Brush(BrushStep),
    ShowPage(String),
    ShowTray,
    Screenshot(PathBuf),
    Quit,
}

impl SmokeStep {
    pub(super) fn is_ready(
        &self,
        frame: u32,
        earliest_end_frame: u32,
        expects_editor: bool,
        window: &CadmiumWindow,
    ) -> bool {
        match self {
            SmokeStep::ChooseTool(_)
            | SmokeStep::OpenDialog(_)
            | SmokeStep::ShowPage(_)
            | SmokeStep::ShowTray => true,
            SmokeStep::Brush(step) => step.is_ready(window),
            SmokeStep::Save(_) | SmokeStep::Open(_) | SmokeStep::Export(_) => true,
            SmokeStep::PaintStroke => window.canvas().presented_frames() > 0,
            SmokeStep::Screenshot(_) => {
                frame >= earliest_end_frame
                    && !window.thumbnails_pending()
                    && editor::ready_for_screenshot(window, expects_editor)
            }
            SmokeStep::Quit => frame >= earliest_end_frame,
        }
    }

    pub(super) fn run(
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
            SmokeStep::Brush(step) => step.run(window),
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
            SmokeStep::Quit => quit(app),
        }
        true
    }
}

impl SmokeConfig {
    /// The steps in the order they run: tool, dialog, brush import, brush pick, stroke, save,
    /// open, export, brush export, brush delete, page, tray, brush editor, screenshot, quit.
    /// The brush folder (`CADMIUM_SMOKE_BRUSH_DIR`) is not a step: the library is loaded from
    /// it at start-up.
    pub(super) fn steps(&self) -> Vec<SmokeStep> {
        let mut steps = Vec::new();
        steps.extend(self.tool.map(SmokeStep::ChooseTool));
        steps.extend(self.dialog.map(SmokeStep::OpenDialog));
        steps.extend(self.brushes.before_stroke());
        if self.stroke {
            steps.push(SmokeStep::PaintStroke);
        }
        steps.extend(self.save.clone().map(SmokeStep::Save));
        steps.extend(self.open.clone().map(SmokeStep::Open));
        steps.extend(self.export.clone().map(SmokeStep::Export));
        steps.extend(self.brushes.after_files());
        steps.extend(self.page.clone().map(SmokeStep::ShowPage));
        if self.tray {
            steps.push(SmokeStep::ShowTray);
        }
        steps.extend(self.brushes.editor_step());
        steps.extend(self.screenshot.clone().map(SmokeStep::Screenshot));
        steps.push(SmokeStep::Quit);
        steps
    }
}

impl SmokeStep {
    /// Whether this step opens the brush editor, which the screenshot must then show.
    pub(super) fn opens_editor(&self) -> bool {
        matches!(self, SmokeStep::Brush(BrushStep::Editor(_)))
    }

    /// How many frames the run's end waits after this step: file work settles,
    /// and the brush editor stays on show.
    pub(super) fn settle_frames(&self) -> Option<u32> {
        match self {
            SmokeStep::Brush(BrushStep::Editor(frames)) => Some(*frames),
            step if FileWork::is_file_step(step) => Some(SETTLE_FRAMES_AFTER_FILE_WORK),
            _ => None,
        }
    }
}

#[derive(Default)]
pub(super) struct FileWork {
    started: Cell<bool>,
    finished: Rc<Cell<bool>>,
}

impl FileWork {
    fn advance<F: Future<Output = bool> + 'static>(
        &self,
        start: impl FnOnce() -> F,
        path: &Path,
    ) -> bool {
        if !self.started.replace(true) {
            let finished = self.finished.clone();
            let work = start();
            let path = path.to_path_buf();
            glib::spawn_future_local(finish_file_work(work, path, finished));
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
            SmokeStep::Save(_) | SmokeStep::Open(_) | SmokeStep::Export(_) | SmokeStep::Brush(_)
        )
    }
}

async fn finish_file_work(
    work: impl Future<Output = bool>,
    path: PathBuf,
    finished: Rc<Cell<bool>>,
) {
    if !work.await {
        glib::g_warning!(LOG_DOMAIN, "smoke file step on {} failed", path.display());
    }
    finished.set(true);
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

pub(super) fn smoke_stroke(width: u32, height: u32) -> Vec<StrokeSample> {
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

pub(super) fn quit(app: &glib::WeakRef<adw::Application>) {
    if let Some(app) = app.upgrade() {
        app.quit();
    }
}
