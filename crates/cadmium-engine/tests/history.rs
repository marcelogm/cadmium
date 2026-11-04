use cadmium_engine::{
    canvas::*, gpu::GpuContext, history::History, pigments::PigmentDb, session::PaintSession,
    stroke::StrokeSample,
};

const TILE_BYTES_ALL_TEXTURES: u64 = 64 * 64 * (6 * 8 + 2 * 4);

fn without_wgpu_errors<T>(gpu: &GpuContext, work: impl FnOnce() -> T) -> T {
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let result = work();
    let error = pollster::block_on(scope.pop());
    assert!(error.is_none(), "wgpu error: {error:?}");
    result
}

fn spec(width: u32, height: u32) -> CanvasSpec {
    CanvasSpec {
        width,
        height,
        primer: Primer::WhiteGesso,
        weave: Weave::Linen,
        pigment_slots: vec![
            "ultramarine_blue".into(),
            "cadmium_yellow".into(),
            "titanium_white".into(),
        ],
    }
}

fn session(gpu: &GpuContext) -> PaintSession {
    without_wgpu_errors(gpu, || PaintSession::new(gpu, spec(512, 256)))
}

fn frame(gpu: &GpuContext, s: &mut PaintSession) {
    without_wgpu_errors(gpu, || {
        let mut e = gpu.device.create_command_encoder(&Default::default());
        s.encode_frame(gpu, &mut e);
        gpu.queue.submit([e.finish()]);
    });
}

fn drag(gpu: &GpuContext, s: &mut PaintSession, samples: &[StrokeSample]) {
    for (i, sample) in samples.iter().enumerate() {
        if i == 0 {
            s.begin_stroke(gpu, *sample)
        } else {
            s.add_samples(&[*sample])
        }
        if i % 4 == 0 {
            frame(gpu, s);
        }
    }
    s.end_stroke(gpu);
    frame(gpu, s);
}

fn line(y: f64, from: f64, to: f64, pressure: f32, tilt_x_deg: f32) -> Vec<StrokeSample> {
    let steps = 60;
    (0..=steps)
        .map(|i| StrokeSample {
            x: from + (to - from) * i as f64 / steps as f64,
            y,
            pressure,
            tilt_x_deg,
            tilt_y_deg: 0.0,
            time_ms: i * 4,
        })
        .collect()
}

fn stroke(gpu: &GpuContext, s: &mut PaintSession, y: f64, from: f64, to: f64) {
    drag(gpu, s, &line(y, from, to, 0.8, 20.0));
}

fn snapshot(gpu: &GpuContext, s: &PaintSession) -> Vec<Vec<u8>> {
    StateTexture::PAINT_STATE
        .iter()
        .map(|t| s.canvas.read_bytes(gpu, *t).unwrap())
        .collect()
}

fn save_step(gpu: &GpuContext, history: &mut History, canvas: &Canvas, tiles: &[TileCoord]) {
    without_wgpu_errors(gpu, || {
        history.begin_step();
        let mut e = gpu.device.create_command_encoder(&Default::default());
        history.encode_save_tiles(gpu, canvas, tiles, &mut e);
        gpu.queue.submit([e.finish()]);
        history.commit_step();
    });
}

fn tiles(count: u32) -> Vec<TileCoord> {
    (0..count).map(|x| TileCoord { x, y: 0 }).collect()
}

#[test]
fn undo_restores_every_state_texture_bit_for_bit() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let after_first = snapshot(&gpu, &s);
    stroke(&gpu, &mut s, 128.0, 470.0, 40.0);
    assert!(without_wgpu_errors(&gpu, || s.undo(&gpu)));
    assert_eq!(snapshot(&gpu, &s), after_first);
}

#[test]
fn redo_reapplies_the_stroke() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let painted = snapshot(&gpu, &s);
    s.undo(&gpu);
    assert!(without_wgpu_errors(&gpu, || s.redo(&gpu)));
    assert_eq!(snapshot(&gpu, &s), painted);
}

#[test]
fn dry_is_one_undoable_step() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let wet = snapshot(&gpu, &s);
    assert!(without_wgpu_errors(&gpu, || s.dry(&gpu)));
    s.undo(&gpu);
    assert_eq!(snapshot(&gpu, &s), wet);
}

#[test]
fn a_new_stroke_after_undo_clears_redo() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    s.undo(&gpu);
    stroke(&gpu, &mut s, 140.0, 40.0, 470.0);
    assert!(!s.history.can_redo());
}

