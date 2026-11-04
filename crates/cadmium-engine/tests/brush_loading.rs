use cadmium_engine::{
    brush::FULL_RESERVOIR_VOLUME_UM,
    canvas::*,
    engine::{MAX_TIP_THICKNESS_UM, PaintEngine, TIP_SIZE},
    gpu::GpuContext,
    pigments::{MAX_SLOTS, Mixture},
    session::PaintSession,
    stroke::StrokeSample,
};

const BLUE: usize = 0;
const YELLOW: usize = 1;
const SMALL_TIP_RADIUS_PX: f32 = 5.0;
const DEFAULT_TIP_RADIUS_PX: f32 = 20.0;
const RELATIVE_TOLERANCE: f64 = 1e-4;
const FRACTION_TOLERANCE: f32 = 1e-3;

fn without_wgpu_errors<T>(gpu: &GpuContext, work: impl FnOnce() -> T) -> T {
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let result = work();
    let error = pollster::block_on(scope.pop());
    assert!(error.is_none(), "wgpu error: {error:?}");
    result
}

fn session(gpu: &GpuContext) -> PaintSession {
    without_wgpu_errors(gpu, || {
        PaintSession::new(
            gpu,
            CanvasSpec {
                width: 512,
                height: 256,
                primer: Primer::WhiteGesso,
                weave: Weave::Linen,
                pigment_slots: vec![
                    "ultramarine_blue".into(),
                    "cadmium_yellow".into(),
                    "titanium_white".into(),
                ],
            },
        )
    })
}

fn frame(gpu: &GpuContext, s: &mut PaintSession) {
    without_wgpu_errors(gpu, || {
        let mut e = gpu.device.create_command_encoder(&Default::default());
        s.encode_frame(gpu, &mut e);
        gpu.queue.submit([e.finish()]);
    });
}

fn stroke(gpu: &GpuContext, s: &mut PaintSession, y: f64, from: f64, to: f64) {
    let steps = 60;
    for i in 0..=steps {
        let sample = StrokeSample {
            x: from + (to - from) * f64::from(i) / f64::from(steps),
            y,
            pressure: 0.8,
            tilt_x_deg: 20.0,
            tilt_y_deg: 0.0,
            time_ms: i * 4,
        };
        if i == 0 {
            s.begin_stroke(gpu, sample)
        } else {
            s.add_samples(&[sample])
        }
        if i % 4 == 0 {
            frame(gpu, s);
        }
    }
    s.end_stroke(gpu);
    frame(gpu, s);
}

fn pure(slot: usize, volume_um: f32) -> Mixture {
    let mut fractions = [0.0; MAX_SLOTS];
    fractions[slot] = 1.0;
    Mixture {
        volume_um,
        fractions,
    }
}

fn disk_texels(radius: f32) -> f64 {
    let half = (TIP_SIZE / 2) as i32;
    let mut count = 0u32;
    for y in -half..half {
        for x in -half..half {
            let (x, y) = (x as f32, y as f32);
            if x * x + y * y <= radius * radius {
                count += 1;
            }
        }
    }
    f64::from(count)
}

fn reserve_factor(s: &PaintSession) -> f64 {
    f64::from(s.engine.params().get("reserve_factor"))
}

fn full_load_volume(s: &PaintSession, thickness_um: f32, radius: f32) -> f64 {
    f64::from(thickness_um) * disk_texels(radius) * (1.0 + reserve_factor(s))
}

fn tip_totals(gpu: &GpuContext, s: &PaintSession) -> PaintTotals {
    let brush = s.engine.brush_totals(gpu).unwrap();
    let reserve = s.engine.reserve_totals(gpu).unwrap();
    PaintTotals {
        volume: brush.volume - reserve.volume,
        pigment: std::array::from_fn(|slot| brush.pigment[slot] - reserve.pigment[slot]),
    }
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= RELATIVE_TOLERANCE * expected.abs().max(1.0),
        "{actual} vs {expected}"
    );
}

fn clean(gpu: &GpuContext, s: &mut PaintSession) {
    without_wgpu_errors(gpu, || s.clean_brush(gpu));
}

fn dip(gpu: &GpuContext, s: &mut PaintSession, mixture: &Mixture, amount: f32) {
    without_wgpu_errors(gpu, || s.dip(gpu, mixture, amount).unwrap());
}

fn tip_mean(gpu: &GpuContext, s: &mut PaintSession) -> Mixture {
    without_wgpu_errors(gpu, || s.tip_mean_mixture(gpu).unwrap())
}

#[test]
fn reserve_is_a_lab_parameter_defaulting_to_three_tip_loads() {
    let gpu = GpuContext::new().unwrap();
    let s = session(&gpu);
    assert_eq!(reserve_factor(&s), 3.0);
}

