//! Headless brush studio: the real simulation on a small canvas, for previews.

mod clock;
mod demo;
mod fit;
mod loading;
mod player;
mod schedule;
mod studio;

pub use clock::Phase;
pub use demo::{DemoStroke, StrokeBox, samples};
pub use player::{LOOP_PAUSE_MS, PlayerRequest, SLOW_MOTION, StudioPaint, StudioPlayer};
pub use schedule::{QUANTUM_MS, SETTLE_MS, StrokeSchedule, StudioEvent};
pub use studio::{BrushStudio, StudioError, THUMBNAIL_SIZE, Thumbnail};
