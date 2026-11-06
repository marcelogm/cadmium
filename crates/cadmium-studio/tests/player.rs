use cadmium_engine::brush::{BrushSpec, factory_brushes};
use cadmium_engine::canvas::{Primer, Weave};
use cadmium_engine::engine::BristleSnapshot;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::params::SimParams;
use cadmium_studio::{
    DemoStroke, Phase, PlayerRequest, SLOW_MOTION, StudioPaint, StudioPlayer, samples,
};

fn paint() -> StudioPaint {
    StudioPaint {
        slots: vec!["ultramarine_blue".to_string()],
        primer: Primer::WhiteGesso,
        weave: Weave::Smooth,
        recipe: vec![(0, 1.0)],
        load: 1.0,
        medium: 0.1,
        params: SimParams::default(),
    }
}

/// A small brush keeps the canvas, and so each frame, cheap.
fn brush() -> BrushSpec {
    BrushSpec {
        size_px: 12.0,
        ..factory_brushes()[0].clone()
    }
}

/// Another brush size and other slots: another canvas and another tray.
fn other_brush() -> BrushSpec {
    BrushSpec {
        size_px: 20.0,
        ..factory_brushes()[1].clone()
    }
}

fn other_paint() -> StudioPaint {
    StudioPaint {
        slots: vec!["cadmium_red".to_string(), "titanium_white".to_string()],
        recipe: vec![(0, 0.7), (1, 0.3)],
        ..paint()
    }
}

fn player(gpu: &GpuContext, stroke: DemoStroke) -> StudioPlayer {
    let (brush, paint) = (brush(), paint());
    let request = PlayerRequest {
        brush: &brush,
        paint: &paint,
        stroke,
    };
    StudioPlayer::new(gpu, request).unwrap()
}

/// More advances than any loop needs, even one real millisecond at a time.
const MAX_ADVANCES: usize = 20_000;

/// Advances by `steps`, round and round, until `done` holds; fails rather than
/// hangs if it never does.
fn advance_until(
    gpu: &GpuContext,
    player: &mut StudioPlayer,
    steps: &[f64],
    done: impl Fn(&StudioPlayer) -> bool,
) {
    for dt in steps.iter().cycle().take(MAX_ADVANCES) {
        if done(player) {
            return;
        }
        player.advance(gpu, *dt).unwrap();
    }
    panic!("the player never got there");
}

/// Advances by `steps` until the loop reaches its pause.
fn to_pause(gpu: &GpuContext, player: &mut StudioPlayer, steps: &[f64]) {
    advance_until(gpu, player, steps, |p| p.phase() == Phase::Pause);
}

fn snapshot(gpu: &GpuContext, player: &StudioPlayer) -> BristleSnapshot {
    player.session().engine.bristle_snapshot(gpu).unwrap()
}

/// Whether the canvas holds paint: some pixel differs from the primer's corner.
fn inked(rgba: &[u8]) -> bool {
    let (pixels, _) = rgba.as_chunks::<4>();
    pixels.iter().any(|p| *p != pixels[0])
}

fn assert_same(gpu: &GpuContext, a: &StudioPlayer, b: &StudioPlayer) {
    let (first, second) = (snapshot(gpu, a), snapshot(gpu, b));
    assert_eq!(first.tips_px, second.tips_px);
    assert_eq!(first.chains_px, second.chains_px);
    assert!(
        a.rgba(gpu).unwrap() == b.rgba(gpu).unwrap(),
        "canvases differ"
    );
}

#[test]
fn two_players_fed_the_same_time_paint_the_same() {
    let gpu = GpuContext::new().unwrap();
    let mut a = player(&gpu, DemoStroke::Zigzag);
    let mut b = player(&gpu, DemoStroke::Zigzag);
    for dt in [16.0, 17.0, 16.5, 40.0, 3.0].repeat(8) {
        a.advance(&gpu, dt).unwrap();
        b.advance(&gpu, dt).unwrap();
    }
    assert_same(&gpu, &a, &b);
}

#[test]
fn slow_motion_paints_what_full_speed_paints() {
    let gpu = GpuContext::new().unwrap();
    let mut full = player(&gpu, DemoStroke::SCurve);
    let mut slow = player(&gpu, DemoStroke::SCurve);
    slow.set_speed(SLOW_MOTION);
    to_pause(&gpu, &mut full, &[6.0]);
    to_pause(&gpu, &mut slow, &[16.0, 17.0, 16.5]);
    assert_same(&gpu, &full, &slow);
}

