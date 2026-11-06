use cadmium_engine::canvas::CanvasError;
use cadmium_engine::gpu::GpuLost;
use cadmium_studio::StudioError;

use super::failure_toast;
use crate::brush_editor::examples::EXAMPLES_FAILED;

const OUT_OF_MEMORY: StudioError = StudioError::Canvas(CanvasError::OutOfVideoMemory);

#[test]
fn a_failure_is_toasted_with_what_failed() {
    assert_eq!(
        failure_toast(EXAMPLES_FAILED, &OUT_OF_MEMORY, false).as_deref(),
        Some(
            "Can't show the example strokes: brush studio canvas: not enough video memory for the canvas"
        )
    );
}

#[test]
fn a_lost_gpu_is_left_to_the_gpu_lost_dialog() {
    assert_eq!(
        failure_toast(EXAMPLES_FAILED, &StudioError::Gpu(GpuLost), false),
        None
    );
    assert_eq!(failure_toast(EXAMPLES_FAILED, &OUT_OF_MEMORY, true), None);
}
