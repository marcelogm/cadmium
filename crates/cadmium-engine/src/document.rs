use std::borrow::Cow;
use std::fmt;
use std::path::Path;

use crate::canvas::{
    Canvas, CanvasError, CanvasSpec, StateTexture, TILE_SIZE, TRAY_REFLECTANCE, TexelBlock,
};
use crate::gpu::{GpuContext, GpuLost, TexelRows, new_encoder, read_all_mapped, staging_buffer};
use crate::session::PaintSession;
use crate::surface::{LEGACY_TRAY_HEIGHT, LEGACY_TRAY_WIDTH, tray_spec};

mod export;
mod read;
mod relief;
mod tray_ground;
mod write;

pub use export::ExportedImage;

const FORMAT_VERSION: u32 = 5;
const ABSOLUTE_DRY_HEIGHT_FORMAT: u32 = 1;
const TRAYLESS_FORMAT: u32 = 2;
/// The format that saved the tray, at its old size.
const TRAY_FORMAT: u32 = 3;
const LEGACY_TRAY: [u32; 2] = [LEGACY_TRAY_WIDTH, LEGACY_TRAY_HEIGHT];
const PAINTING_PREFIX: &str = "";
const TRAY_PREFIX: &str = "tray/";
const DRY_RELIEF_NAME: &str = "dry_relief";
const THICKNESS_UNITS: &str = "um";
const MANIFEST_ENTRY: &str = "manifest.json";
const THICKNESS_BYTES: usize = 4;
const DENSITY_BYTES: usize = 4;
const MAX_GRAPHITE_DENSITY: f32 = 1.0;

#[derive(Debug)]
pub enum DocumentError {
    Io(std::io::Error),
    Format(String),
    UnsupportedVersion(u32),
    GpuLost,
    OutOfVideoMemory,
}

impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocumentError::Io(error) => write!(f, "{error}"),
            DocumentError::Format(reason) => write!(f, "not a valid Cadmium painting: {reason}"),
            DocumentError::UnsupportedVersion(version) => write!(
                f,
                "painting uses file format {version}, this version of Cadmium reads formats up to {FORMAT_VERSION}"
            ),
            DocumentError::GpuLost => write!(f, "{GpuLost}"),
            DocumentError::OutOfVideoMemory => write!(f, "{}", CanvasError::OutOfVideoMemory),
        }
    }
}

impl std::error::Error for DocumentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DocumentError::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for DocumentError {
    fn from(error: std::io::Error) -> Self {
        DocumentError::Io(error)
    }
}

impl From<GpuLost> for DocumentError {
    fn from(_: GpuLost) -> Self {
        DocumentError::GpuLost
    }
}

impl From<CanvasError> for DocumentError {
    fn from(error: CanvasError) -> Self {
        match error {
            CanvasError::OutOfVideoMemory => DocumentError::OutOfVideoMemory,
        }
    }
}

impl From<zip::result::ZipError> for DocumentError {
    fn from(error: zip::result::ZipError) -> Self {
        match error {
            zip::result::ZipError::Io(error) => DocumentError::Io(error),
            other => DocumentError::Format(other.to_string()),
        }
    }
}

