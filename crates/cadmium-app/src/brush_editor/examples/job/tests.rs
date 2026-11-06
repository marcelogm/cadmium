use cadmium_engine::brush::{BrushSpec, factory_brushes};
use cadmium_studio::{ExampleRequest, ExampleSheet, Thumbnail};

use super::{ExampleJob, Tick};
use crate::state::AppState;
use crate::state::test_support::painting_state;

fn sized(size_px: f32) -> BrushSpec {
    BrushSpec {
        size_px,
        ..factory_brushes()[0].clone()
    }
}

/// Ticks `job` until it stops working; a sheet takes three ticks after `start`.
fn finish(job: &mut ExampleJob) -> Thumbnail {
    for _ in 0..3 {
        match job.tick() {
            Tick::Working => {}
            Tick::Done(sheet) => return sheet,
            other => panic!("{other:?}"),
        }
    }
    panic!("the job did not finish in three ticks");
}

fn painted_directly(state: &AppState, brush: &BrushSpec) -> Thumbnail {
    let paint = state.studio_paint();
    let request = ExampleRequest {
        brush,
        paint: &paint,
    };
    ExampleSheet::new().paint(&state.gpu, request).unwrap()
}

#[test]
fn the_job_paints_the_brush_in_the_paintings_paint() {
    let state = painting_state();
    let mut job = ExampleJob::new(&state);
    job.start(&sized(12.0)).unwrap();
    assert_eq!(finish(&mut job), painted_directly(&state, &sized(12.0)));
}

#[test]
fn a_job_with_nothing_started_is_idle() {
    let state = painting_state();
    let mut job = ExampleJob::new(&state);
    assert!(matches!(job.tick(), Tick::Idle));
}

#[test]
fn starting_again_drops_the_painting_under_way() {
    let state = painting_state();
    let mut job = ExampleJob::new(&state);
    job.start(&sized(12.0)).unwrap();
    assert!(matches!(job.tick(), Tick::Working));
    job.start(&sized(16.0)).unwrap();
    assert_eq!(finish(&mut job), painted_directly(&state, &sized(16.0)));
    assert!(matches!(job.tick(), Tick::Idle));
}
