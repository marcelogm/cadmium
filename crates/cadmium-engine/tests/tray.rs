use cadmium_engine::{
    canvas::*,
    draw_up::DrawUpRequest,
    engine::PaintEngine,
    gpu::GpuContext,
    pigments::{Mixture, PigmentDb},
    session::{ClearOutcome, PaintSession},
    stroke::{StrokeSample, Tool},
    surface::{Surface, TRAY_HEIGHT, TRAY_WIDTH, tray_spec},
    tube::Nozzle,
};

fn spec() -> CanvasSpec {
    CanvasSpec {
        width: 512,
        height: 256,
        primer: Primer::WhiteGesso,
        weave: Weave::Linen,
        pigment_slots: vec!["ultramarine_blue".into(), "cadmium_yellow".into()],
    }
}

fn session(gpu: &GpuContext) -> PaintSession {
    PaintSession::new(gpu, spec()).unwrap()
}

fn frame(gpu: &GpuContext, s: &mut PaintSession) {
    let mut e = gpu.device.create_command_encoder(&Default::default());
    s.encode_frame(gpu, &mut e);
    s.encode_tray_frame(gpu, &mut e);
    gpu.queue.submit([e.finish()]);
}

fn sample(x: f64, y: f64, time_ms: u32) -> StrokeSample {
    StrokeSample {
        x,
        y,
        pressure: 0.8,
        tilt_x_deg: 0.0,
        tilt_y_deg: 0.0,
        time_ms,
    }
}

fn stroke_on(gpu: &GpuContext, s: &mut PaintSession, surface: Surface, y: f64) {
    s.begin_stroke_on(gpu, surface, sample(40.0, y, 0));
    for i in 1..=40 {
        s.add_samples(&[sample(40.0 + 10.0 * f64::from(i), y, 8 * i)]);
        if i % 4 == 0 {
            frame(gpu, s);
        }
    }
    s.end_stroke();
    frame(gpu, s);
}

fn load_slot(gpu: &GpuContext, s: &mut PaintSession, slot: usize) {
    s.brush.recipe = vec![(slot, 1.0)];
    s.reload_brush(gpu);
}

#[test]
fn a_new_session_has_a_clean_tray_with_the_painting_pigments() {
    let gpu = GpuContext::new().unwrap();
    let s = session(&gpu);
    assert_eq!(s.tray.spec, tray_spec(&s.canvas.spec.pigment_slots));
    assert_eq!(
        (s.tray.spec.width, s.tray.spec.height),
        (TRAY_WIDTH, TRAY_HEIGHT)
    );
    assert_eq!(s.tray.spec.weave, Weave::Smooth);
    assert_eq!(s.tray.wet_totals(&gpu).unwrap().volume, 0.0);
}

#[test]
fn the_tray_primer_is_a_flat_neutral_grey() {
    let db = PigmentDb::builtin();
    let bands = primer_reflectance(&db, Primer::Tray);
    assert!(bands.iter().all(|band| (*band - 0.75).abs() < 1e-6));
}

#[test]
fn a_pigment_added_to_the_painting_reaches_the_tray() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let slot = s.add_slot(&gpu, "titanium_white").unwrap();
    assert_eq!(slot, 2);
    assert_eq!(s.tray.spec.pigment_slots, s.canvas.spec.pigment_slots);
}

#[test]
fn surfaces_are_reached_by_name() {
    let gpu = GpuContext::new().unwrap();
    let s = session(&gpu);
    assert_eq!(s.canvas_of(Surface::Painting).spec, s.canvas.spec);
    assert_eq!(s.canvas_of(Surface::Tray).spec, s.tray.spec);
}

