//! Wearing, importing and deleting brushes from the brush library.

use super::AppState;
use crate::brush_library::{Imported, LibraryError, index_after_delete, worn_brush};

const DEFAULT_BRUSH_INDEX: usize = 0;

impl AppState {
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
