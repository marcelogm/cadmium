use std::cell::{Cell, RefCell};
use std::ffi::OsString;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use cadmium_engine::stroke::StrokeSample;
use gtk::{glib, graphene, gsk};

use crate::LOG_DOMAIN;
use crate::dmabuf::ModifierPolicy;
use crate::dmabuf_selftest::{DmabufSelftest, SelftestProgress};
use crate::presenter::PresentMode;
use crate::tools::UiTool;
use crate::window::CadmiumWindow;

const EXIT_AFTER_FRAMES: &str = "CADMIUM_SMOKE_EXIT_AFTER_FRAMES";
const SCREENSHOT: &str = "CADMIUM_SMOKE_SCREENSHOT";
const DIALOG: &str = "CADMIUM_SMOKE_DIALOG";
const STROKE: &str = "CADMIUM_SMOKE_STROKE";
const PAGE: &str = "CADMIUM_SMOKE_PAGE";
const TOOL: &str = "CADMIUM_SMOKE_TOOL";
const TOOL_ACTION: &str = "win.tool";
const SAVE: &str = "CADMIUM_SMOKE_SAVE";
const OPEN: &str = "CADMIUM_SMOKE_OPEN";
const EXPORT: &str = "CADMIUM_SMOKE_EXPORT";
const DMABUF_SELFTEST: &str = "CADMIUM_SMOKE_DMABUF_SELFTEST";
const PRESENT_MODE: &str = "CADMIUM_PRESENT";
const DEFAULT_EXIT_AFTER_FRAMES: u32 = 30;
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
    pub exit_after_frames: u32,
    pub screenshot: Option<PathBuf>,
    pub dialog: Option<SmokeDialog>,
    pub tool: Option<UiTool>,
    pub stroke: bool,
    pub page: Option<String>,
    pub save: Option<PathBuf>,
    pub open: Option<PathBuf>,
    pub export: Option<PathBuf>,
    pub dmabuf_selftest: Option<ModifierPolicy>,
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
    Screenshot(PathBuf),
    Quit,
}

impl SmokeStep {
    fn is_ready(&self, frame: u32, earliest_end_frame: u32, window: &CadmiumWindow) -> bool {
        match self {
            SmokeStep::ChooseTool(_) | SmokeStep::OpenDialog(_) | SmokeStep::ShowPage(_) => true,
            SmokeStep::Save(_) | SmokeStep::Open(_) | SmokeStep::Export(_) => true,
            SmokeStep::PaintStroke => window.canvas().presented_frames() > 0,
            SmokeStep::Screenshot(_) | SmokeStep::Quit => frame >= earliest_end_frame,
        }
    }

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
    pub fn from_env() -> Option<SmokeConfig> {
        SmokeConfig::from_lookup(|name| std::env::var_os(name))
    }

    fn from_lookup(lookup: impl Fn(&str) -> Option<OsString>) -> Option<SmokeConfig> {
        let exit_after_frames = lookup(EXIT_AFTER_FRAMES);
        let screenshot = lookup(SCREENSHOT)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        let dialog = lookup(DIALOG);
        let tool = lookup(TOOL).filter(|value| !value.is_empty());
        let stroke = lookup(STROKE).is_some_and(|value| !value.is_empty() && value != "0");
        let page = lookup(PAGE)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string_lossy().into_owned());
        let save = lookup(SAVE)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        let open = lookup(OPEN)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        let export = lookup(EXPORT)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        let dmabuf_selftest = lookup(DMABUF_SELFTEST).map(|_| {
            match lookup(PRESENT_MODE)
                .and_then(|mode| mode.to_str().and_then(PresentMode::from_name))
            {
                Some(PresentMode::Dmabuf(policy)) => policy,
                _ => ModifierPolicy::Any,
            }
        });
        if exit_after_frames.is_none()
            && screenshot.is_none()
            && dialog.is_none()
            && tool.is_none()
            && !stroke
            && page.is_none()
            && save.is_none()
            && open.is_none()
            && export.is_none()
            && dmabuf_selftest.is_none()
        {
            return None;
        }
        let dialog = dialog.and_then(|value| parse_or_warn(DIALOG, &value));
        let tool = tool.and_then(|value| {
            let parsed = value.to_str().and_then(UiTool::from_name);
            if parsed.is_none() {
                glib::g_warning!(LOG_DOMAIN, "ignoring {TOOL}={value:?}: not a tool");
            }
            parsed
        });
        let exit_after_frames = exit_after_frames
            .and_then(|value| parse_or_warn(EXIT_AFTER_FRAMES, &value))
            .unwrap_or(DEFAULT_EXIT_AFTER_FRAMES);
        Some(SmokeConfig {
            exit_after_frames,
            screenshot,
            dialog,
            tool,
            stroke,
            page,
            save,
            open,
            export,
            dmabuf_selftest,
        })
    }

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
        steps.extend(self.screenshot.clone().map(SmokeStep::Screenshot));
        steps.push(SmokeStep::Quit);
        steps
    }

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

