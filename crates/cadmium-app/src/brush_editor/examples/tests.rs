use cadmium_engine::canvas::CanvasError;
use cadmium_engine::gpu::GpuLost;
use cadmium_studio::StudioError;

use super::failure_toast;

#[test]
fn a_sheet_out_of_video_memory_is_toasted() {
    let error = StudioError::Canvas(CanvasError::OutOfVideoMemory);
    assert_eq!(
        failure_toast(&error, false).as_deref(),
        Some(
            "Can't show the example strokes: brush studio canvas: not enough video memory for the canvas"
        )
    );
}

#[test]
fn a_lost_gpu_is_left_to_the_gpu_lost_dialog() {
    assert_eq!(failure_toast(&StudioError::Gpu(GpuLost), false), None);
    let error = StudioError::Canvas(CanvasError::OutOfVideoMemory);
    assert_eq!(failure_toast(&error, true), None);
}
