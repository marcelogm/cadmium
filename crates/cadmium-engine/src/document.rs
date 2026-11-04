use std::borrow::Cow;
use std::fmt;
use std::path::Path;

use crate::canvas::{Canvas, CanvasError, CanvasSpec, StateTexture, TILE_SIZE};
use crate::gpu::{GpuContext, GpuLost, TexelRows, read_all_mapped, staging_buffer};
use crate::session::PaintSession;
use crate::surface::tray_spec;

mod export;
mod read;
mod relief;
mod write;

pub use export::ExportedImage;

const FORMAT_VERSION: u32 = 3;
const ABSOLUTE_DRY_HEIGHT_FORMAT: u32 = 1;
const TRAYLESS_FORMAT: u32 = 2;
const PAINTING_PREFIX: &str = "";
const TRAY_PREFIX: &str = "tray/";
const DRY_RELIEF_NAME: &str = "dry_relief";
const THICKNESS_UNITS: &str = "um";
const MANIFEST_ENTRY: &str = "manifest.json";
const THICKNESS_BYTES: usize = 4;

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

pub struct DocumentSnapshot {
    spec: CanvasSpec,
    states: Vec<StateBytes>,
    tray: Option<Vec<StateBytes>>,
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
            .transpose()?;
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
        upload_states(gpu, &mut session.canvas, self.states);
        if let Some(tray) = self.tray {
            upload_states(gpu, &mut session.tray, tray);
        }
    }

    fn file_format(&self) -> u32 {
        match self.tray {
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
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("document_capture"),
        });
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

fn mark_wet_tiles(canvas: &mut Canvas, wet_volume: &[u8]) {
    let width = canvas.spec.width as usize;
    let tile = TILE_SIZE as usize;
    let columns = width.div_ceil(tile);
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

fn state_entry(prefix: &str, which: StateTexture, format: u32) -> String {
    let name = if which == StateTexture::DryHeight && format > ABSOLUTE_DRY_HEIGHT_FORMAT {
        DRY_RELIEF_NAME
    } else {
        which.label()
    };
    format!("{prefix}state/{name}.zst")
}
