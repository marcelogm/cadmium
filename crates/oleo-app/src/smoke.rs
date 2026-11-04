use std::cell::Cell;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use adw::prelude::*;
use gtk::{glib, graphene, gsk};
use oleo_engine::stroke::StrokeSample;

use crate::LOG_DOMAIN;
use crate::window::OleoWindow;

const EXIT_AFTER_FRAMES: &str = "OLEO_SMOKE_EXIT_AFTER_FRAMES";
const SCREENSHOT: &str = "OLEO_SMOKE_SCREENSHOT";
const DIALOG: &str = "OLEO_SMOKE_DIALOG";
const STROKE: &str = "OLEO_SMOKE_STROKE";
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

    fn open(self, window: &OleoWindow) {
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
    pub stroke: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum SmokeStep {
    OpenDialog(SmokeDialog),
    PaintStroke,
    Screenshot(PathBuf),
    Quit,
}

impl SmokeStep {
    fn is_ready(&self, frame: u32, exit_after_frames: u32, window: &OleoWindow) -> bool {
        match self {
            SmokeStep::OpenDialog(_) => true,
            SmokeStep::PaintStroke => window.canvas().presented_frames() > 0,
            SmokeStep::Screenshot(_) | SmokeStep::Quit => frame >= exit_after_frames,
        }
    }

    fn run(
        &self,
        app: &glib::WeakRef<adw::Application>,
        window: &OleoWindow,
        frames_late: u32,
    ) -> bool {
        match self {
            SmokeStep::OpenDialog(dialog) => dialog.open(window),
            SmokeStep::PaintStroke => paint_stroke(window),
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
        let stroke = lookup(STROKE).is_some_and(|value| !value.is_empty() && value != "0");
        if exit_after_frames.is_none() && screenshot.is_none() && dialog.is_none() && !stroke {
            return None;
        }
        let dialog = dialog.and_then(|value| parse_or_warn(DIALOG, &value));
        let exit_after_frames = exit_after_frames
            .and_then(|value| parse_or_warn(EXIT_AFTER_FRAMES, &value))
            .unwrap_or(DEFAULT_EXIT_AFTER_FRAMES);
        Some(SmokeConfig {
            exit_after_frames,
            screenshot,
            dialog,
            stroke,
        })
    }

    fn steps(&self) -> Vec<SmokeStep> {
        let mut steps = Vec::new();
        steps.extend(self.dialog.map(SmokeStep::OpenDialog));
        if self.stroke {
            steps.push(SmokeStep::PaintStroke);
        }
        steps.extend(self.screenshot.clone().map(SmokeStep::Screenshot));
        steps.push(SmokeStep::Quit);
        steps
    }

    pub fn run(self, app: &adw::Application, window: &OleoWindow) {
        let app = app.downgrade();
        let steps = self.steps();
        let drawn = Cell::new(0u32);
        let next_step = Cell::new(0usize);
        window.add_tick_callback(move |window, _| {
            drawn.set(drawn.get() + 1);
            while let Some(step) = steps.get(next_step.get()) {
                if !step.is_ready(drawn.get(), self.exit_after_frames, window) {
                    return glib::ControlFlow::Continue;
                }
                let frames_late = drawn.get().saturating_sub(self.exit_after_frames);
                if !step.run(&app, window, frames_late) {
                    return glib::ControlFlow::Continue;
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

fn paint_stroke(window: &OleoWindow) {
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
    state.end_stroke();
    canvas.queue_present();
}

fn parse_or_warn<T: std::str::FromStr>(name: &str, value: &OsString) -> Option<T> {
    let parsed = value.to_str().and_then(|text| text.trim().parse().ok());
    if parsed.is_none() {
        glib::g_warning!(LOG_DOMAIN, "ignoring {name}={value:?}: not a valid value");
    }
    parsed
}

fn save_screenshot(window: &OleoWindow, path: &Path) -> Result<(), ScreenshotError> {
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
    fn unknown_dialog_is_ignored() {
        let config = SmokeConfig::from_lookup(lookup(&[(DIALOG, "preferences")])).unwrap();
        assert_eq!(config.dialog, None);
    }
}
