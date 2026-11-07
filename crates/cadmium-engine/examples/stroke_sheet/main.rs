//! Paints a sheet of strokes twice, as the engine paints them today and with a candidate
//! change, and writes a page that shows where the candidate changes the painting.
//!
//! cargo run --release -p cadmium-engine --example stroke_sheet -- <candidate> [out dir]
//!
//! `<candidate>` is `limit=<dabs>`, painting at most that many dabs per frame, or
//! `spacing=<share>`, spacing dabs by that share of the brush diameter. Every stroke crosses
//! wet stripes of yellow, so smearing and pick-up show too. Rows are tool, size and load;
//! columns are pen speed. The page lands in `<out dir>/index.html`, by default
//! `target/stroke_sheet/<candidate>`.
//!
//! `-- dump <dir>` paints every cell as today and writes its raw paint state and brush
//! readouts to `<dir>`; `-- same <reference dir> <candidate dir>` then checks two dumps,
//! made by two builds, are identical byte for byte. `SHEET_ROWS=<text>` keeps only the rows
//! whose name contains the text.

mod compare;
mod exact;
mod page;
mod painting;
mod scenarios;

use std::path::PathBuf;

use cadmium_engine::gpu::GpuContext;

use compare::Comparison;
use painting::{Candidate, paint_cell};
use scenarios::{Cell, cells};

const DEFAULT_OUT_ROOT: &str = "target/stroke_sheet";

struct Arguments {
    candidate: Candidate,
    name: String,
    out_dir: PathBuf,
}

fn arguments() -> Arguments {
    let mut args = std::env::args().skip(1);
    let name = args.next().unwrap_or_else(|| "limit=4".to_string());
    let candidate = Candidate::parse(&name).unwrap_or_else(|| {
        eprintln!("candidate must be limit=<dabs> or spacing=<share>, not {name}");
        std::process::exit(2);
    });
    let out_dir = args.next().map_or_else(
        || PathBuf::from(DEFAULT_OUT_ROOT).join(&name),
        PathBuf::from,
    );
    Arguments {
        candidate,
        name,
        out_dir,
    }
}

pub(crate) struct Result {
    pub(crate) cell: Cell,
    pub(crate) comparison: Comparison,
}

fn exact_mode() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [mode, out_dir] if mode == "dump" => {
            let gpu = GpuContext::new().expect("a Vulkan adapter to paint on");
            exact::dump(&gpu, &PathBuf::from(out_dir));
        }
        [mode, reference, candidate] if mode == "same" => {
            exact::same(&PathBuf::from(reference), &PathBuf::from(candidate));
        }
        _ => return false,
    }
    true
}

fn main() {
    if exact_mode() {
        return;
    }
    let arguments = arguments();
    let gpu = GpuContext::new().expect("a Vulkan adapter to paint on");
    std::fs::create_dir_all(&arguments.out_dir).expect("the output directory can be made");
    let mut results = Vec::new();
    for cell in cells() {
        let reference = paint_cell(&gpu, &cell, Candidate::Reference);
        let candidate = paint_cell(&gpu, &cell, arguments.candidate);
        let comparison = compare::compare(&reference, &candidate);
        page::write_cell_images(
            &arguments.out_dir,
            &cell,
            &reference,
            &candidate,
            &comparison,
        );
        println!(
            "{:<34} {:>5} px/s  mean {:>6.3}  max {:>3}  changed {:>5.2}%",
            cell.row_name(),
            cell.speed_px_per_second,
            comparison.mean_difference,
            comparison.max_difference,
            comparison.changed_share * 100.0
        );
        results.push(Result { cell, comparison });
    }
    let index = page::write_index(&arguments.out_dir, &arguments.name, &results);
    println!("sheet: {}", index.display());
}
