//! Renders the brush editor's example sheet for every factory brush, and compares two
//! renders side by side.
//!
//! ```text
//! cargo run --release -p cadmium-studio --example brush_sheets -- render <dir>
//! cargo run --release -p cadmium-studio --example brush_sheets -- compare <before> <after> <out>
//! ```
//!
//! `render` writes `<dir>/NN.png` per brush and `<dir>/brushes.tsv` naming them. `compare`
//! copies two renders into `<out>/before` and `<out>/after` and writes `<out>/index.html`.

mod page;

use std::fmt::Write as _;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use cadmium_engine::brush::factory_brushes;
use cadmium_engine::canvas::{Primer, Weave};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::params::SimParams;
use cadmium_studio::{ExampleRequest, ExampleSheet, StudioPaint, Thumbnail};

pub(crate) const BRUSHES_FILE: &str = "brushes.tsv";
const PAINT_SLOTS: [&str; 3] = ["titanium_white", "cadmium_yellow", "cadmium_red"];
const YELLOW_SHARE: f32 = 0.55;
const RED_SHARE: f32 = 0.45;
const FULL_LOAD: f32 = 1.0;
const MEDIUM: f32 = 0.1;

/// The paint every sheet is painted with: a cadmium orange, fully loaded, on linen.
fn sheet_paint() -> StudioPaint {
    StudioPaint {
        slots: PAINT_SLOTS.iter().map(|slot| slot.to_string()).collect(),
        primer: Primer::WhiteGesso,
        weave: Weave::Linen,
        recipe: vec![(1, YELLOW_SHARE), (2, RED_SHARE)],
        load: FULL_LOAD,
        medium: MEDIUM,
        params: SimParams::default(),
    }
}

fn write_png(path: &Path, shot: &Thumbnail) {
    let file = File::create(path).expect("the sheet file is writable");
    let mut encoder = png::Encoder::new(BufWriter::new(file), shot.width, shot.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("a PNG header");
    writer
        .write_image_data(&shot.rgba)
        .expect("the sheet's pixels");
}

fn render(dir: &Path) {
    std::fs::create_dir_all(dir).expect("the output directory is creatable");
    let gpu = GpuContext::new().expect("a Vulkan adapter to paint on");
    let paint = sheet_paint();
    let mut sheet = ExampleSheet::new();
    let mut names = String::new();
    for (index, brush) in factory_brushes().iter().enumerate() {
        let request = ExampleRequest {
            brush,
            paint: &paint,
        };
        let shot = sheet.paint(&gpu, request).expect("the sheet paints");
        write_png(&dir.join(format!("{index:02}.png")), &shot);
        let _ = writeln!(names, "{index:02}\t{}", brush.name);
        println!("{index:02} {}", brush.name);
    }
    std::fs::write(dir.join(BRUSHES_FILE), names).expect("the brush list is writable");
}

fn usage() -> ! {
    eprintln!("usage: brush_sheets render <dir> | compare <before> <after> <out>");
    std::process::exit(2);
}

fn main() {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    match args.as_slice() {
        [command, dir] if command.as_os_str() == "render" => render(dir),
        [command, before, after, out] if command.as_os_str() == "compare" => {
            page::compare(before, after, out);
        }
        _ => usage(),
    }
}
