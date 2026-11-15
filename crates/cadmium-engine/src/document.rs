use std::borrow::Cow;
use std::fmt;
use std::path::Path;

use crate::canvas::{Canvas, CanvasError, CanvasSpec, StateTexture, TILE_SIZE};
use crate::gpu::{GpuContext, GpuLost, TexelRows, new_encoder, read_all_mapped, staging_buffer};
use crate::palette::Palette;
use crate::session::PaintSession;
use crate::sheet::sheet_spec;

mod export;
mod read;
mod relief;
mod write;

pub use export::ExportedImage;

const FORMAT_VERSION: u32 = 8;
const PAINTING_PREFIX: &str = "";
const SHEET_PREFIX: &str = "sheet/";
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
                "painting uses file format {version}, this version of Cadmium reads only format {FORMAT_VERSION}"
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
    palette: Palette,
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

pub struct DocumentSnapshot {
    spec: CanvasSpec,
    palette: Palette,
    states: Vec<StateBytes>,
    sheet_spec: CanvasSpec,
    sheet_states: Vec<StateBytes>,
}

pub struct OpenedPainting {
    pub painting: PaintSession,
    pub sheet: PaintSession,
}

impl DocumentSnapshot {
    pub fn capture(
        gpu: &GpuContext,
        painting: &mut PaintSession,
        sheet: &mut PaintSession,
    ) -> Result<DocumentSnapshot, GpuLost> {
        painting.flush(gpu);
        sheet.flush(gpu);
        Ok(DocumentSnapshot {
            spec: painting.canvas.spec.clone(),
            palette: painting.palette.clone(),
            states: capture_canvas(gpu, &painting.canvas)?,
            sheet_spec: sheet.canvas.spec.clone(),
            sheet_states: capture_canvas(gpu, &sheet.canvas)?,
        })
    }

    pub fn into_sessions(self, gpu: &GpuContext) -> Result<OpenedPainting, CanvasError> {
        let mut painting = PaintSession::new(gpu, self.spec.clone())?;
        let mut sheet = PaintSession::for_sheet(gpu, &self.spec.pigment_slots)?;
        self.upload(gpu, &mut painting, &mut sheet);
        Ok(OpenedPainting { painting, sheet })
    }

    pub fn restore_into(
        self,
        gpu: &GpuContext,
        painting: &mut PaintSession,
        sheet: &mut PaintSession,
    ) -> Result<(), CanvasError> {
        sheet.replace_document(gpu, sheet_spec(&self.spec.pigment_slots))?;
        painting.replace_document(gpu, self.spec.clone())?;
        self.upload(gpu, painting, sheet);
        Ok(())
    }

    fn upload(self, gpu: &GpuContext, painting: &mut PaintSession, sheet: &mut PaintSession) {
        painting.palette = self.palette;
        upload_states(gpu, &mut painting.canvas, self.states);
        upload_states(gpu, &mut sheet.canvas, self.sheet_states);
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

fn upload_states(gpu: &GpuContext, canvas: &mut Canvas, states: Vec<StateBytes>) {
    for state in states {
        let (which, raw) = (state.which, state.texels());
        canvas.write_bytes(gpu, which, &raw);
        gpu.queue.submit([]);
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        if which == StateTexture::WetVol {
            mark_wet_tiles(canvas, &raw);
        }
    }
    canvas.dirty.mark_all();
    canvas.margin_touched.mark_all();
}

pub fn save(
    gpu: &GpuContext,
    painting: &mut PaintSession,
    sheet: &mut PaintSession,
    path: &Path,
) -> Result<(), DocumentError> {
    DocumentSnapshot::capture(gpu, painting, sheet)?.write(path)
}

pub fn open(gpu: &GpuContext, path: &Path) -> Result<OpenedPainting, DocumentError> {
    Ok(DocumentSnapshot::read(path)?.into_sessions(gpu)?)
}

pub fn export_png(
    gpu: &GpuContext,
    session: &mut PaintSession,
    path: &Path,
) -> Result<(), DocumentError> {
    ExportedImage::render(gpu, session)?.write_png(path)
}

fn mark_wet_tiles(canvas: &mut Canvas, wet_volume: &[u8]) {
    let width = canvas.spec.width as usize;
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
            let (x, y) = (index % width, index / width);
            wet[(y / tile) * columns + x / tile] = true;
        }
    }
    for (index, _) in wet.iter().enumerate().filter(|(_, wet)| **wet) {
        let x0 = ((index % columns) * tile) as f32;
        let y0 = ((index / columns) * tile) as f32;
        canvas.wet_tiles.mark_rect(x0, y0, x0, y0);
    }
}

fn state_entry(prefix: &str, which: StateTexture) -> String {
    let name = if which == StateTexture::DryHeight {
        DRY_RELIEF_NAME
    } else {
        which.label()
    };
    format!("{prefix}state/{name}.zst")
}
