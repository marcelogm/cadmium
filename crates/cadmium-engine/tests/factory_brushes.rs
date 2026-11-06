pub mod support;

use cadmium_engine::{brush::factory_brushes, canvas::StateTexture, gpu::GpuContext};
use support::{session, stroke};

const STROKE_Y_PX: f64 = 128.0;
const STROKE_FROM_PX: f64 = 140.0;
const STROKE_TO_PX: f64 = 372.0;

#[test]
fn every_factory_brush_lays_finite_paint() {
    let gpu = GpuContext::new().unwrap();
    for brush in factory_brushes() {
        let mut s = session(&gpu);
        s.brush.wear(&brush);
        s.reload_brush(&gpu);
        stroke(&gpu, &mut s, STROKE_Y_PX, STROKE_FROM_PX, STROKE_TO_PX);
        let volume = s.canvas.read_f32(&gpu, StateTexture::WetVol).unwrap();
        assert!(
            volume.iter().all(|um| um.is_finite()),
            "{} left a non-finite wet volume",
            brush.name
        );
        let laid: f64 = volume.iter().map(|&um| f64::from(um)).sum();
        assert!(laid > 0.0, "{} laid no paint", brush.name);
    }
}
