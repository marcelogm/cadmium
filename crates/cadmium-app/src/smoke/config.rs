use std::ffi::OsString;
use std::path::PathBuf;

use gtk::glib;

use crate::LOG_DOMAIN;
use crate::dmabuf::ModifierPolicy;
use crate::presenter::mode::PresentMode;
use crate::tools::UiTool;

use super::SmokeConfig;

pub(super) const EXIT_AFTER_FRAMES: &str = "CADMIUM_SMOKE_EXIT_AFTER_FRAMES";
pub(super) const SCREENSHOT: &str = "CADMIUM_SMOKE_SCREENSHOT";
pub(super) const DIALOG: &str = "CADMIUM_SMOKE_DIALOG";
pub(super) const STROKE: &str = "CADMIUM_SMOKE_STROKE";
pub(super) const PAGE: &str = "CADMIUM_SMOKE_PAGE";
pub(super) const TRAY: &str = "CADMIUM_SMOKE_TRAY";
pub(super) const TOOL: &str = "CADMIUM_SMOKE_TOOL";
pub(super) const SAVE: &str = "CADMIUM_SMOKE_SAVE";
pub(super) const OPEN: &str = "CADMIUM_SMOKE_OPEN";
pub(super) const EXPORT: &str = "CADMIUM_SMOKE_EXPORT";
pub(super) const DMABUF_SELFTEST: &str = "CADMIUM_SMOKE_DMABUF_SELFTEST";
pub(super) const PRESENT_MODE: &str = "CADMIUM_PRESENT";
pub(super) const DEFAULT_EXIT_AFTER_FRAMES: u32 = 30;

impl SmokeConfig {
    pub fn from_env() -> Option<SmokeConfig> {
        SmokeConfig::from_lookup(|name| std::env::var_os(name))
    }

    pub(super) fn from_lookup(lookup: impl Fn(&str) -> Option<OsString>) -> Option<SmokeConfig> {
        let exit_after_frames = lookup(EXIT_AFTER_FRAMES);
        let screenshot = lookup_path(&lookup, SCREENSHOT);
        let dialog = lookup(DIALOG);
        let tool = lookup(TOOL).filter(|value| !value.is_empty());
        let config = SmokeConfig {
            exit_after_frames: DEFAULT_EXIT_AFTER_FRAMES,
            screenshot,
            dialog: None,
            tool: None,
            stroke: lookup_flag(&lookup, STROKE),
            page: lookup(PAGE)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string_lossy().into_owned()),
            tray: lookup_flag(&lookup, TRAY),
            save: lookup_path(&lookup, SAVE),
            open: lookup_path(&lookup, OPEN),
            export: lookup_path(&lookup, EXPORT),
            dmabuf_selftest: lookup(DMABUF_SELFTEST).map(|_| lookup_policy(&lookup)),
        };
        if exit_after_frames.is_none()
            && dialog.is_none()
            && tool.is_none()
            && config.requests_nothing_else()
        {
            return None;
        }
        Some(SmokeConfig {
            dialog: dialog.and_then(|value| parse_or_warn(DIALOG, &value)),
            tool: tool.and_then(parse_tool),
            exit_after_frames: exit_after_frames
                .and_then(|value| parse_or_warn(EXIT_AFTER_FRAMES, &value))
                .unwrap_or(DEFAULT_EXIT_AFTER_FRAMES),
            ..config
        })
    }

    fn requests_nothing_else(&self) -> bool {
        self.screenshot.is_none()
            && !self.stroke
            && self.page.is_none()
            && !self.tray
            && self.save.is_none()
            && self.open.is_none()
            && self.export.is_none()
            && self.dmabuf_selftest.is_none()
    }
}

fn lookup_path(lookup: &impl Fn(&str) -> Option<OsString>, name: &str) -> Option<PathBuf> {
    lookup(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn lookup_flag(lookup: &impl Fn(&str) -> Option<OsString>, name: &str) -> bool {
    lookup(name).is_some_and(|value| !value.is_empty() && value != "0")
}

fn lookup_policy(lookup: &impl Fn(&str) -> Option<OsString>) -> ModifierPolicy {
    match lookup(PRESENT_MODE).and_then(|mode| mode.to_str().and_then(PresentMode::from_name)) {
        Some(PresentMode::Dmabuf(policy)) => policy,
        _ => ModifierPolicy::Any,
    }
}

fn parse_tool(value: OsString) -> Option<UiTool> {
    let parsed = value.to_str().and_then(UiTool::from_name);
    if parsed.is_none() {
        glib::g_warning!(LOG_DOMAIN, "ignoring {TOOL}={value:?}: not a tool");
    }
    parsed
}

fn parse_or_warn<T: std::str::FromStr>(name: &str, value: &OsString) -> Option<T> {
    let parsed = value.to_str().and_then(|text| text.trim().parse().ok());
    if parsed.is_none() {
        glib::g_warning!(LOG_DOMAIN, "ignoring {name}={value:?}: not a valid value");
    }
    parsed
}
