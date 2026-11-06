//! Headless brush studio: the real simulation on a small canvas, for previews.

mod clock;
mod demo;
mod fit;
mod loading;
mod look;
mod player;
mod schedule;
mod studio;

pub use clock::Phase;
pub use demo::{DemoStroke, StrokeBox, samples};
pub use look::{hair_name, head_name};
pub use player::{PlayerRequest, SLOW_MOTION, StudioPaint, StudioPlayer};
pub use schedule::QUANTUM_MS;
pub use studio::{BrushStudio, StudioError, THUMBNAIL_SIZE, Thumbnail};
