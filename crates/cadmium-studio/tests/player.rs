use cadmium_engine::brush::{BrushSpec, factory_brushes};
use cadmium_engine::canvas::{Primer, Weave};
use cadmium_engine::engine::BristleSnapshot;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::params::SimParams;
use cadmium_studio::{DemoStroke, Phase, PlayerRequest, SLOW_MOTION, StudioPaint, StudioPlayer};

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

fn player(gpu: &GpuContext, stroke: DemoStroke) -> StudioPlayer {
    let (brush, paint) = (brush(), paint());
    let request = PlayerRequest {
        brush: &brush,
        paint: &paint,
        stroke,
    };
    StudioPlayer::new(gpu, request).unwrap()
}

/// Advances by `steps`, round and round, until the loop reaches its pause.
fn to_pause(gpu: &GpuContext, player: &mut StudioPlayer, steps: &[f64]) {
    for dt in steps.iter().cycle() {
        if player.phase() == Phase::Pause {
            return;
        }
        player.advance(gpu, *dt).unwrap();
    }
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
    to_pause(&gpu, &mut slow, &[24.0]);
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
    let first = player.rgba(&gpu).unwrap();
    assert!(inked(&first), "nothing painted");
    while player.phase() == Phase::Pause {
        player.advance(&gpu, 18.0).unwrap();
    }
    to_pause(&gpu, &mut player, &[18.0]);
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
