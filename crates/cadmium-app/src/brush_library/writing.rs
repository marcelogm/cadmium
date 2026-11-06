//! Brush files in the library folder: fresh names, safe writes, safe rewrites and removal.

use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use cadmium_engine::brush::{BRUSH_EXTENSION, BrushSpec, brush_to_json};
use gtk::glib;

use crate::LOG_DOMAIN;

const FALLBACK_SLUG: &str = "brush";
pub(crate) const MAX_SLUG_CHARS: usize = 64;
const NO_FOLDER: &str = "there is no brush folder";

/// Writes `brush` to a new file in `dir`, never replacing a file already there.
pub(super) fn write(dir: Option<&Path>, brush: &BrushSpec) -> std::io::Result<PathBuf> {
    let dir = dir.ok_or_else(|| std::io::Error::other(NO_FOLDER))?;
    std::fs::create_dir_all(dir)?;
    let text = brush_to_json(brush);
    let slug = slug(&brush.name);
    for number in 1.. {
        let path = dir.join(numbered_file_name(&slug, number));
        match write_new(&path, &text) {
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            written => return written.map(|()| path),
        }
    }
    unreachable!("some number is free")
}

/// Rewrites the brush file at `path` with `brush`, never leaving it half-written: the text goes
/// to a hidden file beside it (`.<file name>.tmp`, which the library never reads), is synced,
/// takes the file's permissions, then is renamed over the file, and the folder is synced so the
/// rename lasts (a failed folder sync is only logged: the file is already replaced). A temporary
/// file left by a crash is replaced; nothing else can be writing it, since Cadmium runs as a
/// single instance.
pub(super) fn replace(path: &Path, brush: &BrushSpec) -> std::io::Result<()> {
    let temp = temp_path(path);
    let _ = std::fs::remove_file(&temp);
    write_new(&temp, &brush_to_json(brush))?;
    keep_permissions(path, &temp)
        .and_then(|()| std::fs::rename(&temp, path))
        .inspect_err(|_| {
            let _ = std::fs::remove_file(&temp);
        })?;
    if let Err(error) = sync_folder(path) {
        glib::g_warning!(LOG_DOMAIN, "couldn't sync the brush folder: {error}");
    }
    Ok(())
}

/// Gives `temp` the permissions of the file at `path`, if there is one.
fn keep_permissions(path: &Path, temp: &Path) -> std::io::Result<()> {
    match std::fs::metadata(path) {
        Ok(metadata) => std::fs::set_permissions(temp, metadata.permissions()),
        Err(_) => Ok(()),
    }
}

/// Syncs the folder holding `path`, so a rename into it survives a crash.
#[cfg(unix)]
fn sync_folder(path: &Path) -> std::io::Result<()> {
    match path.parent() {
        Some(dir) => std::fs::File::open(dir)?.sync_all(),
        None => Ok(()),
    }
}

/// Elsewhere a folder can't be opened for syncing; the rename stands as it is.
#[cfg(not(unix))]
fn sync_folder(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

/// `.<file name>.tmp` beside `path`: hidden, and never ending in `.cadbrush`.
fn temp_path(path: &Path) -> PathBuf {
    let name = path.file_name().map_or_else(
        || FALLBACK_SLUG.into(),
        |name| name.to_string_lossy().into_owned(),
    );
    path.with_file_name(format!(".{name}.tmp"))
}

/// Removes a brush file; one that is already gone counts as removed.
pub(super) fn remove(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// Creates `path` (failing if it exists) and writes `text`, removing the file if writing fails.
fn write_new(path: &Path, text: &str) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let written = file
        .write_all(text.as_bytes())
        .and_then(|()| file.sync_all());
    if written.is_err() {
        let _ = std::fs::remove_file(path);
    }
    written
}

/// A file name made only of lowercase ASCII letters, digits and dashes.
pub(crate) fn file_name_for(name: &str) -> String {
    numbered_file_name(&slug(name), 1)
}

fn numbered_file_name(slug: &str, number: u32) -> String {
    match number {
        1 => format!("{slug}.{BRUSH_EXTENSION}"),
        number => format!("{slug}-{number}.{BRUSH_EXTENSION}"),
    }
}

fn slug(name: &str) -> String {
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
    slug.truncate(MAX_SLUG_CHARS);
    let slug = slug.trim_end_matches('-');
    match slug {
        "" => FALLBACK_SLUG.to_string(),
        slug => slug.to_string(),
    }
}
