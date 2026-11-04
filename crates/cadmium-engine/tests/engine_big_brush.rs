use cadmium_engine::{
    canvas::*,
    gpu::GpuContext,
    present::{ViewTransform, readback_rgba8},
    session::PaintSession,
    stroke::StrokeSample,
};

const WIDTH: u32 = 2048;
const HEIGHT: u32 = 1536;
const FROM: [f64; 2] = [300.0, 1200.0];
const TO: [f64; 2] = [1700.0, 400.0];
const SAMPLE_SPACING_PX: f64 = 6.0;
const SAMPLE_INTERVAL_MS: u32 = 4;
const FRAME_EVERY_SAMPLES: usize = 2;
const MAX_DIAMETER_PX: f32 = 500.0;
const LIGHT_PRESSURE: f32 = 0.3;
const TILT_X_DEG: f32 = 30.0;
const TILT_Y_DEG: f32 = 10.0;
const FULL_LOAD: f32 = 1.0;
const MEASURED_FROM: f64 = 0.25;
const MEASURED_TO: f64 = 0.75;
const MEASURED_SECTIONS: u32 = 40;
const ACROSS_REACH_PX: i32 = 400;
const PAINTED_DARKENING: f64 = 0.3;
const MAX_WIDTH_SPREAD: f64 = 1.08;

fn session(gpu: &GpuContext) -> PaintSession {
    let mut s = PaintSession::new(
        gpu,
        CanvasSpec {
            width: WIDTH,
            height: HEIGHT,
            primer: Primer::WhiteGesso,
            weave: Weave::Linen,
            pigment_slots: vec!["ultramarine_blue".into(), "titanium_white".into()],
        },
    )
    .unwrap();
    s.brush.geometry.max_diameter_px = MAX_DIAMETER_PX;
    s.brush.load = FULL_LOAD;
    s.brush.recipe = vec![(0, 1.0)];
    s.reload_brush(gpu);
    s
}

fn frame(gpu: &GpuContext, s: &mut PaintSession) {
    let mut e = gpu.device.create_command_encoder(&Default::default());
    s.encode_frame(gpu, &mut e);
    gpu.queue.submit([e.finish()]);
}

fn along(t: f64) -> [f64; 2] {
    [
        FROM[0] + (TO[0] - FROM[0]) * t,
        FROM[1] + (TO[1] - FROM[1]) * t,
    ]
}

fn paint_light_stroke(gpu: &GpuContext, s: &mut PaintSession) {
    let length = (TO[0] - FROM[0]).hypot(TO[1] - FROM[1]);
    let count = (length / SAMPLE_SPACING_PX) as u32;
    for i in 0..=count {
        let [x, y] = along(f64::from(i) / f64::from(count));
        let sample = StrokeSample {
            x,
            y,
            pressure: LIGHT_PRESSURE,
            tilt_x_deg: TILT_X_DEG,
            tilt_y_deg: TILT_Y_DEG,
            time_ms: i * SAMPLE_INTERVAL_MS,
        };
        if i == 0 {
            s.begin_stroke(gpu, sample)
        } else {
            s.add_samples(&[sample])
        }
        if i as usize % FRAME_EVERY_SAMPLES == 0 {
            frame(gpu, s);
        }
    }
    s.end_stroke();
    frame(gpu, s);
}

fn colour_view(gpu: &GpuContext, s: &PaintSession) -> Vec<u8> {
    let target = s.presenter.create_target(gpu, WIDTH, HEIGHT);
    let mut e = gpu.device.create_command_encoder(&Default::default());
    s.presenter.encode_present(
        gpu,
        &s.canvas,
        ViewTransform {
            origin_x: 0.0,
            origin_y: 0.0,
            zoom: 1.0,
        },
        &target,
        &mut e,
    );
    gpu.queue.submit([e.finish()]);
    readback_rgba8(gpu, &target).unwrap()
}

fn brightness(rgba: &[u8], x: i32, y: i32) -> f64 {
    let index = (y as usize * WIDTH as usize + x as usize) * 4;
    rgba[index..index + 3]
        .iter()
        .map(|&channel| f64::from(channel) / 255.0)
        .sum::<f64>()
        / 3.0
}

fn painted_width(rgba: &[u8], centre: [f64; 2], normal: [f64; 2], primer: f64) -> u32 {
    (-ACROSS_REACH_PX..=ACROSS_REACH_PX)
        .filter(|&offset| {
            let x = (centre[0] + normal[0] * f64::from(offset)).round() as i32;
            let y = (centre[1] + normal[1] * f64::from(offset)).round() as i32;
            brightness(rgba, x, y) < primer - PAINTED_DARKENING
        })
        .count() as u32
}

#[test]
fn a_heavily_loaded_wide_brush_keeps_its_width_along_a_light_stroke() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    paint_light_stroke(&gpu, &mut s);
    let rgba = colour_view(&gpu, &s);
    let primer = brightness(&rgba, 4, 4);
    let length = (TO[0] - FROM[0]).hypot(TO[1] - FROM[1]);
    let normal = [(FROM[1] - TO[1]) / length, (TO[0] - FROM[0]) / length];
    let widths: Vec<u32> = (0..=MEASURED_SECTIONS)
        .map(|section| {
            let t = MEASURED_FROM
                + (MEASURED_TO - MEASURED_FROM) * f64::from(section) / f64::from(MEASURED_SECTIONS);
            painted_width(&rgba, along(t), normal, primer)
        })
        .collect();
    let narrowest = *widths.iter().min().unwrap();
    let widest = *widths.iter().max().unwrap();
    eprintln!("stroke widths {narrowest}..{widest} px: {widths:?}");
    assert!(narrowest > 0);
    assert!(f64::from(widest) / f64::from(narrowest) < MAX_WIDTH_SPREAD);
}
