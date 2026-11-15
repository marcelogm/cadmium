use std::borrow::Cow;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::canvas::{CanvasSpec, StateTexture};
use crate::pigments::PigmentDb;
use crate::sheet::sheet_spec;
use crate::spectral::BANDS;

use super::relief::dry_relief;
use super::{
    DocumentError, DocumentSnapshot, FORMAT_VERSION, MANIFEST_ENTRY, Manifest, PAINTING_PREFIX,
    SHEET_PREFIX, StateBytes, THICKNESS_UNITS, state_entry,
};

const ZSTD_LEVEL: i32 = 3;
const PARTIAL_SUFFIX: &str = ".part";

impl DocumentSnapshot {
    pub fn write(&self, path: &Path) -> Result<(), DocumentError> {
        self.check_sheet()?;
        let manifest = Manifest {
            format: FORMAT_VERSION,
            pigment_db: PigmentDb::VERSION,
            spec: self.spec.clone(),
            bands: BANDS,
            units: THICKNESS_UNITS.to_string(),
            palette: self.palette.clone(),
        };
        let manifest = serde_json::to_vec_pretty(&manifest)
            .map_err(|error| DocumentError::Format(error.to_string()))?;
        write_replacing(path, |file| self.write_archive(file, &manifest))
    }

    fn write_archive(
        &self,
        file: &mut BufWriter<File>,
        manifest: &[u8],
    ) -> Result<(), DocumentError> {
        let mut archive = zip::ZipWriter::new(file);
        archive.start_file(MANIFEST_ENTRY, stored_entry())?;
        archive.write_all(manifest)?;
        write_states(&mut archive, PAINTING_PREFIX, &self.spec, &self.states)?;
        write_states(
            &mut archive,
            SHEET_PREFIX,
            &self.sheet_spec,
            &self.sheet_states,
        )?;
        archive.finish()?;
        Ok(())
    }
}

impl DocumentSnapshot {
    fn check_sheet(&self) -> Result<(), DocumentError> {
        if self.sheet_spec == sheet_spec(&self.spec.pigment_slots) {
            return Ok(());
        }
        Err(DocumentError::Format(format!(
            "the mixing sheet ({}×{}, paints {:?}) is not the sheet of the painting (paints {:?})",
            self.sheet_spec.width,
            self.sheet_spec.height,
            self.sheet_spec.pigment_slots,
            self.spec.pigment_slots
        )))
    }
}

fn stored_entry() -> zip::write::SimpleFileOptions {
    zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored)
}

fn write_states<W: Write + std::io::Seek>(
    archive: &mut zip::ZipWriter<W>,
    prefix: &str,
    spec: &CanvasSpec,
    states: &[StateBytes],
) -> Result<(), DocumentError> {
    for state in states {
        let name = state_entry(prefix, state.which);
        if state.rows.row_bytes != spec.width * state.which.bytes_per_texel()
            || state.rows.rows != spec.height
        {
            return Err(DocumentError::Format(format!(
                "{name} does not match the {}×{} canvas",
                spec.width, spec.height
            )));
        }
        let raw = state.texels();
        let stored = if state.which == StateTexture::DryHeight {
            Cow::Owned(dry_relief(spec, &raw))
        } else {
            raw
        };
        let compressed = zstd::encode_all(stored.as_ref(), ZSTD_LEVEL)?;
        archive.start_file(name, stored_entry())?;
        archive.write_all(&compressed)?;
    }
    Ok(())
}

pub(super) fn write_replacing(
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
