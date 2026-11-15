use std::path::PathBuf;

use cadmium_engine::canvas::{CanvasError, CanvasSpec};
use cadmium_engine::document::{DocumentSnapshot, ExportedImage};
use cadmium_engine::gpu::GpuLost;
use cadmium_engine::sheet::sheet_spec;

use super::recipe::recipe_for_palette;
use super::{AppState, EaselId};
use crate::presets::starting_recipe;
use crate::writes::EditMark;

impl AppState {
    pub fn start_painting(&mut self, spec: CanvasSpec) -> Result<(), CanvasError> {
        self.release(EaselId::Painting);
        self.release(EaselId::Sheet);
        let old_palette = self.painting.session.canvas.spec.pigment_slots.clone();
        let replaced = self.painting.session.replace_document(&self.gpu, spec);
        let replaced = replaced.and(self.blank_sheet());
        self.adopt_document(&old_palette, replaced.map(|()| None));
        replaced
    }

    /// Gives the painting a blank sheet with its paints.
    fn blank_sheet(&mut self) -> Result<(), CanvasError> {
        let spec = sheet_spec(&self.painting.session.canvas.spec.pigment_slots);
        self.sheet.session.replace_document(&self.gpu, spec)
    }

    pub fn open_painting(
        &mut self,
        snapshot: DocumentSnapshot,
        path: PathBuf,
    ) -> Result<(), CanvasError> {
        self.release(EaselId::Painting);
        self.release(EaselId::Sheet);
        let old_palette = self.painting.session.canvas.spec.pigment_slots.clone();
        let restored = snapshot.restore_into(
            &self.gpu,
            &mut self.painting.session,
            &mut self.sheet.session,
        );
        if restored.is_err() {
            let _ = self.blank_sheet();
        }
        self.adopt_document(&old_palette, restored.map(|()| Some(path)));
        restored
    }

    pub fn capture_document(&mut self) -> Result<DocumentSnapshot, GpuLost> {
        let painting = &mut self.painting.session;
        DocumentSnapshot::capture(&self.gpu, painting, &mut self.sheet.session)
    }

    pub fn render_export(&mut self) -> Result<ExportedImage, GpuLost> {
        ExportedImage::render(&self.gpu, &mut self.painting.session)
    }

    pub fn has_unsaved_edits(&self) -> bool {
        self.edits.has_unsaved_edits()
    }

    pub fn edit_mark(&self) -> EditMark {
        self.edits.mark()
    }

    pub fn saved_to(&mut self, path: PathBuf, mark: EditMark) {
        self.edits.saved_at(mark);
        self.file = Some(path);
    }

    fn adopt_document(
        &mut self,
        old_palette: &[String],
        file: Result<Option<PathBuf>, CanvasError>,
    ) {
        let palette = &self.painting.session.canvas.spec.pigment_slots;
        let brush = &mut self.painting.session.brush;
        brush.tool = self.painting.resting_tool;
        brush.recipe = recipe_for_palette(&brush.recipe, old_palette, palette);
        if brush.recipe.is_empty() {
            brush.recipe = starting_recipe(palette);
        }
        self.reload_brush();
        self.sheet.session.brush.recipe.clear();
        let gpu = self.gpu.clone();
        self.sheet.measure_brush_once_painted(&gpu);
        match file {
            Ok(file) => {
                self.edits.forget_unsaved();
                self.file = file;
            }
            Err(_) => self.file = None,
        }
        self.wake();
    }
}

#[cfg(test)]
mod tests;
