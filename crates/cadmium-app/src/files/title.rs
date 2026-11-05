use std::path::Path;

use adw::prelude::*;

use crate::window::CadmiumWindow;

const APPLICATION_NAME: &str = "Cadmium";
pub(super) const UNTITLED: &str = "Untitled";

pub(super) fn document_name(file: Option<&Path>) -> String {
    file.and_then(Path::file_stem)
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| UNTITLED.to_string())
}

fn window_title(file: Option<&Path>) -> String {
    match file {
        Some(_) => format!("{} — {APPLICATION_NAME}", document_name(file)),
        None => APPLICATION_NAME.to_string(),
    }
}

impl CadmiumWindow {
    pub fn show_document_title(&self) {
        let title = window_title(self.state().borrow().file.as_deref());
        self.set_title(Some(&title));
    }
}

#[cfg(test)]
mod tests;
