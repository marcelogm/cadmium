pub mod support;

use cadmium_engine::{
    canvas::*, engine::PaintEngine, gpu::GpuContext, session::PaintSession, stroke::StrokeSample,
};
use support::encode_frame;

const WIDTH: u32 = 1280;
const PEN_TILT_DEG: f32 = 25.0;
const HEIGHT: u32 = 128;
const STROKE_FROM_PX: f64 = 100.0;
const STROKE_LENGTH_PX: f64 = 1000.0;
const SAMPLES: u32 = 250;
const SAMPLES_PER_FRAME: u32 = 4;
const ROW: f64 = 64.0;
const VISIBLE_PIGMENT_UM: f32 = 3.0;
const BLUE_SLOT: usize = 1;
const PIGMENT_CHANNELS_PER_TEXTURE: usize = 4;
const TENFOLD_RESERVE: f32 = 10.0;
const LOW_LOAD: f32 = 0.2;
const FULL_LOAD: f32 = 1.0;
const DEFAULT_RESERVE_RUN_PX: usize = 440;
const LOW_LOAD_RUN_PX: usize = 460;

fn blue_along_a_long_stroke(load: f32, reserve_factor: Option<f32>) -> Vec<f32> {
    let gpu = GpuContext::new().unwrap();
    let mut session = loaded_session(&gpu, load, reserve_factor);
    paint_long_stroke(&gpu, &mut session);
    blue_along_the_row(&gpu, &session)
}

fn blue_across_a_long_stroke(load: f32) -> Vec<f32> {
    let gpu = GpuContext::new().unwrap();
    let mut session = loaded_session(&gpu, load, None);
    paint_long_stroke(&gpu, &mut session);
    blue_across_the_mark(&gpu, &session)
}

fn loaded_session(gpu: &GpuContext, load: f32, reserve_factor: Option<f32>) -> PaintSession {
    let mut session = PaintSession::new(
        gpu,
        CanvasSpec {
            width: WIDTH,
            height: HEIGHT,
            primer: Primer::WhiteGesso,
            weave: Weave::Linen,
            pigment_slots: vec!["titanium_white".into(), "ultramarine_blue".into()],
        },
    )
    .unwrap();
    if let Some(reserve_factor) = reserve_factor {
        session
            .engine
            .params_mut()
            .set("reserve_factor", reserve_factor);
    }
    session.brush.recipe = vec![(BLUE_SLOT, 1.0)];
    session.brush.load = load;
    session.reload_brush(gpu);
    session
}

fn paint_long_stroke(gpu: &GpuContext, session: &mut PaintSession) {
    for index in 0..=SAMPLES {
        let sample = StrokeSample {
            x: STROKE_FROM_PX + STROKE_LENGTH_PX * f64::from(index) / f64::from(SAMPLES),
            y: ROW,
            pressure: 1.0,
            tilt_x_deg: PEN_TILT_DEG,
            tilt_y_deg: PEN_TILT_DEG,
            time_ms: index * 8,
        };
        if index == 0 {
            session.begin_stroke(gpu, sample);
        } else {
            session.add_samples(&[sample]);
        }
        if index % SAMPLES_PER_FRAME == 0 {
            encode_frame(gpu, session);
        }
    }
    session.end_stroke();
    encode_frame(gpu, session);
}

fn blue_along_the_row(gpu: &GpuContext, session: &PaintSession) -> Vec<f32> {
    let volume = session.canvas.read_f32(gpu, StateTexture::WetVol).unwrap();
    let pigment = session.canvas.read_f32(gpu, StateTexture::WetPig0).unwrap();
    let row_start = ROW as usize * WIDTH as usize + STROKE_FROM_PX as usize;
    (row_start..row_start + STROKE_LENGTH_PX as usize)
        .map(|texel| pigment[texel * PIGMENT_CHANNELS_PER_TEXTURE + BLUE_SLOT] * volume[texel])
        .collect()
}

fn blue_field(gpu: &GpuContext, session: &PaintSession) -> Vec<f32> {
    let volume = session.canvas.read_f32(gpu, StateTexture::WetVol).unwrap();
    let pigment = session.canvas.read_f32(gpu, StateTexture::WetPig0).unwrap();
    (0..volume.len())
        .map(|texel| pigment[texel * PIGMENT_CHANNELS_PER_TEXTURE + BLUE_SLOT] * volume[texel])
        .collect()
}

fn blue_across_the_mark(gpu: &GpuContext, session: &PaintSession) -> Vec<f32> {
    let blue = blue_field(gpu, session);
    let at = |x: usize, y: usize| blue[y * WIDTH as usize + STROKE_FROM_PX as usize + x];
    let stroke = 0..STROKE_LENGTH_PX as usize;
    let mark_rows: Vec<usize> = (0..HEIGHT as usize)
        .filter(|&y| stroke.clone().any(|x| at(x, y) > VISIBLE_PIGMENT_UM))
        .collect();
    stroke
        .map(|x| mark_rows.iter().map(|&y| at(x, y)).sum::<f32>() / mark_rows.len().max(1) as f32)
        .collect()
}

#[test]
fn tilted_default_brush_with_a_tenfold_reserve_still_lays_paint_a_thousand_pixels_into_the_stroke()
{
    let blue = blue_along_a_long_stroke(cadmium_engine::brush::DEFAULT_LOAD, Some(TENFOLD_RESERVE));
    let near_the_end = (STROKE_LENGTH_PX * 0.95) as usize;
    assert!(
        blue[near_the_end] > VISIBLE_PIGMENT_UM,
        "{} µm of blue",
        blue[near_the_end]
    );
}

#[test]
fn a_low_load_with_the_default_reserve_runs_dry_within_460_px() {
    let blue = blue_across_a_long_stroke(LOW_LOAD);
    let last_visible = blue.iter().rposition(|amount| *amount > VISIBLE_PIGMENT_UM);
    assert!(
        last_visible.is_some_and(|px| px < LOW_LOAD_RUN_PX),
        "last visible blue at {last_visible:?} px"
    );
}

#[test]
fn a_full_load_with_the_default_reserve_still_paints_440_px_into_the_stroke() {
    let blue = blue_along_a_long_stroke(FULL_LOAD, None);
    assert!(
        blue[DEFAULT_RESERVE_RUN_PX] > VISIBLE_PIGMENT_UM,
        "{} µm of blue",
        blue[DEFAULT_RESERVE_RUN_PX]
    );
}
