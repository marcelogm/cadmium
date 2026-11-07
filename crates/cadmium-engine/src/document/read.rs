use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use crate::canvas::{CanvasSpec, StateTexture};
use crate::gpu::TexelRows;
use crate::pigments::PigmentDb;
use crate::spectral::BANDS;
use crate::surface::{legacy_tray_spec, tray_spec};

use super::relief::dry_height_over_weave;
use super::{
    ABSOLUTE_DRY_HEIGHT_FORMAT, DENSITY_BYTES, DocumentError, DocumentSnapshot, FORMAT_VERSION,
    MANIFEST_ENTRY, MAX_GRAPHITE_DENSITY, Manifest, PAINTING_PREFIX, StateBytes, THICKNESS_BYTES,
    THICKNESS_UNITS, TRAY_FORMAT, TRAY_PREFIX, TraySnapshot, state_entry,
};

const OLDEST_READABLE_FORMAT: u32 = ABSOLUTE_DRY_HEIGHT_FORMAT;
const MAX_MANIFEST_BYTES: u64 = 1 << 20;

#[derive(serde::Deserialize)]
struct FormatProbe {
    format: u32,
}

impl DocumentSnapshot {
    pub fn read(path: &Path) -> Result<DocumentSnapshot, DocumentError> {
        let mut archive = zip::ZipArchive::new(BufReader::new(File::open(path)?))?;
        let (format, spec) = read_manifest(&mut archive)?;
        let states = read_states(&mut archive, format, PAINTING_PREFIX, &spec)?;
        let tray = read_tray(&mut archive, format, &spec)?;
        Ok(DocumentSnapshot { spec, states, tray })
    }
}

fn read_tray<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    format: u32,
    spec: &CanvasSpec,
) -> Result<Option<TraySnapshot>, DocumentError> {
    let tray_spec = match format {
        ..TRAY_FORMAT => return Ok(None),
        TRAY_FORMAT => legacy_tray_spec(&spec.pigment_slots),
        _ => tray_spec(&spec.pigment_slots),
    };
    Ok(Some(TraySnapshot {
        states: read_states(archive, format, TRAY_PREFIX, &tray_spec)?,
        spec: tray_spec,
    }))
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
                bytes: read_optional_state(archive, format, prefix, spec, *which)?,
            })
        })
        .collect()
}

/// A painting from before the pencil has no graphite entry: it reads as an empty sketch.
fn read_optional_state<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    format: u32,
    prefix: &str,
    spec: &CanvasSpec,
    which: StateTexture,
) -> Result<Vec<u8>, DocumentError> {
    let name = state_entry(prefix, which, format);
    if which == StateTexture::Graphite && archive.index_for_name(&name).is_none() {
        let texels = spec.width as usize * spec.height as usize;
        return Ok(vec![0; texels * which.bytes_per_texel() as usize]);
    }
    read_state(archive, format, prefix, spec, which)
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
    if which == StateTexture::Graphite && !densities_are_valid(&raw) {
        return Err(DocumentError::Format(format!(
            "{name} holds a graphite density outside 0 to 1"
        )));
    }
    Ok(raw)
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

fn densities_are_valid(raw: &[u8]) -> bool {
    raw.as_chunks::<DENSITY_BYTES>()
        .0
        .iter()
        .map(|texel| f32::from_le_bytes(*texel))
        .all(|density| (0.0..=MAX_GRAPHITE_DENSITY).contains(&density))
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
