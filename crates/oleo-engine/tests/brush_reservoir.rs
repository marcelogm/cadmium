use oleo_engine::{canvas::*, gpu::GpuContext, session::PaintSession, stroke::StrokeSample};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 128;
const STROKE_FROM_PX: f64 = 100.0;
const STROKE_LENGTH_PX: f64 = 1000.0;
const SAMPLES: u32 = 250;
const SAMPLES_PER_FRAME: u32 = 4;
const ROW: f64 = 64.0;
const VISIBLE_PIGMENT_UM: f32 = 3.0;
const BLUE_SLOT: usize = 1;
const PIGMENT_CHANNELS_PER_TEXTURE: usize = 4;

fn encode_frame(gpu: &GpuContext, session: &mut PaintSession) {
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    session.encode_frame(gpu, &mut encoder);
    gpu.queue.submit([encoder.finish()]);
}

#[test]
fn default_brush_at_full_pressure_still_lays_paint_a_thousand_pixels_into_the_stroke() {
    let gpu = GpuContext::new().unwrap();
    let mut session = PaintSession::new(
        &gpu,
        CanvasSpec {
            width: WIDTH,
            height: HEIGHT,
            primer: Primer::WhiteGesso,
            weave: Weave::Smooth,
            pigment_slots: vec!["titanium_white".into(), "ultramarine_blue".into()],
        },
    );
    session.brush.recipe = vec![(BLUE_SLOT, 1.0)];
    session.reload_brush(&gpu);
    for index in 0..=SAMPLES {
        let sample = StrokeSample {
            x: STROKE_FROM_PX + STROKE_LENGTH_PX * f64::from(index) / f64::from(SAMPLES),
            y: ROW,
            pressure: 1.0,
            tilt_x_deg: 20.0,
            tilt_y_deg: 0.0,
            time_ms: index * 8,
        };
        if index == 0 {
            session.begin_stroke(&gpu, sample);
        } else {
            session.add_samples(&[sample]);
        }
        if index % SAMPLES_PER_FRAME == 0 {
            encode_frame(&gpu, &mut session);
        }
    }
    session.end_stroke(&gpu);
    encode_frame(&gpu, &mut session);

    let volume = session.canvas.read_f32(&gpu, StateTexture::WetVol);
    let pigment = session.canvas.read_f32(&gpu, StateTexture::WetPig0);
    let row_start = ROW as usize * WIDTH as usize;
    let near_the_end = (STROKE_FROM_PX + STROKE_LENGTH_PX * 0.95) as usize;
    let texel = row_start + near_the_end;
    let blue_um = pigment[texel * PIGMENT_CHANNELS_PER_TEXTURE + BLUE_SLOT] * volume[texel];
    assert!(blue_um > VISIBLE_PIGMENT_UM, "{blue_um} µm of blue");
}
