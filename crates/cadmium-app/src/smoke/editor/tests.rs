use super::{EditorShown, Readiness, readiness};

#[test]
fn without_an_editor_step_the_screenshot_is_ready() {
    for shown in [
        EditorShown::Closed,
        EditorShown::Unmapped,
        EditorShown::Mapped,
    ] {
        assert_eq!(readiness(false, shown), Readiness::Ready, "{shown:?}");
    }
}

#[test]
fn an_expected_editor_is_ready_once_mapped() {
    assert_eq!(readiness(true, EditorShown::Unmapped), Readiness::Wait);
    assert_eq!(readiness(true, EditorShown::Mapped), Readiness::Ready);
}

#[test]
fn an_expected_editor_that_closed_ends_the_run() {
    assert_eq!(
        readiness(true, EditorShown::Closed),
        Readiness::EditorClosed
    );
}