impl From<png::EncodingError> for DocumentError {
    fn from(error: png::EncodingError) -> Self {
        match error {
            png::EncodingError::IoError(error) => DocumentError::Io(error),
            other => DocumentError::Format(other.to_string()),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Manifest {
    format: u32,
    pigment_db: u32,
    spec: CanvasSpec,
    bands: usize,
    units: String,
}

struct StateBytes {
    which: StateTexture,
    rows: TexelRows,
    bytes: Vec<u8>,
}

impl StateBytes {
    fn texels(&self) -> Cow<'_, [u8]> {
        if self.rows.is_padded() {
            Cow::Owned(self.rows.unpad(&self.bytes))
        } else {
            Cow::Borrowed(&self.bytes)
        }
    }
}

struct TraySnapshot {
    spec: CanvasSpec,
    states: Vec<StateBytes>,
}

pub struct DocumentSnapshot {
    spec: CanvasSpec,
    states: Vec<StateBytes>,
    tray: Option<TraySnapshot>,
}

impl DocumentSnapshot {
    pub fn capture(
        gpu: &GpuContext,
        session: &mut PaintSession,
    ) -> Result<DocumentSnapshot, GpuLost> {
        session.flush(gpu);
        let states = capture_canvas(gpu, &session.canvas)?;
        let tray = (session.tray.spec == tray_spec(&session.canvas.spec.pigment_slots))
            .then(|| capture_canvas(gpu, &session.tray))
            .transpose()?
            .map(|states| TraySnapshot {
                spec: session.tray.spec.clone(),
                states,
            });
        Ok(DocumentSnapshot {
            spec: session.canvas.spec.clone(),
            states,
            tray,
        })
    }

    pub fn into_session(self, gpu: &GpuContext) -> Result<PaintSession, CanvasError> {
        let mut session = PaintSession::new(gpu, self.spec.clone())?;
        self.upload(gpu, &mut session);
        Ok(session)
    }

    pub fn restore_into(
        self,
        gpu: &GpuContext,
        session: &mut PaintSession,
    ) -> Result<(), CanvasError> {
        session.replace_document(gpu, self.spec.clone())?;
        self.upload(gpu, session);
        Ok(())
    }

    fn upload(self, gpu: &GpuContext, session: &mut PaintSession) {
        let painting = TexelBlock {
            origin: [0, 0],
            size: [self.spec.width, self.spec.height],
        };
        upload_states_into(gpu, &mut session.canvas, self.states, painting);
        if let Some(tray) = self.tray {
            let middle = middle_block(&session.tray.spec, &tray.spec);
            let mut states = tray.states;
            on_todays_tray_ground(&mut states);
            upload_states_into(gpu, &mut session.tray, states, middle);
        }
    }

    fn holds_graphite(&self) -> bool {
        self.states
            .iter()
            .filter(|state| state.which == StateTexture::Graphite)
            .any(|state| state.texels().iter().any(|byte| *byte != 0))
    }

    fn file_format(&self) -> u32 {
        // Older builds would open a sketch and drop it, so only the current format carries one.
        if self.holds_graphite() {
            return FORMAT_VERSION;
        }
        match &self.tray {
            // Read from format 3 and written without being opened: keeps its old tray and format.
            Some(tray) if [tray.spec.width, tray.spec.height] == LEGACY_TRAY => TRAY_FORMAT,
            Some(_) => FORMAT_VERSION,
            None => TRAYLESS_FORMAT,
        }
    }
}

fn capture_canvas(gpu: &GpuContext, canvas: &Canvas) -> Result<Vec<StateBytes>, GpuLost> {
    let stagings: Vec<wgpu::Buffer> = StateTexture::PAINT_STATE
        .iter()
        .map(|which| {
            staging_buffer(
                gpu,
                "document_capture",
                canvas.texel_rows(*which).padded_size(),
            )
        })
        .collect();
    let mut encoder = new_encoder(gpu, "document_capture");
    for (which, staging) in StateTexture::PAINT_STATE.iter().zip(&stagings) {
        canvas.encode_copy_out(*which, staging, &mut encoder);
    }
    gpu.queue.submit([encoder.finish()]);
    let stagings: Vec<&wgpu::Buffer> = stagings.iter().collect();
    read_all_mapped(gpu, &stagings, |index, padded| {
        let which = StateTexture::PAINT_STATE[index];
        StateBytes {
            which,
            rows: canvas.texel_rows(which),
            bytes: padded.to_vec(),
        }
    })
}

/// Moves a saved tray's bare ground to today's (see `tray_ground`).
fn on_todays_tray_ground(states: &mut [StateBytes]) {
    let mut low = None;
    let mut high = None;
    for state in states.iter_mut() {
        match state.which {
            StateTexture::DryRefl0 => low = Some(&mut state.bytes),
            StateTexture::DryRefl1 => high = Some(&mut state.bytes),
            _ => {}
        }
    }
    if let (Some(low), Some(high)) = (low, high) {
        tray_ground::rebase_tray_ground(low, high, TRAY_REFLECTANCE);
    }
}

/// The block a `saved` tray takes in the middle of a `full` one.
fn middle_block(full: &CanvasSpec, saved: &CanvasSpec) -> TexelBlock {
    let margin = |full: u32, saved: u32| {
        full.checked_sub(saved)
            .expect("the saved tray fits the tray")
            / 2
    };
    TexelBlock {
        origin: [
            margin(full.width, saved.width),
            margin(full.height, saved.height),
        ],
        size: [saved.width, saved.height],
    }
}

/// Writes `states` into `block` of `canvas`; the rest of the canvas keeps what it had.
fn upload_states_into(
    gpu: &GpuContext,
    canvas: &mut Canvas,
    states: Vec<StateBytes>,
    block: TexelBlock,
) {
    for state in states {
        let (which, raw) = (state.which, state.texels());
        canvas.write_bytes_at(gpu, which, &raw, block);
        gpu.queue.submit([]);
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        if which == StateTexture::WetVol {
            mark_wet_tiles(canvas, &raw, block);
        }
    }
    canvas.dirty.mark_all();
}

pub fn save(
    gpu: &GpuContext,
    session: &mut PaintSession,
    path: &Path,
) -> Result<(), DocumentError> {
    DocumentSnapshot::capture(gpu, session)?.write(path)
}

pub fn open(gpu: &GpuContext, path: &Path) -> Result<PaintSession, DocumentError> {
    Ok(DocumentSnapshot::read(path)?.into_session(gpu)?)
}

pub fn export_png(
    gpu: &GpuContext,
    session: &mut PaintSession,
    path: &Path,
) -> Result<(), DocumentError> {
    ExportedImage::render(gpu, session)?.write_png(path)
}

fn mark_wet_tiles(canvas: &mut Canvas, wet_volume: &[u8], block: TexelBlock) {
    let (left, top) = (block.origin[0] as usize, block.origin[1] as usize);
    let block_width = block.size[0] as usize;
    let tile = TILE_SIZE as usize;
    let columns = (canvas.spec.width as usize).div_ceil(tile);
    let rows = (canvas.spec.height as usize).div_ceil(tile);
    let mut wet = vec![false; columns * rows];
    for (index, texel) in wet_volume
        .as_chunks::<THICKNESS_BYTES>()
        .0
        .iter()
        .enumerate()
    {
        if f32::from_le_bytes(*texel) > 0.0 {
            let (x, y) = (left + index % block_width, top + index / block_width);
            wet[(y / tile) * columns + x / tile] = true;
        }
    }
    for (index, _) in wet.iter().enumerate().filter(|(_, wet)| **wet) {
        let x0 = ((index % columns) * tile) as f32;
        let y0 = ((index / columns) * tile) as f32;
        canvas.wet_tiles.mark_rect(x0, y0, x0, y0);
    }
}

fn state_entry(prefix: &str, which: StateTexture, format: u32) -> String {
    let name = if which == StateTexture::DryHeight && format > ABSOLUTE_DRY_HEIGHT_FORMAT {
        DRY_RELIEF_NAME
    } else {
        which.label()
    };
    format!("{prefix}state/{name}.zst")
}