#[test]
fn replacing_the_document_starts_a_clean_tray_with_the_new_pigments() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let texels = (TRAY_WIDTH * TRAY_HEIGHT) as usize;
    s.tray
        .write_f32(&gpu, StateTexture::WetVol, &vec![20.0; texels]);
    assert!(s.tray.wet_totals(&gpu).unwrap().volume > 0.0);
    let mut other = spec();
    other.pigment_slots = vec!["burnt_sienna".into()];
    s.replace_document(&gpu, other.clone()).unwrap();
    assert_eq!(s.tray.spec, tray_spec(&other.pigment_slots));
    assert_eq!(s.tray.wet_totals(&gpu).unwrap().volume, 0.0);
}

#[test]
fn the_tray_always_carries_the_painting_pigments() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.add_slot(&gpu, "titanium_white").unwrap();
    assert_eq!(s.tray.spec.pigment_slots, s.canvas.spec.pigment_slots);
    let mut other = spec();
    other.pigment_slots = vec!["burnt_sienna".into(), "cadmium_red".into()];
    s.replace_document(&gpu, other).unwrap();
    assert_eq!(s.tray.spec.pigment_slots, s.canvas.spec.pigment_slots);
    s.add_slot(&gpu, "titanium_white").unwrap();
    assert_eq!(s.tray.spec.pigment_slots, s.canvas.spec.pigment_slots);
    assert!(s.add_slot(&gpu, "unobtainium").is_err());
    assert_eq!(s.tray.spec.pigment_slots, s.canvas.spec.pigment_slots);
}

#[test]
fn a_stroke_on_the_tray_leaves_the_painting_untouched() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load_slot(&gpu, &mut s, 0);
    stroke_on(&gpu, &mut s, Surface::Tray, 100.0);
    assert!(s.tray.wet_totals(&gpu).unwrap().volume > 0.0);
    assert_eq!(s.canvas.wet_totals(&gpu).unwrap().volume, 0.0);
}

#[test]
fn the_tip_dirtied_on_the_tray_carries_that_paint_to_the_painting() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load_slot(&gpu, &mut s, 1);
    stroke_on(&gpu, &mut s, Surface::Tray, 100.0);
    load_slot(&gpu, &mut s, 0);
    stroke_on(&gpu, &mut s, Surface::Tray, 100.0);
    stroke_on(&gpu, &mut s, Surface::Painting, 100.0);
    assert!(s.canvas.wet_totals(&gpu).unwrap().pigment[1] > 0.0);
}

#[test]
fn undo_follows_one_order_across_tray_and_painting() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load_slot(&gpu, &mut s, 0);
    stroke_on(&gpu, &mut s, Surface::Painting, 60.0);
    stroke_on(&gpu, &mut s, Surface::Tray, 60.0);
    assert!(s.undo(&gpu));
    assert_eq!(s.tray.wet_totals(&gpu).unwrap().volume, 0.0);
    assert!(s.canvas.wet_totals(&gpu).unwrap().volume > 0.0);
    assert!(s.undo(&gpu));
    assert_eq!(s.canvas.wet_totals(&gpu).unwrap().volume, 0.0);
    assert_eq!(s.tray.wet_totals(&gpu).unwrap().volume, 0.0);
    assert!(s.redo(&gpu));
    assert!(s.redo(&gpu));
    assert!(s.canvas.wet_totals(&gpu).unwrap().volume > 0.0);
    assert!(s.tray.wet_totals(&gpu).unwrap().volume > 0.0);
}

#[test]
fn drying_leaves_the_tray_wet() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load_slot(&gpu, &mut s, 0);
    stroke_on(&gpu, &mut s, Surface::Tray, 60.0);
    stroke_on(&gpu, &mut s, Surface::Painting, 60.0);
    let before = s.tray.wet_totals(&gpu).unwrap().volume;
    s.dry(&gpu);
    assert_eq!(s.tray.wet_totals(&gpu).unwrap().volume, before);
}

