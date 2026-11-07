use cadmium_engine::brush::factory_brushes;
use cadmium_engine::canvas::StateTexture;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::stroke::StrokeSample;

use crate::patches::patch_areas_px2;
use crate::support::{drag, session};

const ANGULAR_BRUSH: &str = "Angular · Synthetic";
const DIAMETER_PX: f32 = 40.0;
const ROW_PX: f64 = 128.0;
const FROM_PX: f64 = 40.0;
const TO_PX: f64 = 470.0;
const SAMPLES: u32 = 400;
const SAMPLE_INTERVAL_MS: u32 = 3;
const VISIBLE_UM: f32 = 0.5;
const MAX_SPECK_PX2: usize = 40;
const MAX_STRAY_SPECKS: usize = 3;
/// Pen holds that left a row of dots beside an angular brush's stroke: (tilt x, tilt y,
/// pressure).
const HOLDS: [(f32, f32, f32); 4] = [
    (30.0, -30.0, 0.3),
    (-40.0, 0.0, 0.5),
    (-30.0, -30.0, 0.5),
    (55.0, 20.0, 0.1),
];

fn stray_specks(volume: &[f32], width: usize) -> usize {
    let painted: Vec<bool> = volume.iter().map(|&um| um > VISIBLE_UM).collect();
    let mut seen = vec![false; painted.len()];
    patch_areas_px2(&painted, &mut seen, width)
        .into_iter()
        .filter(|&area| area <= MAX_SPECK_PX2)
        .count()
}

fn line(tilt_x_deg: f32, tilt_y_deg: f32, pressure: f32) -> Vec<StrokeSample> {
    (0..=SAMPLES)
        .map(|index| StrokeSample {
            x: FROM_PX + (TO_PX - FROM_PX) * f64::from(index) / f64::from(SAMPLES),
            y: ROW_PX,
            pressure,
            tilt_x_deg,
            tilt_y_deg,
            time_ms: index * SAMPLE_INTERVAL_MS,
        })
        .collect()
}

#[test]
fn an_angular_brush_lays_no_row_of_specks_beside_its_stroke() {
    let gpu = GpuContext::new().unwrap();
    let brush = factory_brushes()
        .into_iter()
        .find(|brush| brush.name == ANGULAR_BRUSH)
        .unwrap();
    for (tilt_x_deg, tilt_y_deg, pressure) in HOLDS {
        let mut s = session(&gpu);
        s.brush.wear(&brush);
        s.brush.geometry.max_diameter_px = DIAMETER_PX;
        s.reload_brush(&gpu);
        drag(&gpu, &mut s, &line(tilt_x_deg, tilt_y_deg, pressure));
        let volume = s.canvas.read_f32(&gpu, StateTexture::WetVol).unwrap();
        let specks = stray_specks(&volume, s.canvas.spec.width as usize);
        assert!(
            specks <= MAX_STRAY_SPECKS,
            "tilt ({tilt_x_deg}, {tilt_y_deg}) at pressure {pressure} left {specks} specks"
        );
    }
}