#[test]
fn undoing_twice_returns_to_the_blank_canvas() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let blank = snapshot(&gpu, &s);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    stroke(&gpu, &mut s, 100.0, 470.0, 40.0);
    assert!(s.undo(&gpu));
    assert!(s.undo(&gpu));
    assert!(!s.undo(&gpu));
    assert_eq!(snapshot(&gpu, &s), blank);
}

#[test]
fn undo_restores_partial_tiles_at_the_canvas_edge() {
    let gpu = GpuContext::new().unwrap();
    let mut s = without_wgpu_errors(&gpu, || PaintSession::new(&gpu, spec(150, 90)));
    let blank = snapshot(&gpu, &s);
    stroke(&gpu, &mut s, 80.0, 10.0, 145.0);
    assert_ne!(snapshot(&gpu, &s), blank);
    assert!(without_wgpu_errors(&gpu, || s.undo(&gpu)));
    assert_eq!(snapshot(&gpu, &s), blank);
}

#[test]
fn undo_marks_restored_tiles_for_shading() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    s.canvas.dirty.clear();
    s.undo(&gpu);
    assert!(!s.canvas.dirty.is_empty());
    assert!(s.has_pending_work());
}

#[test]
fn history_keeps_at_most_max_steps() {
    let gpu = GpuContext::new().unwrap();
    let mut canvas = Canvas::new(&gpu, &PigmentDb::builtin(), spec(512, 256));
    let mut history = History::new(&gpu, 64 * TILE_BYTES_ALL_TEXTURES, 2);
    for _ in 0..3 {
        save_step(&gpu, &mut history, &canvas, &tiles(1));
    }
    assert!(history.undo(&gpu, &mut canvas));
    assert!(history.undo(&gpu, &mut canvas));
    assert!(!history.undo(&gpu, &mut canvas));
}

#[test]
fn running_out_of_slots_evicts_the_oldest_steps() {
    let gpu = GpuContext::new().unwrap();
    let mut canvas = Canvas::new(&gpu, &PigmentDb::builtin(), spec(512, 256));
    let mut history = History::new(&gpu, 4 * TILE_BYTES_ALL_TEXTURES, 100);
    save_step(&gpu, &mut history, &canvas, &tiles(2));
    save_step(&gpu, &mut history, &canvas, &tiles(2));
    save_step(&gpu, &mut history, &canvas, &tiles(1));
    assert!(history.undo(&gpu, &mut canvas));
    assert!(history.undo(&gpu, &mut canvas));
    assert!(!history.can_undo());
}

#[test]
fn a_step_larger_than_the_budget_is_dropped() {
    let gpu = GpuContext::new().unwrap();
    let canvas = Canvas::new(&gpu, &PigmentDb::builtin(), spec(512, 256));
    let mut history = History::new(&gpu, 2 * TILE_BYTES_ALL_TEXTURES, 100);
    save_step(&gpu, &mut history, &canvas, &tiles(3));
    assert!(!history.can_undo());
}

#[test]
fn saving_the_same_tile_twice_in_a_step_keeps_the_first_copy() {
    let gpu = GpuContext::new().unwrap();
    let mut canvas = Canvas::new(&gpu, &PigmentDb::builtin(), spec(512, 256));
    let mut history = History::new(&gpu, TILE_BYTES_ALL_TEXTURES, 100);
    without_wgpu_errors(&gpu, || {
        history.begin_step();
        let mut e = gpu.device.create_command_encoder(&Default::default());
        history.encode_save_tiles(&gpu, &canvas, &tiles(1), &mut e);
        history.encode_save_tiles(&gpu, &canvas, &tiles(1), &mut e);
        gpu.queue.submit([e.finish()]);
        history.commit_step();
    });
    assert!(history.can_undo());
    assert!(history.undo(&gpu, &mut canvas));
}

#[test]
fn lifting_the_pen_after_every_dab_was_painted_makes_the_stroke_undoable() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let samples = line(128.0, 40.0, 470.0, 0.8, 20.0);
    s.begin_stroke(&gpu, samples[0]);
    s.add_samples(&samples[1..]);
    frame(&gpu, &mut s);
    s.end_stroke(&gpu);
    assert!(s.history.can_undo());
}

#[test]
fn history_capacity_follows_the_budget() {
    let gpu = GpuContext::new().unwrap();
    let history = without_wgpu_errors(&gpu, || {
        History::new(&gpu, 3 * TILE_BYTES_ALL_TEXTURES, 100)
    });
    assert_eq!(history.slot_capacity(), 3);
    assert!(history.is_enabled());
}