#[test]
fn the_eyedropper_reads_the_tray() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load_slot(&gpu, &mut s, 1);
    stroke_on(&gpu, &mut s, Surface::Tray, 100.0);
    let picked: Mixture = s
        .pick_colour_on(&gpu, Surface::Tray, 200.0, 100.0)
        .unwrap()
        .unwrap();
    assert!(picked.fractions[1] > picked.fractions[0]);
    assert!(
        s.pick_colour_on(&gpu, Surface::Painting, 200.0, 100.0)
            .unwrap()
            .is_none()
    );
}

#[test]
fn clearing_the_tray_can_be_undone() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load_slot(&gpu, &mut s, 0);
    stroke_on(&gpu, &mut s, Surface::Tray, 60.0);
    let painted = s.tray.wet_totals(&gpu).unwrap().volume;
    assert_eq!(s.clear_tray(&gpu), ClearOutcome::Cleared { undoable: true });
    assert_eq!(s.tray.wet_totals(&gpu).unwrap().volume, 0.0);
    assert!(s.undo(&gpu));
    assert_eq!(s.tray.wet_totals(&gpu).unwrap().volume, painted);
}

#[test]
fn redoing_a_clear_leaves_nothing_to_clear() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load_slot(&gpu, &mut s, 0);
    stroke_on(&gpu, &mut s, Surface::Tray, 60.0);
    s.clear_tray(&gpu);
    assert!(s.undo(&gpu));
    assert!(s.redo(&gpu));
    assert_eq!(s.tray.wet_totals(&gpu).unwrap().volume, 0.0);
    assert_eq!(s.clear_tray(&gpu), ClearOutcome::NothingWet);
}

#[test]
fn queued_dabs_land_on_the_surface_they_were_painted_on() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load_slot(&gpu, &mut s, 1);
    s.begin_stroke_on(&gpu, Surface::Tray, sample(40.0, 100.0, 0));
    for i in 1..=40 {
        s.add_samples(&[sample(40.0 + 10.0 * f64::from(i), 100.0, 8 * i)]);
    }
    stroke_on(&gpu, &mut s, Surface::Painting, 200.0);
    assert!(
        s.pick_colour_on(&gpu, Surface::Tray, 200.0, 100.0)
            .unwrap()
            .is_some()
    );
    assert!(
        s.pick_colour_on(&gpu, Surface::Painting, 200.0, 100.0)
            .unwrap()
            .is_none()
    );
    assert!(
        s.pick_colour_on(&gpu, Surface::Painting, 200.0, 200.0)
            .unwrap()
            .is_some()
    );
}

fn still_nozzle(slot: usize, seconds: f32) -> Nozzle {
    Nozzle {
        from: [300.0, 300.0],
        to: [300.0, 300.0],
        radius: 12.0,
        slot,
        pressure: 1.0,
        seconds,
    }
}

#[test]
fn the_tube_squeezes_pure_pigment_without_touching_the_brush() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.clean_brush(&gpu);
    s.begin_hold(&gpu, Surface::Tray);
    s.squeeze(&gpu, still_nozzle(1, 0.2));
    s.end_hold();
    let totals = s.tray.wet_totals(&gpu).unwrap();
    assert!(totals.volume > 0.0);
    assert!((totals.pigment[1] - totals.volume).abs() < 1e-3 * totals.volume);
    assert_eq!(totals.pigment[0], 0.0);
    assert_eq!(s.engine.brush_totals(&gpu).unwrap().volume, 0.0);
}

#[test]
fn a_still_tube_grows_its_blob() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.begin_hold(&gpu, Surface::Tray);
    s.squeeze(&gpu, still_nozzle(0, 0.1));
    let once = s.tray.wet_totals(&gpu).unwrap().volume;
    s.squeeze(&gpu, still_nozzle(0, 0.1));
    s.end_hold();
    assert!(s.tray.wet_totals(&gpu).unwrap().volume > 1.5 * once);
}

