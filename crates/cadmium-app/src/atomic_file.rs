//! Writing a file so it is never left half-written.

use std::io::Write;
use std::path::Path;

use gtk::glib;

use crate::LOG_DOMAIN;

/// Replaces the file at `path` with `text`, never leaving it half-written: the text goes to
/// `temp` (which must be beside `path`), is synced, takes the file's permissions, then is renamed
/// over the file, and the folder is synced so the rename lasts (a failed folder sync is only
/// logged: the file is already replaced). A temporary file left by a crash is replaced; nothing
/// else can be writing it, since Cadmium runs as a single instance.
pub(crate) fn replace(path: &Path, temp: &Path, text: &str) -> std::io::Result<()> {
    let _ = std::fs::remove_file(temp);
    write_new(temp, text)?;
    keep_permissions(path, temp)
        .and_then(|()| std::fs::rename(temp, path))
        .inspect_err(|_| {
            let _ = std::fs::remove_file(temp);
        })?;
    if let Err(error) = sync_folder(path) {
        glib::g_warning!(
            LOG_DOMAIN,
            "couldn't sync the folder of {}: {error}",
            path.display()
        );
    }
    Ok(())
}

/// Creates `path` (failing if it exists) and writes `text`, removing the file if writing fails.
pub(crate) fn write_new(path: &Path, text: &str) -> std::io::Result<()> {
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
