//! Headless brush studio: the real simulation on a small canvas, for previews.

mod demo;
mod loading;
mod look;
mod paint;
mod schedule;
mod studio;
mod tip;

pub use demo::{DemoStroke, StrokeBox, samples};
pub use look::{HairPreset, HeadPreset, hair_name, head_name};
pub use paint::StudioPaint;
pub use studio::{BrushStudio, StudioError, THUMBNAIL_SIZE, Thumbnail};
pub use tip::{Dimensions, Ferrule, TipGeometry, TipView};
