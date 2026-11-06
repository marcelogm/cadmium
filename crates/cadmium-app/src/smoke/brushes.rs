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
pub(super) const SHEET_STROKE: &str = "CADMIUM_SMOKE_SHEET_STROKE";
pub(super) const NEW_BRUSH: &str = "CADMIUM_SMOKE_NEW_BRUSH";

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
    /// Whether a stroke is painted on the editor's test sheet once it shows.
    pub(super) sheet_stroke: bool,
    /// Whether the editor opens on a brush made from scratch, through "New Brush…".
    pub(super) new_brush: bool,
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
            sheet_stroke: lookup_flag(lookup, SHEET_STROKE),
            new_brush: lookup_flag(lookup, NEW_BRUSH),
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

    /// The editor (on the worn brush, or on a new one), then a stroke on its test sheet when
    /// asked for.
    pub(super) fn editor_steps(&self) -> impl Iterator<Item = SmokeStep> {
        let open = if self.new_brush {
            BrushStep::NewBrushEditor
        } else {
            BrushStep::Editor
        };
        let editor = self.editor.map(open);
        let stroke = self
            .editor
            .and(self.sheet_stroke.then_some(BrushStep::SheetStroke));
        editor.into_iter().chain(stroke).map(SmokeStep::Brush)
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
    /// Opens the brush editor, on show for this many frames.
    Editor(u32),
    /// Opens the brush editor on a new brush, as "New Brush…" does, on show for this many
    /// frames.
    NewBrushEditor(u32),
    /// Paints a stroke on the open editor's test sheet.
    SheetStroke,
}

impl BrushStep {
    /// The editor waits for the rack's thumbnails (opening it pauses them); the sheet stroke
    /// waits for the sheet's first frame.
    pub(super) fn is_ready(&self, window: &CadmiumWindow) -> bool {
        match self {
            BrushStep::Editor(_) | BrushStep::NewBrushEditor(_) => !window.thumbnails_pending(),
            BrushStep::SheetStroke => editor::sheet_shown(window),
            _ => true,
        }
    }

    pub(super) fn run(&self, window: &CadmiumWindow) {
        match self {
            BrushStep::Import(path) => window.import_brush_file(path),
            BrushStep::Pick(name) => pick_from_rack(window, name),
            BrushStep::Export(path) => window.export_brush_to(path),
            BrushStep::Delete => window.delete_worn_brush_now(),
            BrushStep::Editor(_) => editor::open(window),
            BrushStep::NewBrushEditor(_) => editor::open_new(window),
            BrushStep::SheetStroke => editor::paint_on_sheet(window),
        }
    }
}

/// Selects the rack entry named `name`, so the rack's own selection handler wears it.
fn pick_from_rack(window: &CadmiumWindow, name: &str) {
    match rack_entry(window.upcast_ref(), name) {
        Some((list, row)) => list.select_row(Some(&row)),
        None => glib::g_warning!(LOG_DOMAIN, "smoke found no brush named {name:?}"),
    }
}

/// A rack row named `name`: its tooltip carries the brush's full name, as does its
/// accessible label, which GTK gives no getter for.
fn rack_entry(widget: &gtk::Widget, name: &str) -> Option<(gtk::ListBox, gtk::ListBoxRow)> {
    if let Some(row) = widget.downcast_ref::<gtk::ListBoxRow>()
        && row.tooltip_text().is_some_and(|shown| shown == name)
    {
        let list = row.parent().and_downcast::<gtk::ListBox>()?;
        return Some((list, row.clone()));
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

#[cfg(test)]
mod tests;
