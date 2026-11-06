//! Saving an edited brush: over itself for a user brush, or as a new user brush.

use cadmium_engine::brush::BrushSpec;

use super::{BrushLibrary, LibraryError, UserBrush, writing};

/// What happened to a saved brush.
#[derive(Debug)]
pub struct Saved {
    pub index: usize,
    /// The name it was saved under.
    pub name: String,
    /// Whether that name had to change because another brush has the wanted one.
    pub renamed: bool,
    /// Why the brush could not be written to the brush folder, if it could not: it then lasts
    /// for this session only.
    pub save_error: Option<std::io::Error>,
}

impl BrushLibrary {
    /// Replaces the user brush at `index`, which must still be `original`, with `brush`, in its
    /// own file (whatever the new name). A name another brush has gets a number; the brush's own
    /// name doesn't count. A brush that never got a file gets one now.
    pub fn save(
        &mut self,
        index: usize,
        original: &BrushSpec,
        brush: &BrushSpec,
    ) -> Result<Saved, LibraryError> {
        if !self.is_user(index) || self.get(index) != Some(original) {
            return Err(LibraryError::Changed);
        }
        let name = self.unique_name_except(&brush.name, Some(index));
        let renamed = name != brush.name;
        let saved = BrushSpec {
            name: name.clone(),
            ..brush.clone()
        };
        let position = index - self.factory.len();
        let save_error = self.rewrite(position, &saved).err();
        self.user[position].brush = saved;
        Ok(Saved {
            index,
            name,
            renamed,
            save_error,
        })
    }

    /// Adds `brush` as a new user brush with a file of its own, numbering a name another brush has.
    pub fn add_new(&mut self, mut brush: BrushSpec) -> Saved {
        let wanted = std::mem::take(&mut brush.name);
        brush.name = self.unique_name(&wanted);
        let (path, save_error) = match writing::write(self.dir.as_deref(), &brush) {
            Ok(path) => (Some(path), None),
            Err(error) => (None, Some(error)),
        };
        let name = brush.name.clone();
        self.user.push(UserBrush { brush, path });
        Saved {
            index: self.factory.len() + self.user.len() - 1,
            renamed: name != wanted,
            name,
            save_error,
        }
    }

    /// Writes the user brush at `position` over its file, or to a fresh file when it has none.
    fn rewrite(&mut self, position: usize, brush: &BrushSpec) -> std::io::Result<()> {
        if let Some(path) = &self.user[position].path {
            return writing::replace(path, brush);
        }
        let path = writing::write(self.dir.as_deref(), brush)?;
        self.user[position].path = Some(path);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
