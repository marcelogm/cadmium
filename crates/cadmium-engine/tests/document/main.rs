#[path = "../support/mod.rs"]
pub mod support;

mod entries;
mod export;
mod palette;
mod read;
mod save;
mod sheet;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use cadmium_engine::{
    canvas::{CanvasSpec, Primer, StateTexture, Weave},
    document,
    gpu::GpuContext,
    session::PaintSession,
    stroke::StrokeSample,
};

use crate::support::encode_frame;

const CURRENT_FORMAT: u32 = 9;
const SKETCH_DENSITY: f32 = 0.75;
const BLANK_WIDTH: u32 = 64;
const BLANK_HEIGHT: u32 = 32;
const DRY_RELIEF: &str = "state/dry_relief.zst";
const GRAPHITE_ENTRY: &str = "state/graphite.zst";
const INVALID_THICKNESS: &str = "holds a negative or non-finite thickness";

/// Writes a one-texel pencil sketch on the painting and returns the whole graphite field.
fn sketch_on_the_painting(gpu: &GpuContext, s: &PaintSession) -> Vec<f32> {
    let mut sketch = vec![0.0; (s.canvas.spec.width * s.canvas.spec.height) as usize];
    sketch[0] = SKETCH_DENSITY;
    s.canvas.write_f32(gpu, StateTexture::Graphite, &sketch);
    sketch
}

static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let unique = format!(
            "cadmium-doc-{}-{}-{name}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        );
        let dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn sample(x: f64, y: f64, time_ms: u32) -> StrokeSample {
    StrokeSample {
        x,
        y,
        pressure: 1.0,
        tilt_x_deg: 0.0,
        tilt_y_deg: 0.0,
        time_ms,
    }
}

fn painted_spec() -> CanvasSpec {
    CanvasSpec {
        width: 256,
        height: 128,
        primer: Primer::SiennaImprimatura,
        weave: Weave::Cotton,
        pigment_slots: vec!["burnt_sienna".into(), "titanium_white".into()],
        margin_px: 0,
    }
}

fn session_with_ended_stroke(gpu: &GpuContext, from: (f64, f64), to: (f64, f64)) -> PaintSession {
    let mut s = PaintSession::new(gpu, painted_spec()).unwrap();
    s.begin_stroke(sample(from.0, from.1, 0));
    s.add_samples(&[sample(to.0, to.1, 200)]);
    s.end_stroke();
    s
}

fn painted_session(gpu: &GpuContext) -> PaintSession {
    painted_session_with_stroke(gpu, (20.0, 60.0), (230.0, 70.0))
}

fn painted_session_with_stroke(gpu: &GpuContext, from: (f64, f64), to: (f64, f64)) -> PaintSession {
    let mut s = session_with_ended_stroke(gpu, from, to);
    encode_frame(gpu, &mut s);
    s
}

fn blank_sheet(gpu: &GpuContext, painting: &PaintSession) -> PaintSession {
    PaintSession::for_sheet(gpu, &painting.canvas.spec.pigment_slots).unwrap()
}

fn save_painting(gpu: &GpuContext, painting: &mut PaintSession, path: &Path) {
    let mut sheet = blank_sheet(gpu, painting);
    document::save(gpu, painting, &mut sheet, path).unwrap();
}

fn open_painting(gpu: &GpuContext, path: &Path) -> PaintSession {
    document::open(gpu, path).unwrap().painting
}
