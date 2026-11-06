//! Headless brush studio: the real simulation on a small canvas, for previews.

mod camera;
mod clock;
mod demo;
mod fit;
mod loading;
mod look;
mod mesh;
mod player;
mod proportions;
mod scene;
mod schedule;
mod studio;

pub use camera::OrbitCamera;
pub use clock::Phase;
pub use demo::{DemoStroke, StrokeBox, samples};
pub use look::{hair_name, hair_tint, head_name};
pub use mesh::{Mesh, MeshVertex, ferrule_mesh, handle_mesh};
pub use player::{LOOP_PAUSE_MS, PlayerRequest, SLOW_MOTION, StudioPaint, StudioPlayer};
pub use scene::{SceneFrame, SceneLayers, SceneRenderer, SceneRequest};
pub use schedule::{QUANTUM_MS, SETTLE_MS, StrokeSchedule, StudioEvent};
pub use studio::{BrushStudio, StudioError, THUMBNAIL_SIZE, Thumbnail};
