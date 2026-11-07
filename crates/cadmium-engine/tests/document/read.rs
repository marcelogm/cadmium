use cadmium_engine::{
    canvas::{MAX_CANVAS_SIZE, StateTexture},
    document::{DocumentError, DocumentSnapshot},
    gpu::GpuContext,
};

use crate::entries::{Entries, format_reason, read_entries, set_first_f32};
use crate::{
    CURRENT_FORMAT, DRY_RELIEF, FORMAT_1_DRY_HEIGHT, GRAPHITE_ENTRY, INVALID_THICKNESS, TempDir,
};

const FUTURE_FORMAT: u32 = CURRENT_FORMAT + 1;
const MAX_DENSITY: f32 = 1.0;
const MAX_MANIFEST_BYTES: usize = 1 << 20;

#[test]
fn reading_a_well_formed_document_needs_no_gpu() {
    assert!(read_entries(Entries::blank()).is_ok());
}

#[test]
fn reading_garbage_is_a_format_error() {
    let dir = TempDir::new("garbage");
    let path = dir.file("garbage.cadmium");
    std::fs::write(&path, b"not a zip").unwrap();
    assert!(matches!(
        DocumentSnapshot::read(&path),
        Err(DocumentError::Format(_))
    ));
}

#[test]
fn reading_a_missing_file_is_an_io_error() {
    let dir = TempDir::new("missing-file");
    assert!(matches!(
        DocumentSnapshot::read(&dir.file("does-not-exist.cadmium")),
        Err(DocumentError::Io(_))
    ));
}

#[test]
fn future_format_version_is_unsupported() {
    let entries = Entries::blank().edit_manifest(|m| {
        m["format"] = FUTURE_FORMAT.into();
        m["spec"] = serde_json::json!({ "something": "new" });
    });
    assert!(matches!(
        read_entries(entries),
        Err(DocumentError::UnsupportedVersion(FUTURE_FORMAT))
    ));
}

#[test]
fn unknown_pigment_in_manifest_is_a_format_error() {
    let entries = Entries::blank().edit_manifest(|m| {
        m["spec"]["pigment_slots"][0] = "unobtainium".into();
    });
    let reason = format_reason(read_entries(entries));
    assert!(
        reason.contains("unknown pigment \"unobtainium\""),
        "{reason}"
    );
}

#[test]
fn oversized_canvas_in_manifest_is_a_format_error() {
    let entries = Entries::blank().edit_manifest(|m| {
        m["spec"]["width"] = (MAX_CANVAS_SIZE + 1).into();
    });
    let reason = format_reason(read_entries(entries));
    assert!(reason.contains("canvas size"), "{reason}");
    assert!(reason.contains(&MAX_CANVAS_SIZE.to_string()), "{reason}");
}

#[test]
fn oversized_manifest_is_a_format_error() {
    let entries = Entries::blank().edit_manifest(|m| {
        m["padding"] = "x".repeat(MAX_MANIFEST_BYTES).into();
    });
    let reason = format_reason(read_entries(entries));
    assert!(reason.contains("too large"), "{reason}");
}

#[test]
fn missing_manifest_is_a_format_error() {
    let reason = format_reason(read_entries(Entries::blank().without("manifest.json")));
    assert!(reason.contains("missing manifest.json"), "{reason}");
}

#[test]
fn missing_state_entry_is_a_format_error() {
    let reason = format_reason(read_entries(
        Entries::blank().without("state/dry_height.zst"),
    ));
    assert!(reason.contains("missing state/dry_height.zst"), "{reason}");
}

#[test]
fn state_entry_shorter_than_the_canvas_is_a_format_error() {
    let entries = Entries::blank().edit_state("state/wet_vol.zst", |raw| {
        raw.truncate(raw.len() - 4);
    });
    let reason = format_reason(read_entries(entries));
    assert!(reason.contains("state/wet_vol.zst holds"), "{reason}");
}

#[test]
fn state_entry_decompressing_past_the_canvas_is_a_format_error() {
    let entries = Entries::blank().edit_state("state/wet_pig0.zst", |raw| {
        raw.extend(std::iter::repeat_n(0, 1 << 16));
    });
    let reason = format_reason(read_entries(entries));
    assert!(reason.contains("state/wet_pig0.zst holds"), "{reason}");
}

#[test]
fn non_finite_or_negative_wet_volume_is_a_format_error() {
    for value in [f32::NAN, f32::INFINITY, -1.0] {
        let entries = Entries::blank().edit_state("state/wet_vol.zst", set_first_f32(value));
        let reason = format_reason(read_entries(entries));
        assert!(
            reason.contains(&format!("state/wet_vol.zst {INVALID_THICKNESS}")),
            "{reason}"
        );
    }
}

#[test]
fn non_finite_or_negative_dry_height_is_a_format_error() {
    for value in [f32::NAN, f32::NEG_INFINITY, -0.5] {
        let entries = Entries::blank().edit_state(FORMAT_1_DRY_HEIGHT, set_first_f32(value));
        let reason = format_reason(read_entries(entries));
        assert!(
            reason.contains(&format!("{FORMAT_1_DRY_HEIGHT} {INVALID_THICKNESS}")),
            "{reason}"
        );
    }
}

#[test]
fn a_dry_relief_that_sinks_below_the_bare_canvas_is_a_format_error() {
    for value in [f32::NAN, f32::NEG_INFINITY, -0.5] {
        let entries = Entries::blank()
            .in_format_2()
            .edit_state(DRY_RELIEF, set_first_f32(value));
        let reason = format_reason(read_entries(entries));
        assert!(
            reason.contains(&format!("{DRY_RELIEF} {INVALID_THICKNESS}")),
            "{reason}"
        );
    }
}

#[test]
fn a_well_formed_format_2_document_reads() {
    assert!(read_entries(Entries::blank().in_format_2()).is_ok());
}

#[test]
fn a_painting_from_before_the_pencil_opens_without_graphite() {
    let gpu = GpuContext::new().unwrap();
    let opened = read_entries(Entries::blank())
        .unwrap()
        .into_session(&gpu)
        .unwrap();
    let graphite = opened.canvas.read_f32(&gpu, StateTexture::Graphite).unwrap();
    assert!(graphite.iter().all(|density| *density == 0.0));
}

#[test]
fn a_graphite_density_outside_zero_to_one_is_a_format_error() {
    for value in [2.0, -0.5, f32::NAN] {
        let entries = Entries::blank()
            .with_graphite()
            .edit_state(GRAPHITE_ENTRY, set_first_f32(value));
        let reason = format_reason(read_entries(entries));
        assert!(
            reason.contains(&format!("{GRAPHITE_ENTRY} holds a graphite density outside")),
            "{reason}"
        );
    }
}

#[test]
fn graphite_densities_of_exactly_zero_and_one_are_accepted() {
    for value in [0.0, MAX_DENSITY] {
        let entries = Entries::blank()
            .with_graphite()
            .edit_state(GRAPHITE_ENTRY, set_first_f32(value));
        assert!(read_entries(entries).is_ok(), "{value}");
    }
}
