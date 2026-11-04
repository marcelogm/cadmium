use std::path::PathBuf;

use cadmium_engine::canvas::{CanvasError, CanvasSpec};
use cadmium_engine::document::{DocumentSnapshot, ExportedImage};
use cadmium_engine::gpu::GpuLost;

use super::AppState;
use super::recipe_for_palette;
use crate::presets::starting_recipe;
use crate::writes::EditMark;

impl AppState {
    pub fn start_painting(&mut self, spec: CanvasSpec) -> Result<(), CanvasError> {
        self.release();
        let old_palette = self.session.canvas.spec.pigment_slots.clone();
        let replaced = self.session.replace_document(&self.gpu, spec);
        self.adopt_document(&old_palette, replaced.map(|()| None));
        replaced
    }

    pub fn open_painting(
        &mut self,
        snapshot: DocumentSnapshot,
        path: PathBuf,
    ) -> Result<(), CanvasError> {
        self.release();
        let old_palette = self.session.canvas.spec.pigment_slots.clone();
        let restored = snapshot.restore_into(&self.gpu, &mut self.session);
        self.adopt_document(&old_palette, restored.map(|()| Some(path)));
        restored
    }

    pub fn capture_document(&mut self) -> Result<DocumentSnapshot, GpuLost> {
        DocumentSnapshot::capture(&self.gpu, &mut self.session)
    }

    pub fn render_export(&mut self) -> Result<ExportedImage, GpuLost> {
        ExportedImage::render(&self.gpu, &mut self.session)
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
        let palette = &self.session.canvas.spec.pigment_slots;
        let brush = &mut self.session.brush;
        brush.tool = self.resting_tool;
        brush.recipe = recipe_for_palette(&brush.recipe, old_palette, palette);
        if brush.recipe.is_empty() {
            brush.recipe = starting_recipe(palette);
        }
        if self.tube_slot >= palette.len() {
            self.tube_slot = 0;
        }
        self.reload_brush();
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
