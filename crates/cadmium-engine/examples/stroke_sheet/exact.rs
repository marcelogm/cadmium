use std::path::{Path, PathBuf};

use cadmium_engine::canvas::StateTexture;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;

use crate::painting::{Candidate, painted_session};
use crate::scenarios::cells;

const STATE_SUFFIX: &str = ".state";

fn readouts(gpu: &GpuContext, session: &mut PaintSession) -> String {
    let lost = "the GPU stays available while reading the brush";
    let brush = session.read_brush(gpu).expect(lost);
    let engine = &session.engine;
    format!(
        "{brush:?}\n{:?}\n{:?}\n{:?}\n",
        engine.brush_totals(gpu).expect(lost),
        engine.bristle_cells(gpu).expect(lost),
        engine.bristle_snapshot(gpu).expect(lost),
    )
}

fn raw_state(gpu: &GpuContext, session: &mut PaintSession) -> Vec<u8> {
    let mut bytes = Vec::new();
    for which in StateTexture::PAINT_STATE {
        let texels = session
            .canvas
            .read_bytes(gpu, which)
            .expect("the GPU stays available while reading the canvas");
        bytes.extend(texels);
    }
    bytes.extend(readouts(gpu, session).into_bytes());
    bytes
}

/// Paints every cell as the engine paints it today and writes, for each, the raw bytes of
/// every paint state texture followed by the brush readouts, into `<out dir>/<cell>.state`.
pub(crate) fn dump(gpu: &GpuContext, out_dir: &Path) {
    std::fs::create_dir_all(out_dir).expect("the dump directory can be made");
    for cell in cells() {
        let mut session = painted_session(gpu, &cell, Candidate::Reference);
        let path = out_dir.join(format!("{}{STATE_SUFFIX}", cell.file_stem()));
        std::fs::write(&path, raw_state(gpu, &mut session)).expect("the dump can be written");
        println!("dumped {}", path.display());
    }
}

fn state_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("the dump directory can be read")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.to_string_lossy().ends_with(STATE_SUFFIX))
        .collect();
    files.sort();
    files
}

fn first_difference(reference: &[u8], candidate: &[u8]) -> Option<usize> {
    reference
        .iter()
        .zip(candidate)
        .position(|(before, after)| before != after)
        .or_else(|| (reference.len() != candidate.len()).then_some(reference.len()))
}

/// Compares two dumps byte for byte and exits non-zero when any cell differs or is missing.
pub(crate) fn same(reference_dir: &Path, candidate_dir: &Path) {
    let mut differing = 0;
    let references = state_files(reference_dir);
    for reference in &references {
        let name = reference.file_name().expect("a dump file has a name");
        let before = std::fs::read(reference).expect("the reference dump can be read");
        let after = std::fs::read(candidate_dir.join(name)).unwrap_or_default();
        if let Some(offset) = first_difference(&before, &after) {
            println!("DIFFERS {} at byte {offset}", name.to_string_lossy());
            differing += 1;
        }
    }
    println!("{} cells, {differing} differ", references.len());
    if differing > 0 || references.is_empty() {
        std::process::exit(1);
    }
}