#[test]
fn a_slow_tube_lays_a_thicker_cord_than_a_fast_one() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let cord = |length: f32, y: f32| Nozzle {
        from: [100.0, y],
        to: [100.0 + length, y],
        radius: 8.0,
        slot: 0,
        pressure: 1.0,
        seconds: 0.1,
    };
    s.begin_hold(&gpu, Surface::Tray);
    s.squeeze(&gpu, cord(40.0, 200.0));
    s.squeeze(&gpu, cord(400.0, 600.0));
    s.end_hold();
    let volume = s.tray.read_f32(&gpu, StateTexture::WetVol).unwrap();
    let thickness = |x: usize, y: usize| volume[y * TRAY_WIDTH as usize + x];
    assert!(thickness(120, 200) > 2.0 * thickness(120, 600));
}

#[test]
fn squeezing_on_the_painting_is_one_undo_step() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.begin_hold(&gpu, Surface::Painting);
    s.squeeze(
        &gpu,
        Nozzle {
            from: [100.0, 100.0],
            to: [140.0, 100.0],
            radius: 6.0,
            slot: 0,
            pressure: 0.5,
            seconds: 0.05,
        },
    );
    s.squeeze(
        &gpu,
        Nozzle {
            from: [140.0, 100.0],
            to: [180.0, 100.0],
            radius: 6.0,
            slot: 0,
            pressure: 0.5,
            seconds: 0.05,
        },
    );
    s.end_hold();
    assert!(s.canvas.wet_totals(&gpu).unwrap().volume > 0.0);
    assert!(s.undo(&gpu));
    assert_eq!(s.canvas.wet_totals(&gpu).unwrap().volume, 0.0);
    assert!(!s.history.can_undo());
}

#[test]
fn squeezing_without_a_hold_does_nothing() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.squeeze(&gpu, still_nozzle(0, 0.2));
    assert_eq!(s.tray.wet_totals(&gpu).unwrap().volume, 0.0);
}

#[test]
fn undoing_while_squeezing_ends_the_hold_and_takes_the_squeeze_back() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke_on(&gpu, &mut s, Surface::Tray, 100.0);
    let painted = s.tray.wet_totals(&gpu).unwrap().volume;
    s.begin_hold(&gpu, Surface::Tray);
    assert!(s.is_holding());
    s.squeeze(&gpu, still_nozzle(0, 0.2));
    assert!(s.undo(&gpu));
    assert!(!s.is_holding());
    assert!((s.tray.wet_totals(&gpu).unwrap().volume - painted).abs() < 1e-6 * painted);
}

#[test]
fn squeezing_onto_wet_paint_keeps_the_fractions_whole() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.begin_hold(&gpu, Surface::Tray);
    s.squeeze(&gpu, still_nozzle(1, 7.0 / 30.0));
    s.squeeze(&gpu, still_nozzle(0, 0.1));
    s.end_hold();
    let low = s.tray.read_f32(&gpu, StateTexture::WetPig0).unwrap();
    let high = s.tray.read_f32(&gpu, StateTexture::WetPig1).unwrap();
    let worst = low
        .chunks(4)
        .zip(high.chunks(4))
        .map(|(low, high)| low.iter().chain(high).sum::<f32>())
        .fold(0.0, f32::max);
    assert!(worst <= 1.0, "fractions sum to {worst}");
}

#[test]
fn a_tube_of_a_missing_slot_squeezes_nothing() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.begin_hold(&gpu, Surface::Tray);
    s.squeeze(&gpu, still_nozzle(2, 0.2));
    s.end_hold();
    assert_eq!(s.tray.wet_totals(&gpu).unwrap().volume, 0.0);
    assert!(!s.history.can_undo());
}

#[test]
fn a_motionless_instant_squeezes_no_undo_step() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.begin_hold(&gpu, Surface::Tray);
    s.squeeze(&gpu, still_nozzle(0, 0.0));
    s.end_hold();
    assert!(!s.history.can_undo());
}

