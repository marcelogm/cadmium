use cadmium_engine::brush::factory_brushes;
use cadmium_engine::canvas::CanvasError;
use cadmium_studio::{StudioError, TEST_SHEET_SIZE};

use super::{SHEET_FAILED, new_sheet};
use crate::brush_editor::failing::failure_toast;
use crate::state::test_support::painting_state;

#[test]
fn a_sheet_is_made_at_the_test_sheet_size() {
    let state = painting_state();
    let sheet = new_sheet(&state, &factory_brushes()[0]).unwrap();
    assert_eq!(sheet.size(), TEST_SHEET_SIZE);
}

#[test]
fn a_sheet_out_of_video_memory_names_the_test_sheet() {
    let error = StudioError::Canvas(CanvasError::OutOfVideoMemory);
    assert_eq!(
        failure_toast(SHEET_FAILED, &error, false).as_deref(),
        Some(
            "Can't show the test sheet: brush studio canvas: not enough video memory for the canvas"
        )
    );
}
