//! Smoke steps on the tray: picking its tool.

use std::ffi::OsString;

use gtk::glib;

use crate::LOG_DOMAIN;
use crate::tools::TrayTool;
use crate::window::CadmiumWindow;

pub(super) const TRAY_TOOL: &str = "CADMIUM_SMOKE_TRAY_TOOL";

pub(super) fn parse_tray_tool(value: OsString) -> Option<TrayTool> {
    let parsed = value.to_str().and_then(TrayTool::from_name);
    if parsed.is_none() {
        glib::g_warning!(
            LOG_DOMAIN,
            "ignoring {TRAY_TOOL}={value:?}: not a tray tool"
        );
    }
    parsed
}

pub(super) fn choose_tray_tool(window: &CadmiumWindow, tool: TrayTool) {
    window.select_tray_tool(tool.name());
}

#[cfg(test)]
mod tests;
