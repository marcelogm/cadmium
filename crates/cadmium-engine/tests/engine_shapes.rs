use cadmium_engine::{
    brush::BrushShape, canvas::*, gpu::GpuContext, session::PaintSession, stroke::StrokeSample,
};

const SIDE: u32 = 512;
const CENTRE: f64 = 256.0;
const DIAMETER_PX: f32 = 120.0;
const STROKE_HALF_LENGTH_PX: f64 = 120.0;
const SAMPLE_SPACING_PX: f64 = 3.0;
const PRESSURE: f32 = 0.8;
const LEANING_TILT_DEG: f32 = 40.0;
const PAINTED_UM: f32 = 1.0;
const ANGLE_TOLERANCE_RAD: f32 = 1e-3;
const MAX_FLAT_THICKNESS_SHARE: f32 = 0.3;
const MIN_BROADSIDE_TO_EDGE_RATIO: f64 = 2.5;

fn session(gpu: &GpuContext, shape: BrushShape) -> PaintSession {
    let mut s = PaintSession::new(
        gpu,
        CanvasSpec {
            width: SIDE,
            height: SIDE,
            primer: Primer::WhiteGesso,
            weave: Weave::Smooth,
            pigment_slots: vec!["ultramarine_blue".into()],
        },
    )
    .unwrap();
    s.brush.shape = shape;
    s.brush.geometry.max_diameter_px = DIAMETER_PX;
    s.brush.recipe = vec![(0, 1.0)];
    s.reload_brush(gpu);
    s
}

fn pen(tilt_x_deg: f32, tilt_y_deg: f32) -> StrokeSample {
    StrokeSample {
        x: CENTRE,
        y: CENTRE,
        pressure: PRESSURE,
        tilt_x_deg,
        tilt_y_deg,
        time_ms: 0,
    }
}

fn axial_difference(a: f32, b: f32) -> f32 {
    let difference = (a - b).rem_euclid(std::f32::consts::PI);
    difference.min(std::f32::consts::PI - difference)
}

fn painted_extent(gpu: &GpuContext, s: &PaintSession, horizontal: bool) -> f64 {
    let volume = s.canvas.read_f32(gpu, StateTexture::WetVol).unwrap();
    let side = SIDE as usize;
    let centre = CENTRE as usize;
    (0..side)
        .filter(|&offset| {
            let index = if horizontal {
                centre * side + offset
            } else {
                offset * side + centre
            };
            volume[index] > PAINTED_UM
        })
        .count() as f64
}

fn paint_straight(gpu: &GpuContext, s: &mut PaintSession, horizontal: bool) {
    let count = (2.0 * STROKE_HALF_LENGTH_PX / SAMPLE_SPACING_PX) as u32;
    for i in 0..=count {
        let along = -STROKE_HALF_LENGTH_PX + f64::from(i) * SAMPLE_SPACING_PX;
        let (x, y) = if horizontal {
            (CENTRE + along, CENTRE)
        } else {
            (CENTRE, CENTRE + along)
        };
        let sample = StrokeSample {
            x,
            y,
            time_ms: i * 4,
            ..pen(0.0, 0.0)
        };
        if i == 0 {
            s.begin_stroke(gpu, sample)
        } else {
            s.add_samples(&[sample])
        }
    }
    s.end_stroke();
    let mut e = gpu.device.create_command_encoder(&Default::default());
    s.encode_frame(gpu, &mut e);
    gpu.queue.submit([e.finish()]);
}

#[test]
fn a_round_brush_outline_is_a_circle_when_the_pen_is_upright() {
    let gpu = GpuContext::new().unwrap();
    let s = session(&gpu, BrushShape::Round);
    let outline = s.brush_outline(&pen(0.0, 0.0)).unwrap();
    assert_eq!(outline.semi_axes[0], outline.semi_axes[1]);
}

#[test]
fn a_flat_brush_outline_is_a_horizontal_bar_when_the_pen_is_upright() {
    let gpu = GpuContext::new().unwrap();
    let s = session(&gpu, BrushShape::Flat);
    let outline = s.brush_outline(&pen(0.0, 0.0)).unwrap();
    let [thickness, width] = outline.semi_axes;
    assert!(
        thickness < MAX_FLAT_THICKNESS_SHARE * width,
        "{thickness} vs {width}"
    );
    assert!(
        axial_difference(outline.orientation, std::f32::consts::FRAC_PI_2) < ANGLE_TOLERANCE_RAD
    );
}

#[test]
fn a_fan_brush_outline_is_thinner_than_a_flat_one() {
    let gpu = GpuContext::new().unwrap();
    let flat = session(&gpu, BrushShape::Flat);
    let fan = session(&gpu, BrushShape::Fan);
    let flat_outline = flat.brush_outline(&pen(0.0, 0.0)).unwrap();
    let fan_outline = fan.brush_outline(&pen(0.0, 0.0)).unwrap();
    assert!(fan_outline.semi_axes[0] < flat_outline.semi_axes[0]);
    assert_eq!(fan_outline.semi_axes[1], flat_outline.semi_axes[1]);
}

#[test]
fn a_flat_brush_turns_its_thin_side_towards_the_lean_of_the_pen() {
    let gpu = GpuContext::new().unwrap();
    let s = session(&gpu, BrushShape::Flat);
    let outline = s.brush_outline(&pen(LEANING_TILT_DEG, 0.0)).unwrap();
    assert!(axial_difference(outline.orientation, 0.0) < ANGLE_TOLERANCE_RAD);
}

#[test]
fn a_flat_brush_paints_broadside_wide_and_edgewise_thin() {
    let gpu = GpuContext::new().unwrap();
    let mut edgewise = session(&gpu, BrushShape::Flat);
    paint_straight(&gpu, &mut edgewise, true);
    let edge = painted_extent(&gpu, &edgewise, false);
    let mut broadside = session(&gpu, BrushShape::Flat);
    paint_straight(&gpu, &mut broadside, false);
    let broad = painted_extent(&gpu, &broadside, true);
    eprintln!("edgewise {edge} px, broadside {broad} px");
    assert!(edge > 0.0);
    assert!(
        broad / edge > MIN_BROADSIDE_TO_EDGE_RATIO,
        "{broad} / {edge}"
    );
}
