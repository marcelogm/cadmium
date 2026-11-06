use cadmium_engine::canvas::CanvasError;
use cadmium_engine::gpu::GpuLost;
use cadmium_studio::StudioError;

use super::failure_toast;
use super::view::markup;

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

#[test]
fn a_labels_arrow_is_set_in_sans() {
    let label = markup("LIGHT → HEAVY");
    assert!(
        label.contains("LIGHT <span font_family=\"sans\">→</span> HEAVY"),
        "{label}"
    );
}

#[test]
fn a_labels_text_is_escaped() {
    let label = markup("A & <B>");
    assert!(label.contains(">A &amp; &lt;B&gt;</span>"), "{label}");
}
