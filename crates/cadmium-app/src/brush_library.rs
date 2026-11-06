//! The brushes a painter can pick: the factory ones, then their own from disk.

use std::path::{Path, PathBuf};

use cadmium_engine::brush::{
    BRUSH_EXTENSION, BrushFileError, BrushSettings, BrushSpec, brush_from_json, brush_to_json,
    factory_brushes,
};

const FALLBACK_SLUG: &str = "brush";
const CUSTOM_BRUSH_NAME: &str = "Custom Brush";

#[derive(Debug)]
pub enum LibraryError {
    Unreadable(BrushFileError),
    NotAUserBrush,
    Io(std::io::Error),
}

impl std::fmt::Display for LibraryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LibraryError::Unreadable(error) => write!(f, "{error}"),
            LibraryError::NotAUserBrush => write!(f, "factory brushes can't be deleted"),
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
    pub saved: bool,
}

struct UserBrush {
    brush: BrushSpec,
    path: Option<PathBuf>,
}

pub struct BrushLibrary {
    dir: PathBuf,
    factory: Vec<BrushSpec>,
    user: Vec<UserBrush>,
}

impl BrushLibrary {
    /// Reads every `.cadbrush` in `dir`, skipping unreadable ones with a warning each.
    pub fn load(dir: PathBuf) -> (BrushLibrary, Vec<String>) {
        let mut warnings = Vec::new();
        let mut user = Vec::new();
        for path in brush_files(&dir) {
            match read_brush(&path) {
                Ok(brush) => user.push(UserBrush {
                    brush,
                    path: Some(path),
                }),
                Err(error) => warnings.push(format!("{}: {error}", path.display())),
            }
        }
        let library = BrushLibrary {
            dir,
            factory: factory_brushes(),
            user,
        };
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
        let mut brush = read.brush;
        let name = self.unique_name(&brush.name);
        let renamed = name != brush.name;
        brush.name = name;
        let path = self.write(&brush).ok();
        let saved = path.is_some();
        self.user.push(UserBrush { brush, path });
        let index = self.factory.len() + self.user.len() - 1;
        Ok(Imported {
            index,
            renamed,
            adjusted: read.adjusted,
            saved,
        })
    }

    pub fn delete(&mut self, index: usize) -> Result<(), LibraryError> {
        if !self.is_user(index) {
            return Err(LibraryError::NotAUserBrush);
        }
        let removed = self.user.remove(index - self.factory.len());
        if let Some(path) = removed.path {
            std::fs::remove_file(path).map_err(LibraryError::Io)?;
        }
        Ok(())
    }

    fn unique_name(&self, wanted: &str) -> String {
        let taken = |name: &str| self.all().any(|brush| brush.name == name);
        if !taken(wanted) {
            return wanted.to_string();
        }
        (2..)
            .map(|number| format!("{wanted} ({number})"))
            .find(|name| !taken(name))
            .expect("some number is free")
    }

    fn write(&self, brush: &BrushSpec) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(&self.dir)?;
        let path = free_path(&self.dir, &file_name_for(&brush.name));
        std::fs::write(&path, brush_to_json(brush))?;
        Ok(path)
    }
}

/// The name the worn brush is exported under, and its library index when it is a library brush.
pub(crate) fn export_name(
    brushes: &BrushLibrary,
    settings: &BrushSettings,
) -> (String, Option<usize>) {
    let index = brushes
        .all()
        .position(|brush| brush.head == settings.head && brush.hair == settings.hair);
    let name = index
        .and_then(|index| brushes.get(index))
        .map_or_else(|| CUSTOM_BRUSH_NAME.to_string(), |brush| brush.name.clone());
    (name, index)
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

/// A file name made only of lowercase ASCII letters, digits and dashes.
pub(crate) fn file_name_for(name: &str) -> String {
    let mut slug = String::new();
    for word in name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        if !slug.is_empty() {
            slug.push('-');
        }
        slug.push_str(&word.to_ascii_lowercase());
    }
    if slug.is_empty() {
        slug.push_str(FALLBACK_SLUG);
    }
    format!("{slug}.{BRUSH_EXTENSION}")
}

fn free_path(dir: &Path, file_name: &str) -> PathBuf {
    let candidate = dir.join(file_name);
    if !candidate.exists() {
        return candidate;
    }
    let stem = file_name.trim_end_matches(&format!(".{BRUSH_EXTENSION}"));
    (2..)
        .map(|number| dir.join(format!("{stem}-{number}.{BRUSH_EXTENSION}")))
        .find(|path| !path.exists())
        .expect("some number is free")
}

#[cfg(test)]
mod tests;
