use super::{BrushFileError, brush_from_json, brush_to_json};
use crate::brush::{BrushSpec, HairSpec, HeadSpec, factory_brushes};

#[test]
fn every_factory_brush_survives_a_round_trip() {
    for brush in factory_brushes() {
        let read = brush_from_json(&brush_to_json(&brush)).unwrap();
        assert_eq!(read.brush, brush);
        assert!(!read.adjusted, "{}", brush.name);
    }
}

#[test]
fn the_file_names_its_format_and_version() {
    let json: serde_json::Value =
        serde_json::from_str(&brush_to_json(&factory_brushes()[0])).unwrap();
    assert_eq!(json["format"], "cadmium-brush");
    assert_eq!(json["version"], 1);
    assert_eq!(json["head"]["thickness"], 1.0);
}

#[test]
fn broken_json_is_malformed() {
    assert!(matches!(
        brush_from_json("{ not json"),
        Err(BrushFileError::Malformed(_))
    ));
}

#[test]
fn another_format_is_not_a_brush() {
    let text = r#"{ "format": "something-else", "version": 1, "name": "x", "size_px": 10 }"#;
    assert!(matches!(
        brush_from_json(text),
        Err(BrushFileError::NotABrush)
    ));
}

#[test]
fn a_newer_version_is_refused() {
    let text = r#"{ "format": "cadmium-brush", "version": 2, "name": "x", "size_px": 10 }"#;
    assert!(matches!(
        brush_from_json(text),
        Err(BrushFileError::NewerVersion(2))
    ));
}

#[test]
fn missing_fields_take_their_defaults() {
    let text = r#"{ "format": "cadmium-brush", "version": 1, "name": "Bare", "size_px": 30,
                   "head": { "dome": 0.5 } }"#;
    let read = brush_from_json(text).unwrap();
    assert_eq!(
        read.brush.head,
        HeadSpec {
            dome: 0.5,
            ..HeadSpec::default()
        }
    );
    assert_eq!(read.brush.hair, HairSpec::default());
    assert!(!read.adjusted);
}

#[test]
fn out_of_range_values_are_clamped_and_reported() {
    let text = r#"{ "format": "cadmium-brush", "version": 1, "name": "Wild", "size_px": 9000,
                   "head": { "thickness": 7.0 }, "hair": { "stiffness": -3.0 } }"#;
    let read = brush_from_json(text).unwrap();
    assert!(read.adjusted);
    assert_eq!(read.brush.size_px, BrushSpec::SIZE_RANGE_PX[1]);
    assert_eq!(read.brush.head.thickness, HeadSpec::THICKNESS_RANGE[1]);
    assert_eq!(read.brush.hair.stiffness, HairSpec::STIFFNESS_RANGE[0]);
}

#[test]
fn a_blank_name_becomes_a_placeholder() {
    let text = r#"{ "format": "cadmium-brush", "version": 1, "name": "  ", "size_px": 30 }"#;
    assert_eq!(
        brush_from_json(text).unwrap().brush.name,
        super::UNNAMED_BRUSH
    );
}

#[test]
fn unknown_fields_are_ignored() {
    let text = r#"{ "format": "cadmium-brush", "version": 1, "name": "Future", "size_px": 30,
                   "thumbnail": "later", "head": { "bristle_colour": "red" } }"#;
    assert!(brush_from_json(text).is_ok());
}

#[test]
fn a_long_name_is_cut_and_reported() {
    let long = "Sable ".repeat(40);
    let text = format!(
        r#"{{ "format": "cadmium-brush", "version": 1, "name": "{long}", "size_px": 30 }}"#
    );
    let read = brush_from_json(&text).unwrap();
    assert_eq!(read.brush.name.chars().count(), super::MAX_NAME_CHARS);
    assert!(read.adjusted);
}

#[test]
fn a_name_is_cut_on_characters_not_bytes() {
    let long = "·".repeat(100);
    let text = format!(
        r#"{{ "format": "cadmium-brush", "version": 1, "name": "{long}", "size_px": 30 }}"#
    );
    let read = brush_from_json(&text).unwrap();
    assert_eq!(read.brush.name, "·".repeat(super::MAX_NAME_CHARS));
}

#[test]
fn version_zero_is_read() {
    let text = r#"{ "format": "cadmium-brush", "version": 0, "name": "Old", "size_px": 30 }"#;
    assert_eq!(brush_from_json(text).unwrap().brush.name, "Old");
}

#[test]
fn a_missing_format_is_malformed() {
    let text = r#"{ "version": 1, "name": "x", "size_px": 10 }"#;
    assert!(matches!(
        brush_from_json(text),
        Err(BrushFileError::Malformed(_))
    ));
}

#[test]
fn a_null_or_text_number_is_malformed() {
    for size in ["null", r#""big""#] {
        let text = format!(
            r#"{{ "format": "cadmium-brush", "version": 1, "name": "x", "size_px": {size} }}"#
        );
        assert!(
            matches!(brush_from_json(&text), Err(BrushFileError::Malformed(_))),
            "{size}"
        );
    }
}
