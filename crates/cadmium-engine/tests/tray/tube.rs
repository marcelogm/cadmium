use cadmium_engine::{
    canvas::StateTexture, engine::PaintEngine, gpu::GpuContext, session::PaintSession,
    surface::Surface, tube::Nozzle,
};

use crate::{CORD_RADIUS, FRAME_SECONDS, session, still_nozzle, stroke_on, tray_heights};

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
fn the_tube_says_whether_it_squeezed() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    assert!(!s.squeeze(&gpu, still_nozzle(0, 0.1)));
    s.begin_hold(&gpu, Surface::Tray);
    let off_the_tray = Nozzle {
        from: [-500.0, -500.0],
        to: [-500.0, -500.0],
        ..still_nozzle(0, 0.1)
    };
    assert!(!s.squeeze(&gpu, off_the_tray));
    assert!(s.squeeze(&gpu, still_nozzle(0, 0.1)));
    s.end_hold();
}

#[test]
fn a_still_tube_grows_its_blob() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.begin_hold(&gpu, Surface::Tray);
    s.squeeze(&gpu, still_nozzle(0, 0.4));
    let once = tray_heights(&gpu, &s)(300, 300);
    s.squeeze(&gpu, still_nozzle(0, 0.2));
    s.end_hold();
    let cord = s.engine.params().get("tube_cord_um");
    assert!(once > cord);
    assert!(tray_heights(&gpu, &s)(300, 300) > once);
}

#[test]
fn a_frame_that_squeezes_nothing_leaves_the_blob_dwelling() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.begin_hold(&gpu, Surface::Tray);
    s.squeeze(&gpu, still_nozzle(0, 0.4));
    let once = tray_heights(&gpu, &s)(300, 300);
    let lifted = Nozzle {
        to: [600.5, 300.5],
        pressure: 0.0,
        ..still_nozzle(0, 0.05)
    };
    assert!(!s.squeeze(&gpu, lifted));
    s.squeeze(&gpu, still_nozzle(0, 0.2));
    s.end_hold();
    assert!(tray_heights(&gpu, &s)(300, 300) > once);
}

#[test]
fn a_held_tube_keeps_growing_through_a_trembling_hand() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let trembling = |frame: usize| {
        let shake = if frame.is_multiple_of(2) { 0.6 } else { -0.6 };
        [300.5 + shake, 300.5 - shake]
    };
    let held = |s: &mut PaintSession, frames: std::ops::Range<usize>| {
        for frame in frames {
            s.squeeze(
                &gpu,
                Nozzle {
                    from: trembling(frame),
                    to: trembling(frame + 1),
                    radius: CORD_RADIUS,
                    slot: 0,
                    pressure: 0.5,
                    seconds: FRAME_SECONDS,
                    still_reach_px: 0.0,
                },
            );
        }
    };
    s.begin_hold(&gpu, Surface::Tray);
    held(&mut s, 0..30);
    let early = tray_heights(&gpu, &s)(300, 300);
    held(&mut s, 30..60);
    let later = tray_heights(&gpu, &s)(300, 300);
    held(&mut s, 60..400);
    s.end_hold();
    let tallest = tray_heights(&gpu, &s)(300, 300);
    let max = s.engine.params().get("tube_max_um");
    assert!(later > early, "{early} -> {later}");
    assert!(tallest <= max && tallest > 0.95 * max, "{tallest}");
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
            still_reach_px: 0.0,
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
            still_reach_px: 0.0,
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
    let rim = tray_heights(&gpu, &s)(300, 310);
    assert!(rim < 0.75 * tallest, "{rim}");
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
            still_reach_px: 0.0,
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
