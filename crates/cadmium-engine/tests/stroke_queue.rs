pub mod support;

use cadmium_engine::{
    canvas::StateTexture, gpu::GpuContext, session::PaintSession, stroke::Tool, surface::Surface,
};
use support::{frame, line, session, without_wgpu_errors};

const FRAMES_TO_DRAIN: usize = 10_000;
const DABS_PER_FRAME: usize = 3;
const SLOT_COUNT: usize = 3;
const FULL_DIP: f32 = 1.0;
const HALF_DIP: f32 = 0.5;
const STROKE_PRESSURE: f32 = 0.8;
const BRUSH_TILT_DEG: f32 = 20.0;
const LAID_KNIFE_TILT_DEG: f32 = 60.0;
const SMALL_DIAMETER_PX: f32 = 12.0;
const UPPER_ROW: f64 = 64.0;
const LOWER_ROW: f64 = 192.0;
const HATCH_ROWS: [f64; 3] = [120.0, 128.0, 136.0];

#[derive(Clone, Copy, Debug)]
enum Act {
    Stroke { surface: Surface, row: f64 },
    Frame,
    Hold(Tool),
    RestoreReserve,
    Dip(f32),
    TopUpKnife,
    CleanBrush,
    Resize { diameter_px: f32 },
}

use Act::{CleanBrush, Dip, Frame, Hold, Resize, RestoreReserve, TopUpKnife};

fn painting(row: f64) -> Act {
    Act::Stroke {
        surface: Surface::Painting,
        row,
    }
}

fn tray(row: f64) -> Act {
    Act::Stroke {
        surface: Surface::Tray,
        row,
    }
}

struct Played {
    session: PaintSession,
    acts_behind_a_backlog: usize,
}

fn snapshot(gpu: &GpuContext, s: &PaintSession) -> Vec<Vec<u8>> {
    [&s.canvas, &s.tray]
        .into_iter()
        .flat_map(|canvas| {
            StateTexture::PAINT_STATE
                .iter()
                .map(|t| canvas.read_bytes(gpu, *t).unwrap())
        })
        .collect()
}

fn drain(gpu: &GpuContext, s: &mut PaintSession) {
    let mut frames = 0;
    while s.has_queued_dabs() && frames < FRAMES_TO_DRAIN {
        frame(gpu, s);
        frames += 1;
    }
    assert!(frames < FRAMES_TO_DRAIN, "the queue never drained");
}

fn stroke_tilt_deg(s: &PaintSession) -> f32 {
    if s.brush.tool == Tool::Knife {
        LAID_KNIFE_TILT_DEG
    } else {
        BRUSH_TILT_DEG
    }
}

fn queue_stroke(s: &mut PaintSession, surface: Surface, row: f64) {
    let samples = line(row, 40.0, 470.0, STROKE_PRESSURE, stroke_tilt_deg(s));
    s.begin_stroke_on(surface, samples[0]);
    s.add_samples(&samples[1..]);
    s.end_stroke();
}

fn perform(gpu: &GpuContext, s: &mut PaintSession, act: Act) {
    let mixture = s.brush.reservoir_mixture(SLOT_COUNT);
    match act {
        Act::Stroke { surface, row } => queue_stroke(s, surface, row),
        Frame => frame(gpu, s),
        Hold(tool) => s.brush.tool = tool,
        RestoreReserve => s.restore_reserve(gpu),
        Dip(amount) => s.dip(gpu, &mixture, amount).unwrap(),
        TopUpKnife => s.top_up_knife(gpu).unwrap(),
        CleanBrush => s.clean_brush(gpu),
        Resize { diameter_px } => {
            s.brush.geometry.max_diameter_px = diameter_px;
            s.resize_tip();
        }
    }
}

fn play(gpu: &GpuContext, acts: &[Act], limit: Option<usize>) -> Played {
    let mut s = session(gpu);
    s.limit_dabs_per_frame(limit);
    let mut acts_behind_a_backlog = 0;
    for act in acts {
        if !matches!(act, Frame | Hold(_)) && s.has_queued_dabs() {
            acts_behind_a_backlog += 1;
        }
        without_wgpu_errors(gpu, || perform(gpu, &mut s, *act));
    }
    drain(gpu, &mut s);
    Played {
        session: s,
        acts_behind_a_backlog,
    }
}

fn assert_a_backlog_paints_the_same_as_painting_at_once(acts: &[Act]) {
    let gpu = GpuContext::new().unwrap();
    let at_once = play(&gpu, acts, None);
    assert_eq!(at_once.acts_behind_a_backlog, 0);
    let spread_out = play(&gpu, acts, Some(DABS_PER_FRAME));
    assert!(
        spread_out.acts_behind_a_backlog > 0,
        "the limit leaves a backlog behind the brush's next moves"
    );
    assert!(
        snapshot(&gpu, &spread_out.session) == snapshot(&gpu, &at_once.session),
        "the backlog painted differently from painting each stroke at once"
    );
}

