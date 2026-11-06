//! Headless brush studio: the real simulation on a small canvas, for previews.

mod demo;
mod loading;
mod look;
mod paint;
mod schedule;
mod studio;

pub use demo::{DemoStroke, StrokeBox, samples};
pub use look::{hair_name, head_name};
pub use paint::StudioPaint;
pub use studio::{BrushStudio, StudioError, THUMBNAIL_SIZE, Thumbnail};
