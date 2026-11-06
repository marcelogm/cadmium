//! Headless brush studio: the real simulation on small canvases, for previews.

mod demo;
mod example;
mod loading;
mod look;
mod paint;
mod schedule;
mod studio;
mod test_sheet;
mod tip;

pub use demo::{DemoStroke, StrokeBox, samples};
pub use example::{EXAMPLE_SHEET_SIZE, ExampleRequest, ExampleSheet};
pub use look::{HairPreset, HeadPreset, hair_name, head_name};
pub use paint::{DEMO_PIGMENT, StudioPaint};
pub use studio::{BrushStudio, StudioError, THUMBNAIL_SIZE, Thumbnail};
pub use test_sheet::{SheetFrame, TEST_SHEET_SIZE, TestSheet};
pub use tip::{Dimensions, Ferrule, TipGeometry, TipView};
