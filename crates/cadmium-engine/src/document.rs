use std::borrow::Cow;
use std::fmt;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use crate::canvas::{Canvas, CanvasError, CanvasSpec, StateTexture, TILE_SIZE, weave_heights};
use crate::gpu::{GpuContext, GpuLost, TexelRows, read_all_mapped, staging_buffer};
use crate::pigments::PigmentDb;
use crate::present::{ViewTransform, readback_rgba8};
use crate::render::ViewMode;
use crate::session::PaintSession;
use crate::spectral::BANDS;
use crate::surface::tray_spec;

const FORMAT_VERSION: u32 = 3;
const ABSOLUTE_DRY_HEIGHT_FORMAT: u32 = 1;
const TRAYLESS_FORMAT: u32 = 2;
const TRAY_FORMAT: u32 = 3;
const PAINTING_PREFIX: &str = "";
const TRAY_PREFIX: &str = "tray/";
const OLDEST_READABLE_FORMAT: u32 = ABSOLUTE_DRY_HEIGHT_FORMAT;
const DRY_RELIEF_NAME: &str = "dry_relief";
const THICKNESS_UNITS: &str = "um";
const MANIFEST_ENTRY: &str = "manifest.json";
const MAX_MANIFEST_BYTES: u64 = 1 << 20;
const ZSTD_LEVEL: i32 = 3;
const PARTIAL_SUFFIX: &str = ".part";
const RGBA_CHANNELS: usize = 4;
const ALPHA_CHANNEL: usize = 3;
const OPAQUE_ALPHA: u8 = 255;
const THICKNESS_BYTES: usize = 4;
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

#[derive(serde::Deserialize)]
struct FormatProbe {
    format: u32,
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
    tray: Vec<StateBytes>,
}

impl DocumentSnapshot {
    pub fn capture(
        gpu: &GpuContext,
        session: &mut PaintSession,
    ) -> Result<DocumentSnapshot, GpuLost> {
        session.flush(gpu);
        Ok(DocumentSnapshot {
            spec: session.canvas.spec.clone(),
            states: capture_canvas(gpu, &session.canvas)?,
            tray: capture_canvas(gpu, &session.tray)?,
        })
    }

    pub fn read(path: &Path) -> Result<DocumentSnapshot, DocumentError> {
        let mut archive = zip::ZipArchive::new(BufReader::new(File::open(path)?))?;
        let (format, spec) = read_manifest(&mut archive)?;
        let states = read_states(&mut archive, format, PAINTING_PREFIX, &spec)?;
        let tray = if format >= TRAY_FORMAT {
            read_states(
                &mut archive,
                format,
                TRAY_PREFIX,
                &tray_spec(&spec.pigment_slots),
            )?
        } else {
            Vec::new()
        };
        Ok(DocumentSnapshot { spec, states, tray })
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
        if !self.tray.is_empty() {
            upload_states(gpu, &mut session.tray, self.tray);
        }
    }

    fn format(&self) -> u32 {
        if self.tray.is_empty() {
            TRAYLESS_FORMAT
        } else {
            FORMAT_VERSION
        }
    }

    pub fn write(&self, path: &Path) -> Result<(), DocumentError> {
        let format = self.format();
        let manifest = Manifest {
            format,
            pigment_db: PigmentDb::VERSION,
            spec: self.spec.clone(),
            bands: BANDS,
            units: THICKNESS_UNITS.to_string(),
        };
        let manifest = serde_json::to_vec_pretty(&manifest)
            .map_err(|error| DocumentError::Format(error.to_string()))?;
        let tray_spec = tray_spec(&self.spec.pigment_slots);
        write_replacing(path, |file| {
            let mut archive = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            archive.start_file(MANIFEST_ENTRY, options)?;
            archive.write_all(&manifest)?;
            write_states(
                &mut archive,
                options,
                format,
                PAINTING_PREFIX,
                &self.spec,
                &self.states,
            )?;
            write_states(
                &mut archive,
                options,
                format,
                TRAY_PREFIX,
                &tray_spec,
                &self.tray,
            )?;
            archive.finish()?;
            Ok(())
        })
    }
}

