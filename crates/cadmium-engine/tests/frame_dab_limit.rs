pub mod support;

use cadmium_engine::{canvas::StateTexture, gpu::GpuContext, session::PaintSession};
use support::{frame, line, session, without_wgpu_errors};

const FRAMES_TO_DRAIN: usize = 10_000;

fn snapshot(gpu: &GpuContext, s: &PaintSession) -> Vec<Vec<u8>> {
    StateTexture::PAINT_STATE
        .iter()
        .map(|t| s.canvas.read_bytes(gpu, *t).unwrap())
        .collect()
}

fn queue_whole_stroke(s: &mut PaintSession) {
    let samples = line(128.0, 40.0, 470.0, 0.8, 20.0);
    s.begin_stroke(samples[0]);
    s.add_samples(&samples[1..]);
    s.end_stroke();
}

fn drain(gpu: &GpuContext, s: &mut PaintSession) -> usize {
    let mut frames = 0;
    while s.has_pending_work() && frames < FRAMES_TO_DRAIN {
        frame(gpu, s);
        frames += 1;
    }
    frames
}

#[test]
fn a_limited_frame_paints_no_more_dabs_than_its_limit_and_leaves_the_rest_queued() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.limit_dabs_per_frame(Some(3));
    queue_whole_stroke(&mut s);
    frame(&gpu, &mut s);
    assert_eq!(s.last_frame_dabs(), 3);
    assert!(s.has_queued_dabs());
}

#[test]
fn a_stroke_painted_a_few_dabs_a_frame_comes_out_the_same_as_painted_all_at_once() {
    let gpu = GpuContext::new().unwrap();
    let mut at_once = session(&gpu);
    queue_whole_stroke(&mut at_once);
    drain(&gpu, &mut at_once);
    let mut spread_out = session(&gpu);
    spread_out.limit_dabs_per_frame(Some(2));
    queue_whole_stroke(&mut spread_out);
    let frames = drain(&gpu, &mut spread_out);
    assert!(
        frames > 1,
        "the limit spreads the stroke over several frames"
    );
    assert_eq!(snapshot(&gpu, &spread_out), snapshot(&gpu, &at_once));
}

#[test]
fn a_stroke_ended_with_dabs_still_queued_becomes_undoable_once_they_are_painted() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let blank = snapshot(&gpu, &s);
    s.limit_dabs_per_frame(Some(2));
    queue_whole_stroke(&mut s);
    frame(&gpu, &mut s);
    assert!(!s.history.can_undo(), "the stroke is still being painted");
    drain(&gpu, &mut s);
    assert!(without_wgpu_errors(&gpu, || s.undo(&gpu)));
    assert_eq!(snapshot(&gpu, &s), blank);
}

#[test]
fn undo_while_dabs_are_queued_paints_them_first_and_takes_back_the_whole_stroke() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let blank = snapshot(&gpu, &s);
    s.limit_dabs_per_frame(Some(2));
    queue_whole_stroke(&mut s);
    frame(&gpu, &mut s);
    assert!(without_wgpu_errors(&gpu, || s.undo(&gpu)));
    assert!(!s.has_queued_dabs());
    assert_eq!(snapshot(&gpu, &s), blank);
}
