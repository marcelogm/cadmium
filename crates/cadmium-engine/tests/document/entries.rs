use std::io::{Read, Write};
use std::path::Path;

use cadmium_engine::{
    canvas::{CanvasSpec, Primer, StateTexture, Weave},
    document::{self, DocumentError, DocumentSnapshot},
    gpu::GpuContext,
    pigments::PigmentDb,
};

use crate::{BLANK_HEIGHT, BLANK_WIDTH, CURRENT_FORMAT, DRY_RELIEF, GRAPHITE_ENTRY, TempDir};

/// The entry names of format 7, spelled out so that renaming one changes a test.
pub(super) const STATE_ENTRIES: [(&str, StateTexture); 9] = [
    ("state/wet_pig0.zst", StateTexture::WetPig0),
    ("state/wet_pig1.zst", StateTexture::WetPig1),
    ("state/wet_vol.zst", StateTexture::WetVol),
    ("state/dry_refl0.zst", StateTexture::DryRefl0),
    ("state/dry_refl1.zst", StateTexture::DryRefl1),
    (DRY_RELIEF, StateTexture::DryHeight),
    ("state/dry_top_pig0.zst", StateTexture::DryTopPig0),
    ("state/dry_top_pig1.zst", StateTexture::DryTopPig1),
    (GRAPHITE_ENTRY, StateTexture::Graphite),
];

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
            "format": CURRENT_FORMAT,
            "pigment_db": PigmentDb::VERSION,
            "spec": spec,
            "bands": 8,
            "units": "um",
            "palette": { "my_colours": [], "circle": [] },
        });
        let mut entries = vec![(
            "manifest.json".to_string(),
            serde_json::to_vec(&manifest).unwrap(),
        )];
        for (name, which) in STATE_ENTRIES {
            let raw = vec![0u8; (BLANK_WIDTH * BLANK_HEIGHT * which.bytes_per_texel()) as usize];
            entries.push((
                name.to_string(),
                zstd::encode_all(raw.as_slice(), 3).unwrap(),
            ));
        }
        Entries(entries)
    }

    pub(super) fn saved(path: &Path) -> Entries {
        let mut archive = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
        let entries = (0..archive.len())
            .map(|index| {
                let mut entry = archive.by_index(index).unwrap();
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes).unwrap();
                (entry.name().to_string(), bytes)
            })
            .collect();
        Entries(entries)
    }

    pub(super) fn names(&self) -> Vec<String> {
        self.0.iter().map(|(name, _)| name.clone()).collect()
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

    pub(super) fn edit_state(mut self, name: &str, edit: impl FnOnce(&mut Vec<u8>)) -> Entries {
        let bytes = self.bytes(name);
        let mut raw = zstd::decode_all(bytes.as_slice()).unwrap();
        edit(&mut raw);
        *bytes = zstd::encode_all(raw.as_slice(), 3).unwrap();
        self
    }

    pub(super) fn with(mut self, name: &str, bytes: Vec<u8>) -> Entries {
        self.0.push((name.to_string(), bytes));
        self
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

#[test]
fn a_saved_painting_holds_its_manifest_and_the_painting_state_only() {
    let gpu = GpuContext::new().unwrap();
    let dir = TempDir::new("entries");
    let mut session = crate::painted_session(&gpu);
    document::save(&gpu, &mut session, &dir.file("p.cadmium")).unwrap();
    let names = Entries::saved(&dir.file("p.cadmium")).names();
    assert!(names.iter().any(|name| name == "manifest.json"));
    assert!(names.iter().any(|name| name == GRAPHITE_ENTRY));
    assert!(
        names
            .iter()
            .all(|name| name == "manifest.json" || name.starts_with("state/"))
    );
}
