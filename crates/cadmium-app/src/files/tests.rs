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
