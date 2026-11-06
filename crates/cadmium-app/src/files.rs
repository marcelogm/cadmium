use std::path::Path;

use cadmium_engine::document::DocumentError;
use gtk::glib;

use crate::LOG_DOMAIN;
use crate::window::CadmiumWindow;

pub const PAINTING_EXTENSION: &str = "cadmium";
pub const IMAGE_EXTENSION: &str = "png";
const BACKGROUND_TASK_FAILED: &str = "the background task failed";
const FILE_NOT_FOUND: &str = "the file does not exist";
const PERMISSION_DENIED: &str = "no permission to access the file";
const ALREADY_EXISTS: &str = "the file already exists";
const STORAGE_FULL: &str = "there is no free disk space";
const READ_ONLY: &str = "the location is read-only";
const UNREADABLE_DATA: &str = "the file is corrupt or incomplete";
const INPUT_OUTPUT_FAILED: &str = "read or write failure";
const NOT_A_PAINTING: &str = "the file is not a valid Cadmium painting";
const GPU_LOST: &str = "the GPU was reset";
pub const OUT_OF_VIDEO_MEMORY: &str = "not enough video memory";

mod close;
pub(crate) mod dialogs;
mod read;
mod title;
mod write;

struct Failure {
    shown: String,
    detail: String,
}

pub fn describe_document_error(error: &DocumentError) -> String {
    match error {
        DocumentError::Io(error) => describe_io_error(error.kind()).to_string(),
        DocumentError::Format(_) => NOT_A_PAINTING.to_string(),
        DocumentError::UnsupportedVersion(version) => {
            format!("the painting uses format {version}, which this version of Cadmium cannot read")
        }
        DocumentError::GpuLost => GPU_LOST.to_string(),
        DocumentError::OutOfVideoMemory => OUT_OF_VIDEO_MEMORY.to_string(),
    }
}

fn describe_io_error(kind: std::io::ErrorKind) -> &'static str {
    match kind {
        std::io::ErrorKind::NotFound => FILE_NOT_FOUND,
        std::io::ErrorKind::PermissionDenied => PERMISSION_DENIED,
        std::io::ErrorKind::AlreadyExists => ALREADY_EXISTS,
        std::io::ErrorKind::StorageFull | std::io::ErrorKind::QuotaExceeded => STORAGE_FULL,
        std::io::ErrorKind::ReadOnlyFilesystem => READ_ONLY,
        std::io::ErrorKind::InvalidData | std::io::ErrorKind::UnexpectedEof => UNREADABLE_DATA,
        _ => INPUT_OUTPUT_FAILED,
    }
}

fn finished<T>(outcome: std::thread::Result<Result<T, DocumentError>>) -> Result<T, Failure> {
    match outcome {
        Ok(result) => result.map_err(|error| Failure {
            shown: describe_document_error(&error),
            detail: error.to_string(),
        }),
        Err(_) => Err(Failure {
            shown: BACKGROUND_TASK_FAILED.to_string(),
            detail: BACKGROUND_TASK_FAILED.to_string(),
        }),
    }
}

impl CadmiumWindow {
    fn report_failure(&self, action: &str, path: &Path, cannot: &str, failure: &Failure) {
        glib::g_warning!(
            LOG_DOMAIN,
            "{} {} failed: {}",
            action,
            path.display(),
            failure.detail
        );
        self.show_toast(&format!("{cannot}: {}", failure.shown));
    }
}

#[cfg(test)]
mod tests;
