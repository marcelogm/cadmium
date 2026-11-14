use std::io::Write;
use std::path::Path;

use cadmium_engine::{
    canvas::{CanvasSpec, Primer, StateTexture, Weave},
    document::{DocumentError, DocumentSnapshot},
    pigments::PigmentDb,
};

use crate::{
    BLANK_HEIGHT, BLANK_WIDTH, DRY_RELIEF, FORMAT_1_DRY_HEIGHT, FORMAT_1_STATE_ENTRIES,
    GRAPHITE_ENTRY, TempDir,
};

pub(super) struct Entries(pub(super) Vec<(String, Vec<u8>)>);

impl Entries {
    pub(super) fn blank() -> Entries {
        let spec = CanvasSpec {
            width: BLANK_WIDTH,
            height: BLANK_HEIGHT,
            primer: Primer::WhiteGesso,
            weave: Weave::Smooth,
            pigment_slots: vec!["titanium_white".into()],
        };
        let manifest = serde_json::json!({
            "format": 1,
            "pigment_db": PigmentDb::VERSION,
            "spec": spec,
            "bands": 8,
            "units": "um",
        });
        let mut entries = vec![(
            "manifest.json".to_string(),
            serde_json::to_vec(&manifest).unwrap(),
        )];
        for (name, which) in FORMAT_1_STATE_ENTRIES {
            let raw = vec![0u8; (BLANK_WIDTH * BLANK_HEIGHT * which.bytes_per_texel()) as usize];
            entries.push((
                name.to_string(),
                zstd::encode_all(raw.as_slice(), 3).unwrap(),
            ));
        }
        Entries(entries)
    }

    fn bytes(&mut self, name: &str) -> &mut Vec<u8> {
        &mut self
            .0
            .iter_mut()
            .find(|(entry, _)| entry == name)
            .unwrap()
            .1
    }

    pub(super) fn edit_manifest(mut self, edit: impl FnOnce(&mut serde_json::Value)) -> Entries {
        let bytes = self.bytes("manifest.json");
        let mut manifest: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        edit(&mut manifest);
        *bytes = serde_json::to_vec(&manifest).unwrap();
        self
    }

    /// Adds a graphite entry, all zero, which `edit_state` can then change.
    pub(super) fn with_graphite(mut self) -> Entries {
        let texel = StateTexture::Graphite.bytes_per_texel();
        let raw = vec![0u8; (BLANK_WIDTH * BLANK_HEIGHT * texel) as usize];
        self.0.push((
            GRAPHITE_ENTRY.to_string(),
            zstd::encode_all(raw.as_slice(), 3).unwrap(),
        ));
        self
    }

    pub(super) fn edit_state(mut self, name: &str, edit: impl FnOnce(&mut Vec<u8>)) -> Entries {
        let bytes = self.bytes(name);
        let mut raw = zstd::decode_all(bytes.as_slice()).unwrap();
        edit(&mut raw);
        *bytes = zstd::encode_all(raw.as_slice(), 3).unwrap();
        self
    }

    pub(super) fn in_format_2(mut self) -> Entries {
        for (entry, _) in self
            .0
            .iter_mut()
            .filter(|(entry, _)| entry == FORMAT_1_DRY_HEIGHT)
        {
            *entry = DRY_RELIEF.to_string();
        }
        self.edit_manifest(|m| m["format"] = 2.into())
    }

    pub(super) fn without(mut self, name: &str) -> Entries {
        self.0.retain(|(entry, _)| entry != name);
        self
    }

    fn write(&self, path: &Path) {
        let mut writer = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, bytes) in &self.0 {
            writer.start_file(name.as_str(), options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
    }
}

pub(super) fn read_entries(entries: Entries) -> Result<DocumentSnapshot, DocumentError> {
    let dir = TempDir::new("read");
    let path = dir.file("painting.cadmium");
    entries.write(&path);
    DocumentSnapshot::read(&path)
}

pub(super) fn format_reason(result: Result<DocumentSnapshot, DocumentError>) -> String {
    match result {
        Err(DocumentError::Format(reason)) => reason,
        Err(other) => panic!("expected a format error, got {other:?}"),
        Ok(_) => panic!("expected a format error, the document was accepted"),
    }
}

pub(super) fn set_first_f32(value: f32) -> impl FnOnce(&mut Vec<u8>) {
    move |raw| raw[..4].copy_from_slice(&value.to_le_bytes())
}
