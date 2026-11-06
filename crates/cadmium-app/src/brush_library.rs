//! The brushes a painter can pick: the factory ones, then their own from disk.

use std::path::{Path, PathBuf};

use cadmium_engine::brush::{
    BRUSH_EXTENSION, BrushFileError, BrushSpec, brush_from_json, factory_brushes,
};

#[derive(Debug)]
pub enum LibraryError {
    Unreadable(BrushFileError),
    NotAUserBrush,
    Changed,
    Io(std::io::Error),
}

impl std::fmt::Display for LibraryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LibraryError::Unreadable(error) => write!(f, "{error}"),
            LibraryError::NotAUserBrush => write!(f, "that brush is a factory brush"),
            LibraryError::Changed => write!(f, "the brush changed while it was being edited"),
            LibraryError::Io(error) => write!(f, "{error}"),
        }
    }
}

/// What happened to an imported brush.
#[derive(Debug)]
pub struct Imported {
    pub index: usize,
    pub renamed: bool,
    pub adjusted: bool,
    /// Why the brush could not be written to the brush folder, if it could not.
    pub save_error: Option<std::io::Error>,
}

impl Imported {
    pub fn saved(&self) -> bool {
        self.save_error.is_none()
    }
}

struct UserBrush {
    brush: BrushSpec,
    path: Option<PathBuf>,
}

pub struct BrushLibrary {
    dir: Option<PathBuf>,
    factory: Vec<BrushSpec>,
    user: Vec<UserBrush>,
}

impl BrushLibrary {
    /// Just the factory brushes; imports live for the session and are never written.
    pub fn factory_only() -> BrushLibrary {
        BrushLibrary {
            dir: None,
            factory: factory_brushes(),
            user: Vec::new(),
        }
    }

    /// Reads every `.cadbrush` in `dir`, skipping unreadable ones with a warning each.
    pub fn load(dir: PathBuf) -> (BrushLibrary, Vec<String>) {
        let mut warnings = Vec::new();
        let mut library = BrushLibrary {
            dir: Some(dir),
            ..BrushLibrary::factory_only()
        };
        let paths = library.dir.as_deref().map(brush_files).unwrap_or_default();
        for path in paths {
            match read_brush(&path) {
                Ok(brush) => library.add(brush, Some(path)),
                Err(error) => warnings.push(format!("{}: {error}", path.display())),
            }
        }
        (library, warnings)
    }

    /// The default library location: `<user data dir>/cadmium/brushes`.
    pub fn default_dir() -> PathBuf {
        gtk::glib::user_data_dir().join("cadmium").join("brushes")
    }

    pub fn all(&self) -> impl Iterator<Item = &BrushSpec> {
        self.factory
            .iter()
            .chain(self.user.iter().map(|user| &user.brush))
    }

    pub fn get(&self, index: usize) -> Option<&BrushSpec> {
        self.all().nth(index)
    }

    pub fn is_user(&self, index: usize) -> bool {
        index >= self.factory.len() && index < self.factory.len() + self.user.len()
    }

    pub fn import(&mut self, text: &str) -> Result<Imported, LibraryError> {
        let read = brush_from_json(text).map_err(LibraryError::Unreadable)?;
        let saved = self.add_new(read.brush);
        Ok(Imported {
            index: saved.index,
            renamed: saved.renamed,
            adjusted: read.adjusted,
            save_error: saved.save_error,
        })
    }

    /// Removes a user brush's file, then the brush; a file already gone counts as removed.
    pub fn delete(&mut self, index: usize) -> Result<(), LibraryError> {
        if !self.is_user(index) {
            return Err(LibraryError::NotAUserBrush);
        }
        let position = index - self.factory.len();
        if let Some(path) = &self.user[position].path {
            writing::remove(path).map_err(LibraryError::Io)?;
        }
        self.user.remove(position);
        Ok(())
    }

    /// Adds a brush read from disk, numbering its name when another brush has it.
    fn add(&mut self, mut brush: BrushSpec, path: Option<PathBuf>) {
        brush.name = self.unique_name(&brush.name);
        self.user.push(UserBrush { brush, path });
    }

    fn unique_name(&self, wanted: &str) -> String {
        self.unique_name_except(wanted, None)
    }

    /// `wanted`, or `wanted (n)` with the first free number; the brush at `skip` doesn't count.
    fn unique_name_except(&self, wanted: &str, skip: Option<usize>) -> String {
        let taken = |name: &str| {
            self.all()
                .enumerate()
                .any(|(index, brush)| Some(index) != skip && brush.name == name)
        };
        if !taken(wanted) {
            return wanted.to_string();
        }
        (2..)
            .map(|number| format!("{wanted} ({number})"))
            .find(|name| !taken(name))
            .expect("some number is free")
    }
}

fn brush_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == BRUSH_EXTENSION))
        .collect();
    paths.sort();
    paths
}

fn read_brush(path: &Path) -> Result<BrushSpec, String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    brush_from_json(&text)
        .map(|read| read.brush)
        .map_err(|error| error.to_string())
}

mod saving;
mod worn;
mod writing;

pub(crate) use saving::Saved;
pub(crate) use worn::{export_name, index_after_delete, worn_brush};

#[cfg(test)]
use writing::MAX_SLUG_CHARS;
pub(crate) use writing::file_name_for;

#[cfg(test)]
mod tests;
