//! Wearing, importing, saving and deleting brushes from the brush library.

use cadmium_engine::brush::BrushSpec;

use super::AppState;
use crate::brush_library::{Imported, LibraryError, Saved, index_after_delete, worn_brush};

const DEFAULT_BRUSH_INDEX: usize = 0;
/// The name of a worn brush that is not in the library.
const CURRENT_BRUSH_NAME: &str = "Current Brush";

impl AppState {
    /// The worn brush as a library brush at the current size.
    pub fn worn_brush_spec(&self) -> BrushSpec {
        let settings = &self.session.brush;
        let size_px = settings.geometry.max_diameter_px;
        match self.worn_library_brush().and_then(|i| self.brushes.get(i)) {
            Some(brush) => BrushSpec {
                size_px,
                ..brush.clone()
            },
            None => BrushSpec {
                name: CURRENT_BRUSH_NAME.into(),
                size_px,
                head: settings.head,
                hair: settings.hair,
            },
        }
    }

    /// The library brush being worn, preferring the one last picked over its twins.
    pub fn worn_library_brush(&self) -> Option<usize> {
        worn_brush(
            self.brushes.all(),
            self.worn_brush_index,
            &self.session.brush,
        )
    }

    /// Wears the library brush at `index`; false when there is none.
    pub fn wear_library_brush(&mut self, index: usize) -> bool {
        let Some(brush) = self.brushes.get(index).cloned() else {
            return false;
        };
        self.wear_brush(&brush);
        self.worn_brush_index = Some(index);
        true
    }

    /// Adds a brush from a file's text to the library and wears it.
    pub fn import_brush(&mut self, text: &str) -> Result<Imported, LibraryError> {
        let imported = self.brushes.import(text)?;
        self.wear_library_brush(imported.index);
        Ok(imported)
    }

    /// Saves an edited user brush over itself, then wears it at its suggested size (C1).
    pub fn save_library_brush(
        &mut self,
        index: usize,
        original: &BrushSpec,
        brush: &BrushSpec,
    ) -> Result<Saved, LibraryError> {
        let saved = self.brushes.save(index, original, brush)?;
        self.wear_library_brush(saved.index);
        Ok(saved)
    }

    /// Adds an edited brush as a new user brush, then wears it at its suggested size (C1).
    pub fn add_library_brush(&mut self, brush: BrushSpec) -> Saved {
        let saved = self.brushes.add_new(brush);
        self.wear_library_brush(saved.index);
        saved
    }

    /// Deletes a user brush; wearing it falls back to the default brush.
    pub fn delete_library_brush(&mut self, index: usize) -> Result<(), LibraryError> {
        let worn = self.worn_library_brush();
        self.brushes.delete(index)?;
        if worn == Some(index) {
            self.wear_library_brush(DEFAULT_BRUSH_INDEX);
        } else {
            self.worn_brush_index = index_after_delete(self.worn_brush_index, index);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
