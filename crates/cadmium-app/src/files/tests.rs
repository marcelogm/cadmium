use super::*;

#[test]
fn document_errors_read_in_plain_words() {
    let missing = DocumentError::Io(std::io::Error::from(std::io::ErrorKind::NotFound));
    assert_eq!(describe_document_error(&missing), FILE_NOT_FOUND);
    let full = DocumentError::Io(std::io::Error::from(std::io::ErrorKind::StorageFull));
    assert_eq!(describe_document_error(&full), STORAGE_FULL);
    let broken = DocumentError::Format("missing manifest.json".into());
    assert_eq!(describe_document_error(&broken), NOT_A_PAINTING);
    assert_eq!(
        describe_document_error(&DocumentError::UnsupportedVersion(7)),
        "the painting uses format 7, which this version of Cadmium cannot read"
    );
    let other = DocumentError::Io(std::io::Error::other("boom"));
    assert_eq!(describe_document_error(&other), INPUT_OUTPUT_FAILED);
}

#[test]
fn a_panicked_background_task_reads_as_a_failure() {
    let outcome: std::thread::Result<Result<(), DocumentError>> = Err(Box::new("boom"));
    let failure = finished(outcome).unwrap_err();
    assert_eq!(failure.shown, BACKGROUND_TASK_FAILED);
    assert_eq!(failure.detail, BACKGROUND_TASK_FAILED);
}

#[test]
fn a_document_error_shows_in_plain_words_and_logs_in_full() {
    let failure = finished::<()>(Ok(Err(DocumentError::GpuLost))).unwrap_err();
    assert_eq!(failure.shown, GPU_LOST);
    assert_eq!(failure.detail, DocumentError::GpuLost.to_string());
}

#[test]
fn a_finished_task_hands_back_its_value() {
    assert_eq!(finished(Ok(Ok(7))).ok(), Some(7));
}

#[test]
fn a_painting_write_is_worded_as_saving() {
    let wording = write_wording(WriteKind::Painting);
    assert_eq!(wording.busy, "Saving…");
    assert_eq!(wording.done, "Saved");
    assert_eq!(wording.action, "saving");
    assert_eq!(wording.failed, "Could not save");
}

#[test]
fn an_image_write_is_worded_as_exporting() {
    let wording = write_wording(WriteKind::Image);
    assert_eq!(wording.busy, "Exporting…");
    assert_eq!(wording.done, "Exported");
    assert_eq!(wording.action, "exporting");
    assert_eq!(wording.failed, "Could not export");
}
