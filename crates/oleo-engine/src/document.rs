use std::fmt;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use crate::canvas::{Canvas, CanvasSpec, StateTexture, TILE_SIZE};
use crate::gpu::GpuContext;
use crate::pigments::PigmentDb;
use crate::present::{ViewTransform, readback_rgba8};
use crate::render::ViewMode;
use crate::session::PaintSession;
use crate::spectral::BANDS;

const FORMAT_VERSION: u32 = 1;
const THICKNESS_UNITS: &str = "um";
const MANIFEST_ENTRY: &str = "manifest.json";
const MAX_MANIFEST_BYTES: u64 = 1 << 20;
const ZSTD_LEVEL: i32 = 3;
const PARTIAL_SUFFIX: &str = ".part";
const RGBA_CHANNELS: usize = 4;
const ALPHA_CHANNEL: usize = 3;
const OPAQUE_ALPHA: u8 = 255;
const WET_VOLUME_BYTES: usize = 4;
const UNSCALED_VIEW: ViewTransform = ViewTransform {
    origin_x: 0.0,
    origin_y: 0.0,
    zoom: 1.0,
};

#[derive(Debug)]
pub enum DocumentError {
    Io(std::io::Error),
    Format(String),
    UnsupportedVersion(u32),
}

impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocumentError::Io(error) => write!(f, "{error}"),
            DocumentError::Format(reason) => write!(f, "not a valid Óleo painting: {reason}"),
            DocumentError::UnsupportedVersion(version) => write!(
                f,
                "painting uses file format {version}, this version of Óleo reads format {FORMAT_VERSION}"
            ),
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

#[derive(serde::Deserialize)]
struct FormatProbe {
    format: u32,
}

pub struct DocumentSnapshot {
    spec: CanvasSpec,
    states: Vec<(StateTexture, Vec<u8>)>,
}

impl DocumentSnapshot {
    pub fn capture(gpu: &GpuContext, session: &PaintSession) -> DocumentSnapshot {
        DocumentSnapshot {
            spec: session.canvas.spec.clone(),
            states: StateTexture::PAINT_STATE
                .iter()
                .map(|which| (*which, session.canvas.read_bytes(gpu, *which)))
                .collect(),
        }
    }

    pub fn write(&self, path: &Path) -> Result<(), DocumentError> {
        let manifest = Manifest {
            format: FORMAT_VERSION,
            pigment_db: PigmentDb::VERSION,
            spec: self.spec.clone(),
            bands: BANDS,
            units: THICKNESS_UNITS.to_string(),
        };
        let manifest = serde_json::to_vec_pretty(&manifest)
            .map_err(|error| DocumentError::Format(error.to_string()))?;
        write_replacing(path, |file| {
            let mut archive = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            archive.start_file(MANIFEST_ENTRY, options)?;
            archive.write_all(&manifest)?;
            for (which, raw) in &self.states {
                let compressed = zstd::encode_all(raw.as_slice(), ZSTD_LEVEL)?;
                archive.start_file(state_entry(*which), options)?;
                archive.write_all(&compressed)?;
            }
            archive.finish()?;
            Ok(())
        })
    }
}

pub struct ExportedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl ExportedImage {
    pub fn render(gpu: &GpuContext, session: &mut PaintSession) -> ExportedImage {
        let (width, height) = (session.canvas.spec.width, session.canvas.spec.height);
        let chosen_mode = session.view_mode;
        session.view_mode = ViewMode::Color;
        session.canvas.dirty.mark_all();
        let target = session.presenter.create_target(gpu, width, height);
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("export_png"),
            });
        session.encode_frame(gpu, &mut encoder);
        session.presenter.encode_present(
            gpu,
            &session.canvas,
            UNSCALED_VIEW,
            &target,
            &mut encoder,
        );
        gpu.queue.submit([encoder.finish()]);
        let mut rgba = readback_rgba8(gpu, &target);
        session.view_mode = chosen_mode;
        if chosen_mode != ViewMode::Color {
            session.canvas.dirty.mark_all();
        }
        for pixel in rgba.as_chunks_mut::<RGBA_CHANNELS>().0 {
            pixel[ALPHA_CHANNEL] = OPAQUE_ALPHA;
        }
        ExportedImage {
            width,
            height,
            rgba,
        }
    }

    pub fn write_png(&self, path: &Path) -> Result<(), DocumentError> {
        write_replacing(path, |file| {
            let mut encoder = png::Encoder::new(file, self.width, self.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
            let mut writer = encoder.write_header()?;
            writer.write_image_data(&self.rgba)?;
            writer.finish()?;
            Ok(())
        })
    }
}

pub fn save(gpu: &GpuContext, session: &PaintSession, path: &Path) -> Result<(), DocumentError> {
    DocumentSnapshot::capture(gpu, session).write(path)
}

