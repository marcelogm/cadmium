//! Headless brush studio: the real simulation on a small canvas, for previews.

mod demo;
mod studio;

pub use demo::{DemoStroke, StrokeBox, samples};
pub use studio::{BrushStudio, StudioError, THUMBNAIL_SIZE, Thumbnail};