#[test]
fn dip_adds_its_share_of_a_full_load_to_tip_and_reserve() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    clean(&gpu, &mut s);
    let blue = pure(BLUE, 100.0);
    dip(&gpu, &mut s, &blue, 0.5);
    let once = s.engine.brush_totals(&gpu).unwrap();
    assert_close(
        once.volume,
        0.5 * full_load_volume(&s, 100.0, DEFAULT_TIP_RADIUS_PX),
    );
    assert_close(once.pigment[BLUE], once.volume);
    dip(&gpu, &mut s, &blue, 0.5);
    let twice = s.engine.brush_totals(&gpu).unwrap();
    assert_close(twice.volume, 2.0 * once.volume);
    assert_close(
        s.engine.reserve_totals(&gpu).unwrap().volume,
        reserve_factor(&s) * 100.0 * disk_texels(DEFAULT_TIP_RADIUS_PX),
    );
}

#[test]
fn dipping_a_second_colour_mixes_with_what_the_brush_holds_by_volume() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    clean(&gpu, &mut s);
    dip(&gpu, &mut s, &pure(BLUE, 120.0), 1.0);
    dip(&gpu, &mut s, &pure(YELLOW, 80.0), 0.5);
    let tip = tip_mean(&gpu, &mut s);
    assert!((tip.volume_um - 160.0).abs() < 1e-2, "{}", tip.volume_um);
    assert!((tip.fractions[BLUE] - 0.75).abs() < FRACTION_TOLERANCE);
    assert!((tip.fractions[YELLOW] - 0.25).abs() < FRACTION_TOLERANCE);
    let reserve = s.engine.reserve_totals(&gpu).unwrap();
    assert_close(reserve.pigment[BLUE] / reserve.volume, 0.75);
    assert_close(reserve.pigment[YELLOW] / reserve.volume, 0.25);
}

#[test]
fn dipping_never_piles_the_tip_above_its_maximum_thickness() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    clean(&gpu, &mut s);
    let blue = pure(BLUE, FULL_RESERVOIR_VOLUME_UM);
    dip(&gpu, &mut s, &blue, 1.0);
    dip(&gpu, &mut s, &blue, 1.0);
    dip(&gpu, &mut s, &blue, 1.0);
    let tip = tip_mean(&gpu, &mut s);
    assert!(
        tip.volume_um <= MAX_TIP_THICKNESS_UM + 1e-3,
        "{}",
        tip.volume_um
    );
    assert_close(
        s.engine.brush_totals(&gpu).unwrap().volume,
        f64::from(MAX_TIP_THICKNESS_UM)
            * disk_texels(DEFAULT_TIP_RADIUS_PX)
            * (1.0 + reserve_factor(&s)),
    );
}

#[test]
fn a_dip_of_nothing_leaves_the_brush_untouched() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let before = s.engine.brush_totals(&gpu).unwrap();
    dip(&gpu, &mut s, &pure(YELLOW, 100.0), 0.0);
    dip(&gpu, &mut s, &pure(YELLOW, 100.0), f32::NAN);
    dip(&gpu, &mut s, &pure(YELLOW, 0.0), 1.0);
    assert_eq!(s.engine.brush_totals(&gpu).unwrap(), before);
}

#[test]
fn reload_replaces_a_contaminated_brush_with_the_recipe() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.brush.recipe = vec![(YELLOW, 1.0)];
    s.reload_brush(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    s.brush.recipe = vec![(BLUE, 1.0)];
    s.reload_brush(&gpu);
    stroke(&gpu, &mut s, 128.0, 470.0, 40.0);
    assert!(tip_mean(&gpu, &mut s).fractions[YELLOW] > 0.0);
    s.reload_brush(&gpu);
    let tip = tip_mean(&gpu, &mut s);
    let recipe = s.brush.reservoir_mixture(3);
    assert_eq!(tip.fractions[YELLOW], 0.0);
    assert!((tip.fractions[BLUE] - recipe.fractions[BLUE]).abs() < FRACTION_TOLERANCE);
    assert!((tip.volume_um - recipe.volume_um).abs() < 1e-2);
    assert_close(
        s.engine.brush_totals(&gpu).unwrap().volume,
        full_load_volume(&s, recipe.volume_um, DEFAULT_TIP_RADIUS_PX),
    );
}

#[test]
fn reloading_with_an_empty_recipe_cleans_the_brush_instead_of_loading_bare_medium() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.brush.recipe.clear();
    s.reload_brush(&gpu);
    assert_eq!(s.engine.brush_totals(&gpu).unwrap().volume, 0.0);
}

#[test]
fn shrinking_the_tip_keeps_the_paint_outside_the_new_radius() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let before = s.engine.brush_totals(&gpu).unwrap();
    s.brush.geometry.max_diameter_px = 2.0 * SMALL_TIP_RADIUS_PX;
    s.resize_tip();
    assert_eq!(s.engine.brush_totals(&gpu).unwrap(), before);
    s.brush.geometry.max_diameter_px = 2.0 * DEFAULT_TIP_RADIUS_PX;
    s.resize_tip();
    let tip = tip_mean(&gpu, &mut s);
    assert!((tip.volume_um - s.brush.reservoir_mixture(3).volume_um).abs() < 1e-2);
}