#[test]
fn uneven_steps_paint_what_even_ones_do() {
    let gpu = GpuContext::new().unwrap();
    let mut even = player(&gpu, DemoStroke::TapAndLift);
    let mut uneven = player(&gpu, DemoStroke::TapAndLift);
    to_pause(&gpu, &mut even, &[6.0]);
    to_pause(&gpu, &mut uneven, &[5.0, 13.0, 7.0, 0.5, 31.0, 2.5]);
    assert_same(&gpu, &even, &uneven);
}

#[test]
fn a_paused_player_stands_still() {
    let gpu = GpuContext::new().unwrap();
    let mut player = player(&gpu, DemoStroke::RisingLine);
    player.run_until(&gpu, 200.0).unwrap();
    let (before, canvas) = (snapshot(&gpu, &player), player.rgba(&gpu).unwrap());
    player.pause();
    assert!(!player.is_playing());
    player.advance(&gpu, 1000.0).unwrap();
    assert_eq!(snapshot(&gpu, &player), before);
    assert!(player.rgba(&gpu).unwrap() == canvas, "the canvas moved");
    player.play();
    player.advance(&gpu, 30.0).unwrap();
    assert_ne!(snapshot(&gpu, &player), before);
}

#[test]
fn every_loop_paints_the_same_stroke_again() {
    let gpu = GpuContext::new().unwrap();
    let mut player = player(&gpu, DemoStroke::TapAndLift);
    to_pause(&gpu, &mut player, &[18.0]);
    let (first, bristles) = (player.rgba(&gpu).unwrap(), snapshot(&gpu, &player));
    assert!(inked(&first), "nothing painted");
    advance_until(&gpu, &mut player, &[18.0], |p| p.phase() != Phase::Pause);
    to_pause(&gpu, &mut player, &[18.0]);
    let again = snapshot(&gpu, &player);
    assert_eq!(again.tips_px, bristles.tips_px);
    assert_eq!(again.chains_px, bristles.chains_px);
    assert!(
        player.rgba(&gpu).unwrap() == first,
        "the second loop differs"
    );
}

#[test]
fn the_brush_shows_from_the_loops_first_frame() {
    let gpu = GpuContext::new().unwrap();
    let mut player = player(&gpu, DemoStroke::SCurve);
    assert!(!player.brush_visible());
    assert_eq!(player.phase(), Phase::Stroke);
    player.advance(&gpu, 5.0).unwrap();
    assert!(!player.brush_visible());
    player.advance(&gpu, 1.0).unwrap();
    assert!(player.brush_visible());
    player.run_until(&gpu, 200.0).unwrap();
    assert!(player.brush_visible());
    assert_eq!(player.phase(), Phase::Stroke);
}

#[test]
fn the_brush_hides_during_the_pause_until_the_next_loop() {
    let gpu = GpuContext::new().unwrap();
    let mut player = player(&gpu, DemoStroke::SCurve);
    to_pause(&gpu, &mut player, &[30.0]);
    assert!(!player.brush_visible());
    player.run_until(&gpu, player.loop_ms()).unwrap();
    assert_eq!(player.phase(), Phase::Pause);
    assert!(!player.brush_visible());
    player.advance(&gpu, 6.0).unwrap();
    assert_eq!(player.phase(), Phase::Stroke);
    assert!(player.brush_visible());
}

#[test]
fn the_stroke_lasts_until_its_last_sample() {
    let gpu = GpuContext::new().unwrap();
    for stroke in DemoStroke::ALL {
        let player = player(&gpu, stroke);
        let last = samples(stroke, player.stroke_box()).last().unwrap().time_ms;
        assert_eq!(player.stroke_ms(), f64::from(last), "{stroke:?}");
    }
}

#[test]
fn half_the_stroke_is_still_stroke_and_past_it_the_pen_has_lifted() {
    let gpu = GpuContext::new().unwrap();
    let mut player = player(&gpu, DemoStroke::SCurve);
    player.run_until(&gpu, player.stroke_ms() / 2.0).unwrap();
    assert_eq!(player.phase(), Phase::Stroke);
    player.run_until(&gpu, player.stroke_ms() + 60.0).unwrap();
    assert_eq!(player.phase(), Phase::Settle);
}

#[test]
fn the_pen_lifts_into_the_settle_before_the_pause() {
    let gpu = GpuContext::new().unwrap();
    let mut player = player(&gpu, DemoStroke::TapAndLift);
    let mut phases = vec![player.phase()];
    while player.phase() != Phase::Pause {
        player.advance(&gpu, 6.0).unwrap();
        if phases.last() != Some(&player.phase()) {
            phases.push(player.phase());
        }
    }
    assert_eq!(phases, [Phase::Stroke, Phase::Settle, Phase::Pause]);
}