pub fn open(gpu: &GpuContext, path: &Path) -> Result<PaintSession, DocumentError> {
    let mut archive = zip::ZipArchive::new(BufReader::new(File::open(path)?))?;
    let spec = read_manifest(&mut archive)?;
    let mut session = PaintSession::new(gpu, spec);
    for which in StateTexture::PAINT_STATE {
        let raw = read_state(&mut archive, &session.canvas.spec, which)?;
        session.canvas.write_bytes(gpu, which, &raw);
        if which == StateTexture::WetVol {
            mark_wet_tiles(&mut session.canvas, &raw);
        }
    }
    session.canvas.dirty.mark_all();
    Ok(session)
}

pub fn export_png(
    gpu: &GpuContext,
    session: &mut PaintSession,
    path: &Path,
) -> Result<(), DocumentError> {
    ExportedImage::render(gpu, session).write_png(path)
}

fn read_manifest<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<CanvasSpec, DocumentError> {
    let mut bytes = Vec::new();
    archive
        .by_name(MANIFEST_ENTRY)
        .map_err(|_| missing_entry(MANIFEST_ENTRY))?
        .take(MAX_MANIFEST_BYTES)
        .read_to_end(&mut bytes)?;
    let probe: FormatProbe = serde_json::from_slice(&bytes).map_err(manifest_error)?;
    if probe.format != FORMAT_VERSION {
        return Err(DocumentError::UnsupportedVersion(probe.format));
    }
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(manifest_error)?;
    if manifest.pigment_db != PigmentDb::VERSION {
        return Err(DocumentError::Format(format!(
            "pigment database version {} is not {}",
            manifest.pigment_db,
            PigmentDb::VERSION
        )));
    }
    if manifest.bands != BANDS {
        return Err(DocumentError::Format(format!(
            "{} spectral bands, expected {BANDS}",
            manifest.bands
        )));
    }
    if manifest.units != THICKNESS_UNITS {
        return Err(DocumentError::Format(format!(
            "thickness units {:?}, expected {THICKNESS_UNITS:?}",
            manifest.units
        )));
    }
    manifest
        .spec
        .validate(&PigmentDb::builtin())
        .map_err(DocumentError::Format)?;
    Ok(manifest.spec)
}

fn read_state<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    spec: &CanvasSpec,
    which: StateTexture,
) -> Result<Vec<u8>, DocumentError> {
    let name = state_entry(which);
    let expected = spec.width as usize * spec.height as usize * which.bytes_per_texel() as usize;
    let entry = archive.by_name(&name).map_err(|_| missing_entry(&name))?;
    let corrupt = |error: std::io::Error| DocumentError::Format(format!("{name}: {error}"));
    let mut raw = Vec::with_capacity(expected);
    zstd::stream::read::Decoder::new(entry)
        .map_err(corrupt)?
        .take(expected as u64 + 1)
        .read_to_end(&mut raw)
        .map_err(corrupt)?;
    if raw.len() != expected {
        return Err(DocumentError::Format(format!(
            "{name} holds {} bytes, expected {expected}",
            raw.len()
        )));
    }
    Ok(raw)
}

fn mark_wet_tiles(canvas: &mut Canvas, wet_volume: &[u8]) {
    let width = canvas.spec.width as usize;
    let tile = TILE_SIZE as usize;
    let columns = width.div_ceil(tile);
    let rows = (canvas.spec.height as usize).div_ceil(tile);
    let mut wet = vec![false; columns * rows];
    for (index, texel) in wet_volume
        .as_chunks::<WET_VOLUME_BYTES>()
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

fn state_entry(which: StateTexture) -> String {
    let name = match which {
        StateTexture::WetPig0 => "wet_pig0",
        StateTexture::WetPig1 => "wet_pig1",
        StateTexture::WetVol => "wet_vol",
        StateTexture::DryRefl0 => "dry_refl0",
        StateTexture::DryRefl1 => "dry_refl1",
        StateTexture::DryHeight => "dry_height",
        StateTexture::DryTopPig0 => "dry_top_pig0",
        StateTexture::DryTopPig1 => "dry_top_pig1",
        StateTexture::Display => "display",
    };
    format!("state/{name}.zst")
}

fn missing_entry(name: &str) -> DocumentError {
    DocumentError::Format(format!("missing {name}"))
}

fn manifest_error(error: serde_json::Error) -> DocumentError {
    DocumentError::Format(format!("{MANIFEST_ENTRY}: {error}"))
}

fn write_replacing(
    path: &Path,
    write: impl FnOnce(&mut BufWriter<File>) -> Result<(), DocumentError>,
) -> Result<(), DocumentError> {
    let partial = partial_path(path)?;
    let result = File::create(&partial)
        .map_err(DocumentError::from)
        .and_then(|file| {
            let mut file = BufWriter::new(file);
            write(&mut file)?;
            file.into_inner()
                .map_err(|error| error.into_error())?
                .sync_all()?;
            std::fs::rename(&partial, path)?;
            Ok(())
        });
    if result.is_err() {
        let _ = std::fs::remove_file(&partial);
    }
    result
}

fn partial_path(path: &Path) -> Result<PathBuf, DocumentError> {
    let Some(name) = path.file_name() else {
        return Err(DocumentError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{} does not name a file", path.display()),
        )));
    };
    let mut partial = name.to_os_string();
    partial.push(PARTIAL_SUFFIX);
    Ok(path.with_file_name(partial))
}