fn parse_or_warn<T: std::str::FromStr>(name: &str, value: &OsString) -> Option<T> {
    let parsed = value.to_str().and_then(|text| text.trim().parse().ok());
    if parsed.is_none() {
        glib::g_warning!(LOG_DOMAIN, "ignoring {name}={value:?}: not a valid value");
    }
    parsed
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

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        }
    }

    #[test]
    fn no_smoke_variables_means_no_smoke_run() {
        assert_eq!(SmokeConfig::from_lookup(lookup(&[])), None);
    }

    #[test]
    fn screenshot_alone_exits_after_default_frame_count() {
        let config = SmokeConfig::from_lookup(lookup(&[(SCREENSHOT, "/tmp/shot.png")])).unwrap();
        assert_eq!(config.exit_after_frames, DEFAULT_EXIT_AFTER_FRAMES);
        assert_eq!(config.screenshot, Some(PathBuf::from("/tmp/shot.png")));
    }

    #[test]
    fn explicit_frame_count_is_honoured() {
        let config = SmokeConfig::from_lookup(lookup(&[(EXIT_AFTER_FRAMES, "60")])).unwrap();
        assert_eq!(config.exit_after_frames, 60);
        assert_eq!(config.screenshot, None);
    }

    #[test]
    fn unparsable_frame_count_falls_back_to_default() {
        let config = SmokeConfig::from_lookup(lookup(&[(EXIT_AFTER_FRAMES, "soon")])).unwrap();
        assert_eq!(config.exit_after_frames, DEFAULT_EXIT_AFTER_FRAMES);
    }

    #[test]
    fn dialog_alone_starts_a_smoke_run_that_opens_it() {
        let config = SmokeConfig::from_lookup(lookup(&[(DIALOG, "new")])).unwrap();
        assert_eq!(config.dialog, Some(SmokeDialog::NewPainting));
        assert_eq!(config.exit_after_frames, DEFAULT_EXIT_AFTER_FRAMES);
    }

    #[test]
    fn about_and_shortcuts_dialogs_are_recognised() {
        let about = SmokeConfig::from_lookup(lookup(&[(DIALOG, "about")])).unwrap();
        let shortcuts = SmokeConfig::from_lookup(lookup(&[(DIALOG, "shortcuts")])).unwrap();
        assert_eq!(about.dialog, Some(SmokeDialog::About));
        assert_eq!(shortcuts.dialog, Some(SmokeDialog::Shortcuts));
    }

    #[test]
    fn empty_screenshot_path_counts_as_unset() {
        assert_eq!(SmokeConfig::from_lookup(lookup(&[(SCREENSHOT, "")])), None);
    }

    #[test]
    fn stroke_runs_after_the_dialog_and_before_the_screenshot_and_quit() {
        let config = SmokeConfig::from_lookup(lookup(&[
            (STROKE, "1"),
            (DIALOG, "about"),
            (SCREENSHOT, "/tmp/shot.png"),
        ]))
        .unwrap();
        assert_eq!(
            config.steps(),
            [
                SmokeStep::OpenDialog(SmokeDialog::About),
                SmokeStep::PaintStroke,
                SmokeStep::Screenshot(PathBuf::from("/tmp/shot.png")),
                SmokeStep::Quit,
            ]
        );
    }

    #[test]
    fn page_is_shown_after_the_stroke_and_before_the_screenshot() {
        let config = SmokeConfig::from_lookup(lookup(&[
            (PAGE, "lab"),
            (STROKE, "1"),
            (SCREENSHOT, "/tmp/shot.png"),
        ]))
        .unwrap();
        assert_eq!(
            config.steps(),
            [
                SmokeStep::PaintStroke,
                SmokeStep::ShowPage("lab".to_string()),
                SmokeStep::Screenshot(PathBuf::from("/tmp/shot.png")),
                SmokeStep::Quit,
            ]
        );
    }

    #[test]
    fn file_steps_run_after_the_stroke_and_before_the_page_and_screenshot() {
        let config = SmokeConfig::from_lookup(lookup(&[
            (OPEN, "/tmp/a.cadmium"),
            (EXPORT, "/tmp/a.png"),
            (PAGE, "lab"),
            (SAVE, "/tmp/a.cadmium"),
            (STROKE, "1"),
            (SCREENSHOT, "/tmp/shot.png"),
        ]))
        .unwrap();
        assert_eq!(
            config.steps(),
            [
                SmokeStep::PaintStroke,
                SmokeStep::Save(PathBuf::from("/tmp/a.cadmium")),
                SmokeStep::Open(PathBuf::from("/tmp/a.cadmium")),
                SmokeStep::Export(PathBuf::from("/tmp/a.png")),
                SmokeStep::ShowPage("lab".to_string()),
                SmokeStep::Screenshot(PathBuf::from("/tmp/shot.png")),
                SmokeStep::Quit,
            ]
        );
    }

    #[test]
    fn tool_is_chosen_before_anything_else() {
        let config =
            SmokeConfig::from_lookup(lookup(&[(STROKE, "1"), (TOOL, "knife"), (DIALOG, "about")]))
                .unwrap();
        assert_eq!(
            config.steps(),
            [
                SmokeStep::ChooseTool(UiTool::Paint(cadmium_engine::stroke::Tool::Knife)),
                SmokeStep::OpenDialog(SmokeDialog::About),
                SmokeStep::PaintStroke,
                SmokeStep::Quit,
            ]
        );
    }

    #[test]
    fn unknown_tool_is_ignored() {
        let config = SmokeConfig::from_lookup(lookup(&[(TOOL, "spray")])).unwrap();
        assert_eq!(config.tool, None);
    }

    #[test]
    fn open_alone_starts_a_smoke_run() {
        let config = SmokeConfig::from_lookup(lookup(&[(OPEN, "/tmp/a.cadmium")])).unwrap();
        assert_eq!(config.open, Some(PathBuf::from("/tmp/a.cadmium")));
    }

    #[test]
    fn page_alone_starts_a_smoke_run() {
        let config = SmokeConfig::from_lookup(lookup(&[(PAGE, "lab")])).unwrap();
        assert_eq!(config.page.as_deref(), Some("lab"));
    }

    #[test]
    fn stroke_zero_does_not_paint() {
        assert_eq!(SmokeConfig::from_lookup(lookup(&[(STROKE, "0")])), None);
    }

    #[test]
    fn smoke_stroke_crosses_the_canvas_middle_with_a_pressure_ramp() {
        let samples = smoke_stroke(2048, 1536);
        assert_eq!(samples.len(), STROKE_SAMPLES);
        assert!((samples[0].pressure - STROKE_PRESSURE_START).abs() < 1e-6);
        assert!((samples[STROKE_SAMPLES - 1].pressure - STROKE_PRESSURE_END).abs() < 1e-6);
        let peak = samples.iter().map(|s| s.pressure).fold(0.0, f32::max);
        assert!(peak > 0.95);
        assert!(samples.iter().all(|s| s.x > 0.0 && s.x < 2048.0));
        assert!(
            samples
                .iter()
                .all(|s| s.y > 1536.0 * 0.3 && s.y < 1536.0 * 0.7)
        );
        assert!(
            samples
                .windows(2)
                .all(|pair| pair[1].time_ms > pair[0].time_ms)
        );
    }

    #[test]
    fn dmabuf_selftest_alone_starts_a_smoke_run() {
        let config = SmokeConfig::from_lookup(lookup(&[(DMABUF_SELFTEST, "1")])).unwrap();
        assert_eq!(config.dmabuf_selftest, Some(ModifierPolicy::Any));
    }

    #[test]
    fn linear_present_mode_restricts_the_dmabuf_selftest_to_linear() {
        let config =
            SmokeConfig::from_lookup(lookup(&[(DMABUF_SELFTEST, "1"), (PRESENT_MODE, "linear")]))
                .unwrap();
        assert_eq!(config.dmabuf_selftest, Some(ModifierPolicy::LinearOnly));
    }

    #[test]
    fn nodcc_present_mode_keeps_the_dmabuf_selftest_off_compressed_modifiers() {
        let config = SmokeConfig::from_lookup(lookup(&[
            (DMABUF_SELFTEST, "1"),
            (PRESENT_MODE, "dmabuf-nodcc"),
        ]))
        .unwrap();
        assert_eq!(config.dmabuf_selftest, Some(ModifierPolicy::NoCompression));
    }

    #[test]
    fn unknown_dialog_is_ignored() {
        let config = SmokeConfig::from_lookup(lookup(&[(DIALOG, "preferences")])).unwrap();
        assert_eq!(config.dialog, None);
    }
}
