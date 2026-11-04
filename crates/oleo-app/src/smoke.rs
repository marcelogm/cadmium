use std::cell::Cell;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use adw::prelude::*;
use gtk::{glib, graphene, gsk};

use crate::window::OleoWindow;

const EXIT_AFTER_FRAMES: &str = "OLEO_SMOKE_EXIT_AFTER_FRAMES";
const SCREENSHOT: &str = "OLEO_SMOKE_SCREENSHOT";
const DEFAULT_EXIT_AFTER_FRAMES: u32 = 30;
const LOG_DOMAIN: &str = "oleo";

#[derive(Clone, Debug, PartialEq)]
pub struct SmokeConfig {
    pub exit_after_frames: u32,
    pub screenshot: Option<PathBuf>,
}

impl SmokeConfig {
    pub fn from_env() -> Option<SmokeConfig> {
        SmokeConfig::from_lookup(|name| std::env::var_os(name))
    }

    fn from_lookup(lookup: impl Fn(&str) -> Option<OsString>) -> Option<SmokeConfig> {
        let exit_after_frames = lookup(EXIT_AFTER_FRAMES);
        let screenshot = lookup(SCREENSHOT).map(PathBuf::from);
        if exit_after_frames.is_none() && screenshot.is_none() {
            return None;
        }
        let exit_after_frames = exit_after_frames
            .and_then(|value| parse_or_warn(EXIT_AFTER_FRAMES, &value))
            .unwrap_or(DEFAULT_EXIT_AFTER_FRAMES);
        Some(SmokeConfig {
            exit_after_frames,
            screenshot,
        })
    }

    pub fn run(self, app: &adw::Application, window: &OleoWindow) {
        let app = app.downgrade();
        let drawn = Cell::new(0u32);
        window.add_tick_callback(move |window, _| {
            drawn.set(drawn.get() + 1);
            if drawn.get() < self.exit_after_frames {
                return glib::ControlFlow::Continue;
            }
            if let Some(path) = &self.screenshot
                && let Err(error) = save_screenshot(window, path)
            {
                glib::g_warning!(
                    LOG_DOMAIN,
                    "smoke screenshot to {} failed: {error}",
                    path.display()
                );
            }
            if let Some(app) = app.upgrade() {
                app.quit();
            }
            glib::ControlFlow::Break
        });
    }
}

fn parse_or_warn<T: std::str::FromStr>(name: &str, value: &OsString) -> Option<T> {
    let parsed = value.to_str().and_then(|text| text.trim().parse().ok());
    if parsed.is_none() {
        glib::g_warning!(LOG_DOMAIN, "ignoring {name}={value:?}: not a valid value");
    }
    parsed
}

fn save_screenshot(window: &OleoWindow, path: &Path) -> Result<(), String> {
    let width = window.width() as f32;
    let height = window.height() as f32;
    let paintable = gtk::WidgetPaintable::new(Some(window));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, width as f64, height as f64);
    let node = snapshot
        .to_node()
        .ok_or_else(|| "window produced no render node".to_string())?;
    let renderer = window
        .renderer()
        .ok_or_else(|| "window has no renderer".to_string())?;
    let viewport = graphene::Rect::new(0.0, 0.0, width, height);
    let texture = gsk::prelude::GskRendererExt::render_texture(&renderer, &node, Some(&viewport));
    texture.save_to_png(path).map_err(|error| error.to_string())
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
}