#[test]
fn a_budget_below_one_tile_disables_history() {
    let gpu = GpuContext::new().unwrap();
    let canvas = Canvas::new(&gpu, &PigmentDb::builtin(), spec(512, 256));
    let mut history = History::new(&gpu, TILE_BYTES_ALL_TEXTURES - 1, 100);
    assert_eq!(history.slot_capacity(), 0);
    assert!(!history.is_enabled());
    save_step(&gpu, &mut history, &canvas, &tiles(1));
    assert!(!history.can_undo());
}

#[test]
fn a_step_too_large_for_the_history_clears_earlier_steps() {
    let gpu = GpuContext::new().unwrap();
    let canvas = Canvas::new(&gpu, &PigmentDb::builtin(), spec(512, 256));
    let mut history = History::new(&gpu, 4 * TILE_BYTES_ALL_TEXTURES, 100);
    save_step(&gpu, &mut history, &canvas, &tiles(2));
    save_step(&gpu, &mut history, &canvas, &tiles(5));
    assert!(!history.can_undo());
    assert!(!history.can_redo());
}

#[test]
fn after_an_unrecorded_step_the_full_capacity_is_free_again() {
    let gpu = GpuContext::new().unwrap();
    let mut canvas = Canvas::new(&gpu, &PigmentDb::builtin(), spec(512, 256));
    let mut history = History::new(&gpu, 4 * TILE_BYTES_ALL_TEXTURES, 100);
    save_step(&gpu, &mut history, &canvas, &tiles(2));
    save_step(&gpu, &mut history, &canvas, &tiles(5));
    save_step(&gpu, &mut history, &canvas, &tiles(4));
    assert!(history.undo(&gpu, &mut canvas));
    assert!(!history.can_undo());
}

#[test]
fn clearing_the_history_forgets_undo_and_redo() {
    let gpu = GpuContext::new().unwrap();
    let mut canvas = Canvas::new(&gpu, &PigmentDb::builtin(), spec(512, 256));
    let mut history = History::new(&gpu, 4 * TILE_BYTES_ALL_TEXTURES, 100);
    save_step(&gpu, &mut history, &canvas, &tiles(1));
    save_step(&gpu, &mut history, &canvas, &tiles(1));
    assert!(history.undo(&gpu, &mut canvas));
    history.clear();
    assert!(!history.can_undo());
    assert!(!history.can_redo());
    save_step(&gpu, &mut history, &canvas, &tiles(4));
    assert!(history.can_undo());
}

#[test]
fn a_dry_too_large_for_the_history_clears_earlier_steps_and_is_not_undoable() {
    let gpu = GpuContext::new().unwrap();
    let mut s = without_wgpu_errors(&gpu, || PaintSession::new(&gpu, spec(1024, 640)));
    s.history = History::new(&gpu, 64 * TILE_BYTES_ALL_TEXTURES, 100);
    stroke(&gpu, &mut s, 20.0, 10.0, 30.0);
    assert!(s.history.can_undo());
    s.canvas.wet_tiles.mark_all();
    assert!(!without_wgpu_errors(&gpu, || s.dry(&gpu)));
    assert!(!s.history.can_undo());
    assert!(!s.history.can_redo());
}

#[test]
fn an_action_that_saves_no_tiles_keeps_redo() {
    let gpu = GpuContext::new().unwrap();
    let mut canvas = Canvas::new(&gpu, &PigmentDb::builtin(), spec(512, 256));
    let mut history = History::new(&gpu, 4 * TILE_BYTES_ALL_TEXTURES, 100);
    save_step(&gpu, &mut history, &canvas, &tiles(1));
    assert!(history.undo(&gpu, &mut canvas));
    save_step(&gpu, &mut history, &canvas, &[]);
    save_step(&gpu, &mut history, &canvas, &[TileCoord { x: 100, y: 100 }]);
    assert!(history.can_redo());
}

#[test]
fn redoing_a_dry_leaves_its_tiles_dry() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    s.dry(&gpu);
    s.undo(&gpu);
    assert!(!s.canvas.wet_tiles.is_empty());
    assert!(without_wgpu_errors(&gpu, || s.redo(&gpu)));
    assert!(s.canvas.wet_tiles.is_empty());
}

#[test]
fn redoing_a_stroke_marks_its_tiles_wet() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    s.undo(&gpu);
    s.canvas.wet_tiles.clear();
    s.redo(&gpu);
    assert!(!s.canvas.wet_tiles.is_empty());
}
