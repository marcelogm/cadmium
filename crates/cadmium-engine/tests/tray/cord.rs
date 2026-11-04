use cadmium_engine::{
    canvas::StateTexture,
    engine::PaintEngine,
    gpu::GpuContext,
    session::PaintSession,
    surface::{Surface, TRAY_WIDTH},
    tube::Nozzle,
};

use crate::{CORD_RADIUS, FRAME_SECONDS, session, tray_heights};

struct TubeDrag {
    from: [f32; 2],
    to: [f32; 2],
    frames: usize,
    pressure: f32,
}

impl TubeDrag {
    fn new(from: [f32; 2], to: [f32; 2], frames: usize, pressure: f32) -> TubeDrag {
        TubeDrag {
            from,
            to,
            frames,
            pressure,
        }
    }
}

fn drag_tube(gpu: &GpuContext, s: &mut PaintSession, drag: TubeDrag) {
    drag_tube_of(gpu, s, 0, drag);
}

fn drag_tube_of(gpu: &GpuContext, s: &mut PaintSession, slot: usize, drag: TubeDrag) {
    let TubeDrag {
        from,
        to,
        frames,
        pressure,
    } = drag;
    let at = |frame: usize| {
        let t = frame as f32 / frames as f32;
        [
            from[0] + t * (to[0] - from[0]),
            from[1] + t * (to[1] - from[1]),
        ]
    };
    s.begin_hold(gpu, Surface::Tray);
    for frame in 0..frames {
        s.squeeze(
            gpu,
            Nozzle {
                from: at(frame),
                to: at(frame + 1),
                radius: CORD_RADIUS,
                slot,
                pressure,
                seconds: FRAME_SECONDS,
                still_reach_px: 0.0,
            },
        );
    }
    s.end_hold();
}

#[test]
fn a_moving_tube_lays_an_even_cord_without_beads_at_the_frame_joints() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    drag_tube(
        &gpu,
        &mut s,
        TubeDrag::new([100.0, 200.5], [400.0, 200.5], 23, 1.0),
    );
    let height = tray_heights(&gpu, &s);
    let along: Vec<f32> = (130..370).map(|x| height(x, 200)).collect();
    let lowest = along.iter().copied().fold(f32::INFINITY, f32::min);
    let highest = along.iter().copied().fold(0.0, f32::max);
    assert!(lowest > 0.0);
    assert!(highest / lowest <= 1.001, "{lowest}..{highest}");
}

#[test]
fn a_very_slow_drag_lays_the_same_even_cord_as_a_brisk_one() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    drag_tube(
        &gpu,
        &mut s,
        TubeDrag::new([200.0, 200.5], [260.0, 200.5], 300, 1.0),
    );
    drag_tube(
        &gpu,
        &mut s,
        TubeDrag::new([200.0, 400.5], [260.0, 400.5], 4, 1.0),
    );
    let height = tray_heights(&gpu, &s);
    let brisk = height(230, 400);
    for x in 205..255 {
        assert!((height(x, 200) - brisk).abs() <= 1e-3 * brisk, "{x}");
    }
}

#[test]
fn a_fast_and_a_slow_tube_raise_the_same_cord() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    drag_tube(
        &gpu,
        &mut s,
        TubeDrag::new([100.0, 200.5], [400.0, 200.5], 60, 0.8),
    );
    drag_tube(
        &gpu,
        &mut s,
        TubeDrag::new([100.0, 400.5], [400.0, 400.5], 5, 0.8),
    );
    let height = tray_heights(&gpu, &s);
    let crest = s.engine.params().get("tube_cord_um") * 0.8;
    for y in [200, 400] {
        assert!((height(250, y) - crest).abs() < 0.01 * crest, "{y}");
    }
}

#[test]
fn a_harder_squeeze_raises_a_taller_cord() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    drag_tube(
        &gpu,
        &mut s,
        TubeDrag::new([100.0, 200.5], [400.0, 200.5], 10, 1.0),
    );
    drag_tube(
        &gpu,
        &mut s,
        TubeDrag::new([100.0, 400.5], [400.0, 400.5], 10, 0.4),
    );
    let height = tray_heights(&gpu, &s);
    assert!(height(250, 200) > 2.0 * height(250, 400));
}

#[test]
fn the_cord_is_rounded_across() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    drag_tube(
        &gpu,
        &mut s,
        TubeDrag::new([100.0, 200.5], [400.0, 200.5], 10, 1.0),
    );
    let height = tray_heights(&gpu, &s);
    assert!(height(250, 200) > height(250, 203));
    assert!(height(250, 203) > height(250, 206));
    assert!(height(250, 206) > 0.0);
    assert!((height(250, 194) - height(250, 206)).abs() < 1e-3 * height(250, 206));
}

#[test]
fn a_tube_crossing_its_own_cord_does_not_double_it() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    drag_tube(
        &gpu,
        &mut s,
        TubeDrag::new([100.0, 200.5], [400.0, 200.5], 10, 1.0),
    );
    let before = tray_heights(&gpu, &s)(250, 200);
    drag_tube(
        &gpu,
        &mut s,
        TubeDrag::new([250.5, 100.0], [250.5, 300.0], 10, 1.0),
    );
    drag_tube(
        &gpu,
        &mut s,
        TubeDrag::new([400.0, 200.5], [100.0, 200.5], 10, 1.0),
    );
    let after = tray_heights(&gpu, &s)(250, 200);
    assert!(
        (after - before).abs() < 0.01 * before,
        "{before} -> {after}"
    );
}

#[test]
fn a_tube_crossing_another_pigment_lays_its_own_colour_there() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    drag_tube_of(
        &gpu,
        &mut s,
        1,
        TubeDrag::new([100.0, 200.5], [400.0, 200.5], 10, 1.0),
    );
    drag_tube_of(
        &gpu,
        &mut s,
        0,
        TubeDrag::new([250.5, 100.0], [250.5, 300.0], 10, 0.5),
    );
    let height = tray_heights(&gpu, &s)(250, 200);
    let low = s.tray.read_f32(&gpu, StateTexture::WetPig0).unwrap();
    let ultramarine = low[(200 * TRAY_WIDTH as usize + 250) * 4] * height;
    let crest = s.engine.params().get("tube_cord_um") * 0.5;
    assert!(ultramarine > 0.98 * crest, "{ultramarine} of {crest}");
}

#[test]
fn a_cord_starts_and_ends_round() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    drag_tube(
        &gpu,
        &mut s,
        TubeDrag::new([100.5, 200.5], [400.5, 200.5], 10, 1.0),
    );
    let height = tray_heights(&gpu, &s);
    let crest = height(250, 200);
    for x in [96, 404] {
        assert!(height(x, 200) > 0.5 * crest, "{x}");
        assert!(height(x, 200) < crest, "{x}");
    }
    assert_eq!(height(91, 200), 0.0);
    assert_eq!(height(409, 200), 0.0);
}