#[test]
fn a_long_still_squeeze_stops_at_the_tube_height() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.begin_hold(&gpu, Surface::Tray);
    s.squeeze(&gpu, still_nozzle(0, 10.0));
    s.squeeze(&gpu, still_nozzle(0, 10.0));
    s.end_hold();
    let tallest = s
        .tray
        .read_f32(&gpu, StateTexture::WetVol)
        .unwrap()
        .into_iter()
        .fold(0.0, f32::max);
    assert_eq!(tallest, s.engine.params().get("tube_max_um"));
}

#[test]
fn a_stroke_during_a_hold_leaves_the_squeeze_its_own_undo_step() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.begin_hold(&gpu, Surface::Painting);
    s.squeeze(
        &gpu,
        Nozzle {
            from: [300.0, 200.0],
            to: [300.0, 200.0],
            radius: 12.0,
            slot: 0,
            pressure: 1.0,
            seconds: 0.2,
        },
    );
    let squeezed = s.canvas.wet_totals(&gpu).unwrap().volume;
    stroke_on(&gpu, &mut s, Surface::Painting, 60.0);
    assert!(!s.is_holding());
    assert!(s.canvas.wet_totals(&gpu).unwrap().volume > squeezed);
    assert!(s.undo(&gpu));
    assert!((s.canvas.wet_totals(&gpu).unwrap().volume - squeezed).abs() < 1e-6 * squeezed);
    assert!(s.undo(&gpu));
    assert_eq!(s.canvas.wet_totals(&gpu).unwrap().volume, 0.0);
}

fn blob(gpu: &GpuContext, s: &mut PaintSession, slot: usize) {
    s.begin_hold(gpu, Surface::Tray);
    for _ in 0..5 {
        s.squeeze(gpu, still_nozzle(slot, 0.2));
    }
    s.end_hold();
}

fn touch(pressure: f32, seconds: f32) -> DrawUpRequest {
    DrawUpRequest {
        center: [300.0, 300.0],
        radius: 12.0,
        pressure,
        seconds,
    }
}

#[test]
fn drawing_up_moves_paint_from_the_tray_into_the_reservoir() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    blob(&gpu, &mut s, 1);
    s.clean_brush(&gpu);
    let tray_before = s.tray.wet_totals(&gpu).unwrap().volume;
    s.begin_hold(&gpu, Surface::Tray);
    let taken = s.draw_up(&gpu, touch(1.0, 0.1)).unwrap();
    s.end_hold();
    let tray_after = s.tray.wet_totals(&gpu).unwrap().volume;
    let reserve = s.engine.reserve_totals(&gpu).unwrap();
    assert!(taken.volume > 0.0);
    assert!(((tray_before - tray_after) - taken.volume).abs() < 1e-2 * taken.volume);
    assert!((reserve.volume - taken.volume).abs() < 1e-2 * taken.volume);
    assert!((reserve.pigment[1] - reserve.volume).abs() < 1e-2 * reserve.volume);
}

#[test]
fn harder_touches_draw_up_faster() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    blob(&gpu, &mut s, 0);
    s.clean_brush(&gpu);
    s.begin_hold(&gpu, Surface::Tray);
    let light = s.draw_up(&gpu, touch(0.2, 0.05)).unwrap().volume;
    let hard = s.draw_up(&gpu, touch(1.0, 0.05)).unwrap().volume;
    s.end_hold();
    assert!(hard > 2.0 * light);
}