fn write_states<W: Write + std::io::Seek>(
    archive: &mut zip::ZipWriter<W>,
    options: zip::write::SimpleFileOptions,
    format: u32,
    prefix: &str,
    spec: &CanvasSpec,
    states: &[StateBytes],
) -> Result<(), DocumentError> {
    for state in states {
        let raw = state.texels();
        let stored = if state.which == StateTexture::DryHeight {
            Cow::Owned(dry_relief(spec, &raw))
        } else {
            raw
        };
        let compressed = zstd::encode_all(stored.as_ref(), ZSTD_LEVEL)?;
        archive.start_file(state_entry(prefix, state.which, format), options)?;
        archive.write_all(&compressed)?;
    }
    Ok(())
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

pub struct ExportedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl ExportedImage {
    pub fn render(gpu: &GpuContext, session: &mut PaintSession) -> Result<ExportedImage, GpuLost> {
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
        let rgba = readback_rgba8(gpu, &target);
        session.view_mode = chosen_mode;
        if chosen_mode != ViewMode::Color {
            session.canvas.dirty.mark_all();
        }
        let mut rgba = rgba?;
        for pixel in rgba.as_chunks_mut::<RGBA_CHANNELS>().0 {
            pixel[ALPHA_CHANNEL] = OPAQUE_ALPHA;
        }
        Ok(ExportedImage {
            width,
            height,
            rgba,
        })
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

fn read_manifest<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<(u32, CanvasSpec), DocumentError> {
    let mut bytes = Vec::new();
    entry(archive, MANIFEST_ENTRY)?
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(DocumentError::Format(format!(
            "{MANIFEST_ENTRY} is too large, the limit is {MAX_MANIFEST_BYTES} bytes"
        )));
    }
    let probe: FormatProbe = serde_json::from_slice(&bytes).map_err(manifest_error)?;
    if !(OLDEST_READABLE_FORMAT..=FORMAT_VERSION).contains(&probe.format) {
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
    Ok((manifest.format, manifest.spec))
}

fn read_states<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    format: u32,
    prefix: &str,
    spec: &CanvasSpec,
) -> Result<Vec<StateBytes>, DocumentError> {
    StateTexture::PAINT_STATE
        .iter()
        .map(|which| {
            Ok(StateBytes {
                which: *which,
                rows: TexelRows::unpadded(spec.width * which.bytes_per_texel(), spec.height),
                bytes: read_state(archive, format, prefix, spec, *which)?,
            })
        })
        .collect()
}

fn read_state<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    format: u32,
    prefix: &str,
    spec: &CanvasSpec,
    which: StateTexture,
) -> Result<Vec<u8>, DocumentError> {
    let name = state_entry(prefix, which, format);
    let expected = spec.width as usize * spec.height as usize * which.bytes_per_texel() as usize;
    let entry = entry(archive, &name)?;
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
    let raw = if which == StateTexture::DryHeight && format > ABSOLUTE_DRY_HEIGHT_FORMAT {
        dry_height_over_weave(spec, &raw)
    } else {
        raw
    };
    if holds_thickness(which) && !thicknesses_are_valid(&raw) {
        return Err(DocumentError::Format(format!(
            "{name} holds a negative or non-finite thickness"
        )));
    }
    Ok(raw)
}

fn dry_relief(spec: &CanvasSpec, dry_height: &[u8]) -> Vec<u8> {
    let weave = weave_heights(spec.weave, spec.width, spec.height);
    combine_thicknesses(dry_height, &weave, |height, weave| height - weave)
}

fn dry_height_over_weave(spec: &CanvasSpec, relief: &[u8]) -> Vec<u8> {
    let weave = weave_heights(spec.weave, spec.width, spec.height);
    combine_thicknesses(relief, &weave, |relief, weave| relief + weave)
}

fn combine_thicknesses(raw: &[u8], weave: &[f32], combine: impl Fn(f32, f32) -> f32) -> Vec<u8> {
    raw.as_chunks::<THICKNESS_BYTES>()
        .0
        .iter()
        .zip(weave)
        .flat_map(|(texel, weave)| combine(f32::from_le_bytes(*texel), *weave).to_le_bytes())
        .collect()
}

fn holds_thickness(which: StateTexture) -> bool {
    matches!(which, StateTexture::WetVol | StateTexture::DryHeight)
}

fn thicknesses_are_valid(raw: &[u8]) -> bool {
    raw.as_chunks::<THICKNESS_BYTES>()
        .0
        .iter()
        .map(|texel| f32::from_le_bytes(*texel))
        .all(|thickness| thickness.is_finite() && thickness >= 0.0)
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

fn entry<'a, R: Read + std::io::Seek>(
    archive: &'a mut zip::ZipArchive<R>,
    name: &str,
) -> Result<zip::read::ZipFile<'a, R>, DocumentError> {
    archive.by_name(name).map_err(|error| match error {
        zip::result::ZipError::FileNotFound => DocumentError::Format(format!("missing {name}")),
        other => other.into(),
    })
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
            File::open(parent_dir(path))?.sync_all()?;
            Ok(())
        });
    if result.is_err() {
        let _ = std::fs::remove_file(&partial);
    }
    result
}

fn parent_dir(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
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
