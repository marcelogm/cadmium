//! Headless brush studio: the real simulation on a small canvas, for previews.

mod demo;
mod fit;
mod player;
mod schedule;
mod studio;

pub use demo::{DemoStroke, StrokeBox, samples};
pub use player::{LOOP_PAUSE_MS, Phase, PlayerRequest, SLOW_MOTION, StudioPaint, StudioPlayer};
pub use schedule::{QUANTUM_MS, SETTLE_MS, StrokeSchedule, StudioEvent};
pub use studio::{BrushStudio, StudioError, THUMBNAIL_SIZE, Thumbnail};
