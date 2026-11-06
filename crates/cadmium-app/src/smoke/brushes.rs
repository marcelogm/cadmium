//! Brush library smoke steps: import, pick, export and delete without dialogs.

use std::ffi::OsString;
use std::path::PathBuf;

use adw::prelude::*;
use gtk::glib;

use super::SmokeConfig;
use super::config::{lookup_flag, lookup_path, parse_or_warn};
use super::editor;
use super::steps::SmokeStep;
use crate::LOG_DOMAIN;
use crate::window::CadmiumWindow;

pub(super) const BRUSH_DIR: &str = "CADMIUM_SMOKE_BRUSH_DIR";
pub(super) const BRUSH_IMPORT: &str = "CADMIUM_SMOKE_BRUSH_IMPORT";
pub(super) const BRUSH: &str = "CADMIUM_SMOKE_BRUSH";
pub(super) const BRUSH_EXPORT: &str = "CADMIUM_SMOKE_BRUSH_EXPORT";
pub(super) const BRUSH_DELETE: &str = "CADMIUM_SMOKE_BRUSH_DELETE";
pub(super) const BRUSH_EDITOR: &str = "CADMIUM_SMOKE_BRUSH_EDITOR";

/// What a smoke run does with the brush library.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct SmokeBrushes {
    pub(super) dir: Option<PathBuf>,
    pub(super) import: Option<PathBuf>,
    pub(super) pick: Option<String>,
    pub(super) export: Option<PathBuf>,
    pub(super) delete: bool,
    /// How many frames the brush editor stays on show.
    pub(super) editor: Option<u32>,
}

impl SmokeBrushes {
    pub(super) fn from_lookup(lookup: &impl Fn(&str) -> Option<OsString>) -> SmokeBrushes {
        SmokeBrushes {
            dir: lookup_path(lookup, BRUSH_DIR),
            import: lookup_path(lookup, BRUSH_IMPORT),
            pick: lookup(BRUSH)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string_lossy().into_owned()),
            export: lookup_path(lookup, BRUSH_EXPORT),
            delete: lookup_flag(lookup, BRUSH_DELETE),
            editor: lookup(BRUSH_EDITOR).and_then(|value| parse_or_warn(BRUSH_EDITOR, &value)),
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        *self == SmokeBrushes::default()
    }

    pub(super) fn before_stroke(&self) -> impl Iterator<Item = SmokeStep> {
        let import = self.import.clone().map(BrushStep::Import);
        let pick = self.pick.clone().map(BrushStep::Pick);
        import.into_iter().chain(pick).map(SmokeStep::Brush)
    }

    pub(super) fn after_files(&self) -> impl Iterator<Item = SmokeStep> {
        let export = self.export.clone().map(BrushStep::Export);
        let delete = self.delete.then_some(BrushStep::Delete);
        export.into_iter().chain(delete).map(SmokeStep::Brush)
    }

    pub(super) fn editor_step(&self) -> Option<SmokeStep> {
        self.editor
            .map(|frames| SmokeStep::Brush(BrushStep::Editor(frames)))
    }
}

impl SmokeConfig {
    /// The brush folder a smoke run loads its library from; none keeps it factory-only.
    pub fn brush_dir(&self) -> Option<PathBuf> {
        self.brushes.dir.clone()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum BrushStep {
    Import(PathBuf),
    Pick(String),
    Export(PathBuf),
    Delete,
    /// Opens the brush editor paused mid-stroke, on show for this many frames.
    Editor(u32),
}

impl BrushStep {
    /// The editor waits for the rack's thumbnails: opening the studio pauses them.
    pub(super) fn is_ready(&self, window: &CadmiumWindow) -> bool {
        match self {
            BrushStep::Editor(_) => !window.thumbnails_pending(),
            _ => true,
        }
    }

    pub(super) fn run(&self, window: &CadmiumWindow) {
        match self {
            BrushStep::Import(path) => window.import_brush_file(path),
            BrushStep::Pick(name) => pick_from_rack(window, name),
            BrushStep::Export(path) => window.export_brush_to(path),
            BrushStep::Delete => window.delete_worn_brush_now(),
            BrushStep::Editor(_) => editor::open_mid_stroke(window),
        }
    }
}

/// Selects the rack entry named `name`, so the rack's own selection handler wears it.
fn pick_from_rack(window: &CadmiumWindow, name: &str) {
    match rack_entry(window.upcast_ref(), name) {
        Some((flow, child)) => flow.select_child(&child),
        None => glib::g_warning!(LOG_DOMAIN, "smoke found no brush named {name:?}"),
    }
}

/// The name on a rack entry: the label below its thumbnail.
fn entry_name(child: &gtk::FlowBoxChild) -> Option<glib::GString> {
    let label = child.child()?.last_child().and_downcast::<gtk::Label>()?;
    Some(label.label())
}

fn rack_entry(widget: &gtk::Widget, name: &str) -> Option<(gtk::FlowBox, gtk::FlowBoxChild)> {
    if let Some(child) = widget.downcast_ref::<gtk::FlowBoxChild>()
        && entry_name(child).is_some_and(|shown| shown == name)
    {
        let flow = child.parent().and_downcast::<gtk::FlowBox>()?;
        return Some((flow, child.clone()));
    }
    let mut next = widget.first_child();
    while let Some(child) = next {
        if let Some(found) = rack_entry(&child, name) {
            return Some(found);
        }
        next = child.next_sibling();
    }
    None
}