#[test]
fn a_stroke_begun_behind_a_backlog_leaves_the_backlog_queued() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.limit_dabs_per_frame(Some(DABS_PER_FRAME));
    queue_stroke(&mut s, Surface::Painting, UPPER_ROW);
    frame(&gpu, &mut s);
    let backlog = s.queued_dabs_on(Surface::Painting).len();
    s.restore_reserve(&gpu);
    let next = line(LOWER_ROW, 40.0, 470.0, STROKE_PRESSURE, BRUSH_TILT_DEG);
    s.begin_stroke(next[0]);
    assert!(s.queued_dabs_on(Surface::Painting).len() >= backlog);
}

#[test]
fn a_reserve_restored_behind_a_backlog_reaches_only_the_next_stroke() {
    assert_a_backlog_paints_the_same_as_painting_at_once(&[
        painting(UPPER_ROW),
        Frame,
        RestoreReserve,
        painting(LOWER_ROW),
        Frame,
    ]);
}

#[test]
fn hatched_strokes_queued_behind_each_other_paint_as_drawn_one_by_one() {
    assert_a_backlog_paints_the_same_as_painting_at_once(&[
        painting(HATCH_ROWS[0]),
        Frame,
        RestoreReserve,
        painting(HATCH_ROWS[1]),
        Frame,
        RestoreReserve,
        painting(HATCH_ROWS[2]),
        Frame,
    ]);
}

#[test]
fn a_brush_resized_behind_a_backlog_leaves_the_earlier_stroke_its_size() {
    assert_a_backlog_paints_the_same_as_painting_at_once(&[
        painting(UPPER_ROW),
        Frame,
        Resize {
            diameter_px: SMALL_DIAMETER_PX,
        },
        painting(LOWER_ROW),
        Frame,
    ]);
}

#[test]
fn a_knife_dipped_behind_a_backlog_loads_only_for_the_next_stroke() {
    assert_a_backlog_paints_the_same_as_painting_at_once(&[
        Hold(Tool::Knife),
        Dip(FULL_DIP),
        painting(HATCH_ROWS[0]),
        Frame,
        Dip(HALF_DIP),
        painting(HATCH_ROWS[2]),
        Frame,
    ]);
}

#[test]
fn a_knife_topped_up_behind_a_backlog_tops_up_what_the_backlog_left() {
    assert_a_backlog_paints_the_same_as_painting_at_once(&[
        Hold(Tool::Knife),
        Dip(FULL_DIP),
        painting(HATCH_ROWS[0]),
        Frame,
        TopUpKnife,
        painting(HATCH_ROWS[2]),
        Frame,
    ]);
}

#[test]
fn a_knife_cleaned_behind_a_backlog_stays_a_knife_cleaning_when_the_brush_comes_back() {
    assert_a_backlog_paints_the_same_as_painting_at_once(&[
        Hold(Tool::Knife),
        Dip(FULL_DIP),
        painting(UPPER_ROW),
        Frame,
        CleanBrush,
        Hold(Tool::Brush),
        painting(LOWER_ROW),
        Frame,
    ]);
}

#[test]
fn a_tray_stroke_queued_before_a_painting_stroke_lands_on_the_tray() {
    assert_a_backlog_paints_the_same_as_painting_at_once(&[
        tray(UPPER_ROW),
        Frame,
        RestoreReserve,
        painting(LOWER_ROW),
        Frame,
    ]);
}

#[test]
fn undo_behind_a_backlog_takes_back_only_the_last_stroke() {
    let gpu = GpuContext::new().unwrap();
    let only_the_first = play(&gpu, &[painting(UPPER_ROW), Frame], None);
    let mut queued = session(&gpu);
    queued.limit_dabs_per_frame(Some(DABS_PER_FRAME));
    for act in [
        painting(UPPER_ROW),
        Frame,
        RestoreReserve,
        painting(LOWER_ROW),
        Frame,
    ] {
        without_wgpu_errors(&gpu, || perform(&gpu, &mut queued, act));
    }
    assert!(queued.has_queued_dabs(), "the first stroke is still queued");
    assert!(without_wgpu_errors(&gpu, || queued.undo(&gpu)));
    assert!(queued.history.can_undo(), "the first stroke stays undoable");
    drain(&gpu, &mut queued);
    assert!(snapshot(&gpu, &queued) == snapshot(&gpu, &only_the_first.session));
    assert!(without_wgpu_errors(&gpu, || queued.undo(&gpu)));
    assert!(!queued.history.can_undo());
}