#[test]
fn drawing_up_stops_when_the_reservoir_is_full() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let wide = |seconds| Nozzle {
        from: [300.0, 300.0],
        to: [300.0, 300.0],
        radius: 60.0,
        slot: 0,
        pressure: 1.0,
        seconds,
    };
    s.begin_hold(&gpu, Surface::Tray);
    for _ in 0..20 {
        s.squeeze(&gpu, wide(0.5));
    }
    s.end_hold();
    s.clean_brush(&gpu);
    let wide_touch = DrawUpRequest {
        center: [300.0, 300.0],
        radius: 60.0,
        pressure: 1.0,
        seconds: 0.1,
    };
    s.begin_hold(&gpu, Surface::Tray);
    for _ in 0..200 {
        let _ = s.draw_up(&gpu, wide_touch);
    }
    let last = s.draw_up(&gpu, wide_touch).unwrap();
    s.end_hold();
    let reserve = s.engine.reserve_totals(&gpu).unwrap().volume as f32;
    assert!(reserve <= s.engine.full_reserve_volume() * 1.001);
    assert!(reserve >= s.engine.full_reserve_volume() * 0.99);
    assert!(last.volume < 1e-3 * f64::from(s.engine.full_reserve_volume()));
}

#[test]
fn drawing_up_from_a_clean_tray_takes_nothing() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.clean_brush(&gpu);
    s.begin_hold(&gpu, Surface::Tray);
    let taken = s.draw_up(&gpu, touch(1.0, 0.1)).unwrap();
    s.end_hold();
    assert_eq!(taken.volume, 0.0);
    assert!(!s.history.can_undo());
}

#[test]
fn drawing_up_beside_the_paint_leaves_no_undo_step() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    blob(&gpu, &mut s, 0);
    s.clean_brush(&gpu);
    s.begin_hold(&gpu, Surface::Tray);
    let mut beside = touch(1.0, 0.1);
    beside.center = [326.0, 300.0];
    let taken = s.draw_up(&gpu, beside).unwrap();
    s.end_hold();
    assert_eq!(taken.volume, 0.0);
    assert!(s.undo(&gpu));
    assert!(!s.history.can_undo());
}

#[test]
fn drawing_up_without_a_hold_takes_nothing() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    blob(&gpu, &mut s, 0);
    s.clean_brush(&gpu);
    let taken = s.draw_up(&gpu, touch(1.0, 0.1)).unwrap();
    assert_eq!(taken.volume, 0.0);
    assert_eq!(s.engine.reserve_totals(&gpu).unwrap().volume, 0.0);
}

#[test]
fn undoing_a_draw_up_puts_the_paint_back_on_the_tray() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    blob(&gpu, &mut s, 0);
    let squeezed = s.tray.wet_totals(&gpu).unwrap().volume;
    s.begin_hold(&gpu, Surface::Tray);
    s.draw_up(&gpu, touch(1.0, 0.1)).unwrap();
    s.end_hold();
    assert!(s.tray.wet_totals(&gpu).unwrap().volume < squeezed);
    assert!(s.undo(&gpu));
    assert_eq!(s.tray.wet_totals(&gpu).unwrap().volume, squeezed);
}

#[test]
fn the_knife_draws_up_into_its_blade() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    blob(&gpu, &mut s, 1);
    s.brush.tool = Tool::Knife;
    s.clean_brush(&gpu);
    s.begin_hold(&gpu, Surface::Tray);
    let taken = s.draw_up(&gpu, touch(1.0, 0.1)).unwrap();
    s.end_hold();
    let knife = s.engine.knife_totals(&gpu).unwrap();
    assert!(taken.volume > 0.0);
    assert!((knife.volume - taken.volume).abs() < 1e-2 * taken.volume);
}

#[test]
fn the_blender_and_the_rag_draw_up_nothing() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    blob(&gpu, &mut s, 0);
    let squeezed = s.tray.wet_totals(&gpu).unwrap().volume;
    for tool in [Tool::Blender, Tool::Rag] {
        s.brush.tool = tool;
        s.begin_hold(&gpu, Surface::Tray);
        let taken = s.draw_up(&gpu, touch(1.0, 0.1)).unwrap();
        s.end_hold();
        assert_eq!(taken.volume, 0.0);
    }
    assert_eq!(s.tray.wet_totals(&gpu).unwrap().volume, squeezed);
}
