use cadmium_engine::{
    canvas::MAX_CANVAS_SIZE,
    document::{DocumentError, DocumentSnapshot},
};

use crate::entries::{Entries, format_reason, read_entries, set_first_f32};
use crate::{CURRENT_FORMAT, DRY_RELIEF, GRAPHITE_ENTRY, INVALID_THICKNESS, TempDir};

const FUTURE_FORMAT: u32 = CURRENT_FORMAT + 1;
const PREVIOUS_FORMAT: u32 = CURRENT_FORMAT - 1;
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

fn unsupported_version_of_a_document_claiming(format: u32) -> Option<u32> {
    let entries = Entries::blank().edit_manifest(|m| m["format"] = format.into());
    match read_entries(entries) {
        Err(DocumentError::UnsupportedVersion(found)) => Some(found),
        _ => None,
    }
}

#[test]
fn every_format_but_the_current_one_is_unsupported() {
    for format in [1, 2, 3, 4, 5, 6, 7, 9] {
        let found = unsupported_version_of_a_document_claiming(format);
        assert_eq!(found, Some(format), "format {format}");
    }
}

#[test]
fn a_painting_of_the_format_before_the_sheet_is_unsupported_rather_than_malformed() {
    let mut entries = Entries::blank().edit_manifest(|m| m["format"] = PREVIOUS_FORMAT.into());
    entries.0.retain(|(name, _)| !name.starts_with("sheet/"));
    assert!(matches!(
        read_entries(entries),
        Err(DocumentError::UnsupportedVersion(PREVIOUS_FORMAT))
    ));
}

#[test]
fn a_manifest_whose_format_is_missing_or_not_an_integer_is_a_format_error() {
    let missing = Entries::blank().edit_manifest(|m| {
        m.as_object_mut().unwrap().remove("format");
    });
    let reason = format_reason(read_entries(missing));
    assert!(reason.contains("manifest.json"), "{reason}");
    for format in [serde_json::json!("six"), serde_json::json!(6.5)] {
        let entries = Entries::blank().edit_manifest(|m| m["format"] = format);
        let reason = format_reason(read_entries(entries));
        assert!(reason.contains("manifest.json"), "{reason}");
    }
}

#[test]
fn an_entry_the_format_does_not_name_is_ignored() {
    let entries = Entries::blank().with("tray/wet_vol.zst", b"not a state".to_vec());
    assert!(read_entries(entries).is_ok());
}

#[test]
fn a_document_without_its_graphite_entry_is_a_format_error() {
    let reason = format_reason(read_entries(Entries::blank().without(GRAPHITE_ENTRY)));
    assert!(
        reason.contains(&format!("missing {GRAPHITE_ENTRY}")),
        "{reason}"
    );
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
    let reason = format_reason(read_entries(Entries::blank().without(DRY_RELIEF)));
    assert!(
        reason.contains(&format!("missing {DRY_RELIEF}")),
        "{reason}"
    );
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
fn a_dry_relief_that_sinks_below_the_bare_canvas_is_a_format_error() {
    for value in [f32::NAN, f32::NEG_INFINITY, -0.5] {
        let entries = Entries::blank().edit_state(DRY_RELIEF, set_first_f32(value));
        let reason = format_reason(read_entries(entries));
        assert!(
            reason.contains(&format!("{DRY_RELIEF} {INVALID_THICKNESS}")),
            "{reason}"
        );
    }
}

#[test]
fn a_graphite_density_outside_zero_to_one_is_a_format_error() {
    for value in [2.0, -0.5, f32::NAN] {
        let entries = Entries::blank().edit_state(GRAPHITE_ENTRY, set_first_f32(value));
        let reason = format_reason(read_entries(entries));
        assert!(
            reason.contains(&format!(
                "{GRAPHITE_ENTRY} holds a graphite density outside"
            )),
            "{reason}"
        );
    }
}

#[test]
fn graphite_densities_of_exactly_zero_and_one_are_accepted() {
    for value in [0.0, MAX_DENSITY] {
        let entries = Entries::blank().edit_state(GRAPHITE_ENTRY, set_first_f32(value));
        assert!(read_entries(entries).is_ok(), "{value}");
    }
}
