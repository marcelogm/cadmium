#[path = "../support/mod.rs"]
pub mod support;

mod entries;
mod export;
mod read;
mod save;
mod tray;

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use cadmium_engine::{
    canvas::{CanvasSpec, Primer, StateTexture, Weave},
    gpu::GpuContext,
    session::PaintSession,
    stroke::StrokeSample,
    surface::Surface,
    tube::Nozzle,
};

use crate::support::encode_frame;

const BLANK_WIDTH: u32 = 64;
const BLANK_HEIGHT: u32 = 32;
const FORMAT_1_DRY_HEIGHT: &str = "state/dry_height.zst";
const DRY_RELIEF: &str = "state/dry_relief.zst";
const INVALID_THICKNESS: &str = "holds a negative or non-finite thickness";
const FORMAT_1_STATE_ENTRIES: [(&str, StateTexture); 8] = [
    ("state/wet_pig0.zst", StateTexture::WetPig0),
    ("state/wet_pig1.zst", StateTexture::WetPig1),
    ("state/wet_vol.zst", StateTexture::WetVol),
    ("state/dry_refl0.zst", StateTexture::DryRefl0),
    ("state/dry_refl1.zst", StateTexture::DryRefl1),
    (FORMAT_1_DRY_HEIGHT, StateTexture::DryHeight),
    ("state/dry_top_pig0.zst", StateTexture::DryTopPig0),
    ("state/dry_top_pig1.zst", StateTexture::DryTopPig1),
];

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

fn squeeze_on_the_tray(gpu: &GpuContext, s: &mut PaintSession) {
    squeeze_on_the_tray_at(gpu, s, [300.0, 300.0]);
}

fn squeeze_on_the_tray_at(gpu: &GpuContext, s: &mut PaintSession, at: [f32; 2]) {
    s.begin_hold(gpu, Surface::Tray);
    s.squeeze(
        gpu,
        Nozzle {
            from: at,
            to: at,
            radius: 12.0,
            slot: 1,
            pressure: 1.0,
            seconds: 0.2,
            still_reach_px: 0.0,
        },
    );
    s.end_hold();
}