#[test]
fn a_restart_paints_what_a_new_player_paints() {
    let gpu = GpuContext::new().unwrap();
    let mut player = player(&gpu, DemoStroke::Zigzag);
    player.set_speed(SLOW_MOTION);
    player.advance(&gpu, 100.0).unwrap();
    player.advance(&gpu, 16.0).unwrap();
    player.pause();
    let (brush, paint) = (other_brush(), other_paint());
    let request = PlayerRequest {
        brush: &brush,
        paint: &paint,
        stroke: DemoStroke::SCurve,
    };
    player.restart(&gpu, request).unwrap();
    assert!(player.is_ready() && !player.is_playing());
    assert_eq!((player.brush(), player.paint()), (&brush, &paint));
    assert_eq!(player.session().tray.spec.pigment_slots, paint.slots);
    assert_eq!(player.session().canvas.spec.pigment_slots, paint.slots);
    player.play();
    let mut fresh = StudioPlayer::new(&gpu, request).unwrap();
    fresh.set_speed(SLOW_MOTION);
    for one in [&mut player, &mut fresh] {
        // 2 ms of loop time: no quantum unless time from before the restart is left.
        one.advance(&gpu, 8.0).unwrap();
        assert!(!one.brush_visible());
        one.advance(&gpu, 16.0).unwrap();
        assert!(one.brush_visible());
        to_pause(&gpu, one, &[16.0, 17.0, 16.5]);
    }
    assert_same(&gpu, &player, &fresh);
}

#[test]
fn a_restart_on_the_same_canvas_paints_what_a_new_player_paints() {
    let gpu = GpuContext::new().unwrap();
    let mut player = player(&gpu, DemoStroke::Zigzag);
    player.run_until(&gpu, 300.0).unwrap();
    let (brush, paint) = (brush(), paint());
    let request = PlayerRequest {
        brush: &brush,
        paint: &paint,
        stroke: DemoStroke::TapAndLift,
    };
    player.restart(&gpu, request).unwrap();
    let mut fresh = StudioPlayer::new(&gpu, request).unwrap();
    for one in [&mut player, &mut fresh] {
        assert!(one.is_ready());
        to_pause(&gpu, one, &[6.0]);
    }
    assert_same(&gpu, &player, &fresh);
}

/// A textured canvas and a lighter load: where a stroke's first contact is
/// most sensitive to what the engine remembers from earlier strokes.
fn linen(load: f32) -> StudioPaint {
    StudioPaint {
        weave: Weave::Linen,
        load,
        ..paint()
    }
}

/// Plays `from` for `frames` 50 ms frames, restarts on the same canvas with `to`,
/// and checks every later frame against a fresh player shown `to`. A missing
/// or ignored first-rest flag fails on every driver; `mix(previous, found, 1.0)`
/// in place of an outright replace only on drivers that lower `mix` as
/// `a + t * (b - a)`, such as radv.
fn assert_restart_mid_stroke_matches(frames: usize, from: &StudioPaint, to: &StudioPaint) {
    let gpu = GpuContext::new().unwrap();
    let brush = BrushSpec {
        size_px: 16.0,
        ..factory_brushes()[0].clone()
    };
    let request = |paint| PlayerRequest {
        brush: &brush,
        paint,
        stroke: DemoStroke::SCurve,
    };
    let mut player = StudioPlayer::new(&gpu, request(from)).unwrap();
    for _ in 0..frames {
        player.advance(&gpu, 50.0).unwrap();
    }
    player.restart(&gpu, request(to)).unwrap();
    let mut fresh = StudioPlayer::new(&gpu, request(to)).unwrap();
    for frame in 0..40 {
        player.advance(&gpu, 50.0).unwrap();
        fresh.advance(&gpu, 50.0).unwrap();
        assert_eq!(
            snapshot(&gpu, &player),
            snapshot(&gpu, &fresh),
            "frame {frame}"
        );
        let same = player.rgba(&gpu).unwrap() == fresh.rgba(&gpu).unwrap();
        assert!(same, "frame {frame}: canvases differ");
    }
}

#[test]
fn a_restart_mid_stroke_with_less_load_paints_what_a_new_player_paints() {
    for frames in [1, 3, 10] {
        assert_restart_mid_stroke_matches(frames, &linen(1.0), &linen(0.4));
    }
}

#[test]
fn a_restart_mid_stroke_with_the_same_paint_paints_what_a_new_player_paints() {
    for frames in [1, 3, 10] {
        assert_restart_mid_stroke_matches(frames, &linen(0.4), &linen(0.4));
    }
}