#[test]
fn growing_the_tip_does_not_load_the_newly_exposed_bristles() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.brush.geometry.max_diameter_px = 2.0 * SMALL_TIP_RADIUS_PX;
    s.reload_brush(&gpu);
    let before = s.engine.brush_totals(&gpu).unwrap();
    s.brush.geometry.max_diameter_px = 2.0 * DEFAULT_TIP_RADIUS_PX;
    s.resize_tip();
    assert_eq!(s.engine.brush_totals(&gpu).unwrap(), before);
    let loaded = s.brush.reservoir_mixture(3).volume_um;
    let expected_mean =
        loaded * (disk_texels(SMALL_TIP_RADIUS_PX) / disk_texels(DEFAULT_TIP_RADIUS_PX)) as f32;
    assert!((tip_mean(&gpu, &mut s).volume_um - expected_mean).abs() < 1e-2);
}

#[test]
fn restoring_the_reserve_refills_it_to_the_load_and_leaves_the_dirty_tip_alone() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 64.0, 40.0, 470.0);
    s.brush.recipe = vec![(YELLOW, 1.0)];
    s.reload_brush(&gpu);
    stroke(&gpu, &mut s, 64.0, 470.0, 40.0);
    let tip_before = tip_totals(&gpu, &s);
    let reserve_before = s.engine.reserve_totals(&gpu).unwrap();
    let recipe = s.brush.reservoir_mixture(3);
    let full_reserve =
        reserve_factor(&s) * f64::from(recipe.volume_um) * disk_texels(DEFAULT_TIP_RADIUS_PX);
    assert!(reserve_before.volume < full_reserve);
    without_wgpu_errors(&gpu, || s.restore_reserve(&gpu));
    let tip_after = tip_totals(&gpu, &s);
    assert_close(tip_after.volume, tip_before.volume);
    assert_close(tip_after.pigment[BLUE], tip_before.pigment[BLUE]);
    let reserve = s.engine.reserve_totals(&gpu).unwrap();
    assert_close(reserve.volume, full_reserve);
    assert_close(
        reserve.pigment[YELLOW] - reserve_before.pigment[YELLOW],
        f64::from(recipe.fractions[YELLOW]) * (full_reserve - reserve_before.volume),
    );
}

#[test]
fn restoring_a_full_reserve_adds_nothing() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let before = s.engine.brush_totals(&gpu).unwrap();
    without_wgpu_errors(&gpu, || s.restore_reserve(&gpu));
    assert_eq!(s.engine.brush_totals(&gpu).unwrap(), before);
}

#[test]
fn restoring_the_reserve_with_an_empty_recipe_adds_nothing() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 64.0, 40.0, 470.0);
    let before = s.engine.brush_totals(&gpu).unwrap();
    s.brush.recipe.clear();
    without_wgpu_errors(&gpu, || s.restore_reserve(&gpu));
    assert_eq!(s.engine.brush_totals(&gpu).unwrap(), before);
}

#[test]
fn the_tip_mean_of_a_clean_brush_is_empty() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    clean(&gpu, &mut s);
    let tip = tip_mean(&gpu, &mut s);
    assert_eq!(tip.volume_um, 0.0);
    assert_eq!(tip.fractions, [0.0; MAX_SLOTS]);
}

#[test]
fn the_tip_mean_shows_wet_paint_picked_up_from_the_canvas() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.brush.recipe = vec![(YELLOW, 1.0)];
    s.reload_brush(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    s.brush.recipe = vec![(BLUE, 1.0)];
    s.reload_brush(&gpu);
    stroke(&gpu, &mut s, 128.0, 470.0, 40.0);
    let tip = tip_mean(&gpu, &mut s);
    assert!(tip.fractions[YELLOW] > 0.01, "{:?}", tip.fractions);
    assert_eq!(
        s.engine.reservoir_mixture(&gpu).unwrap().fractions[YELLOW],
        0.0
    );
}

#[test]
fn dipping_conserves_everything_already_on_canvas_and_brush_plus_the_dip() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 64.0, 40.0, 470.0);
    let total = |s: &PaintSession| {
        s.canvas.wet_totals(&gpu).unwrap().volume + s.engine.brush_totals(&gpu).unwrap().volume
    };
    let before = total(&s);
    let yellow = pure(YELLOW, 40.0);
    dip(&gpu, &mut s, &yellow, 0.25);
    let dipped = 0.25 * full_load_volume(&s, 40.0, DEFAULT_TIP_RADIUS_PX);
    assert_close(total(&s), before + dipped);
    stroke(&gpu, &mut s, 128.0, 470.0, 40.0);
    assert_close(total(&s), before + dipped);
}

#[test]
fn one_brush_readout_matches_reading_tip_reserve_and_knife_apart() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let readout = without_wgpu_errors(&gpu, || s.read_brush(&gpu).unwrap());
    assert_eq!(readout.tip_mean, s.tip_mean_mixture(&gpu).unwrap());
    assert_eq!(readout.reserve, s.engine.reserve_totals(&gpu).unwrap());
    assert_eq!(readout.knife, s.engine.knife_totals(&gpu).unwrap());
    assert!(readout.reserve.volume > 0.0);
}
