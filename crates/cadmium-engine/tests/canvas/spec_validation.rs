use cadmium_engine::{canvas::*, pigments::PigmentDb};

use crate::spec;
use crate::support::painting_request;

fn validated(edit: impl FnOnce(&mut CanvasSpec)) -> Result<(), String> {
    let mut s = spec(Primer::WhiteGesso, Weave::Linen);
    edit(&mut s);
    s.validate(&PigmentDb::builtin())
}

#[test]
fn spec_validation_accepts_a_normal_canvas() {
    assert!(validated(|_| {}).is_ok());
    assert!(
        validated(|s| {
            s.width = MAX_PAINTING_SIZE;
            s.height = 1;
        })
        .is_ok()
    );
}

#[test]
fn spec_validation_rejects_out_of_range_dimensions() {
    assert!(validated(|s| s.width = 0).is_err());
    assert!(validated(|s| s.height = 0).is_err());
    assert!(validated(|s| s.width = MAX_PAINTING_SIZE + 1).is_err());
    assert!(validated(|s| s.height = MAX_PAINTING_SIZE + 1).is_err());
}

#[test]
fn spec_validation_rejects_empty_or_overfull_slot_tables() {
    assert!(validated(|s| s.pigment_slots.clear()).is_err());
    assert!(
        validated(|s| {
            s.pigment_slots = PigmentDb::builtin()
                .all()
                .iter()
                .take(9)
                .map(|p| p.id.to_string())
                .collect()
        })
        .is_err()
    );
}

#[test]
fn spec_validation_rejects_unknown_and_duplicate_pigments() {
    assert!(validated(|s| s.pigment_slots.push("mauveine".into())).is_err());
    assert!(validated(|s| s.pigment_slots.push("titanium_white".into())).is_err());
}

#[test]
fn spec_survives_a_json_roundtrip() {
    let original = spec(Primer::SiennaImprimatura, Weave::Cotton);
    let json = serde_json::to_string(&original).unwrap();
    assert_eq!(serde_json::from_str::<CanvasSpec>(&json).unwrap(), original);
}

#[test]
fn a_painting_sits_inside_a_margin_on_every_side() {
    let spec = CanvasSpec::for_painting(painting_request(1000, 700));
    assert_eq!(
        (spec.width, spec.height, spec.margin_px),
        (1000 + 2 * MARGIN_PX, 700 + 2 * MARGIN_PX, MARGIN_PX)
    );
    assert_eq!(
        spec.painting_rect(),
        PaintingRect {
            x: MARGIN_PX,
            y: MARGIN_PX,
            width: 1000,
            height: 700
        }
    );
    assert!(spec.is_in_margin(0, 0));
    assert!(spec.is_in_margin(MARGIN_PX - 1, MARGIN_PX + 5));
    assert!(!spec.is_in_margin(MARGIN_PX, MARGIN_PX));
    assert!(!spec.is_in_margin(MARGIN_PX + 999, MARGIN_PX + 699));
    assert!(spec.is_in_margin(MARGIN_PX + 1000, MARGIN_PX + 699));
}

#[test]
fn the_largest_painting_is_still_valid_and_a_larger_one_is_not() {
    let db = PigmentDb::builtin();
    assert!(
        CanvasSpec::for_painting(painting_request(MAX_PAINTING_SIZE, MAX_PAINTING_SIZE))
            .validate(&db)
            .is_ok()
    );
    assert!(
        CanvasSpec::for_painting(painting_request(MAX_PAINTING_SIZE + 1, 100))
            .validate(&db)
            .is_err()
    );
}

#[test]
fn a_margin_that_is_not_whole_tiles_or_leaves_no_painting_is_invalid() {
    let db = PigmentDb::builtin();
    let mut spec = CanvasSpec::for_painting(painting_request(200, 200));
    spec.margin_px = 100;
    assert!(spec.validate(&db).is_err());
    spec.margin_px = MARGIN_PX;
    spec.width = 2 * MARGIN_PX - 1;
    let reason = spec.validate(&db).unwrap_err();
    assert!(reason.contains("leaves no painting"), "{reason}");
}

#[test]
fn a_spec_saved_without_a_margin_has_none() {
    let json = r#"{"width":64,"height":64,"primer":"WhiteGesso","weave":"Linen","pigment_slots":["titanium_white"]}"#;
    let spec: CanvasSpec = serde_json::from_str(json).unwrap();
    assert_eq!(spec.margin_px, 0);
    assert_eq!(
        spec.painting_rect(),
        PaintingRect {
            x: 0,
            y: 0,
            width: 64,
            height: 64
        }
    );
}

#[test]
fn a_margin_too_wide_to_double_is_refused_without_overflowing() {
    let mut spec = CanvasSpec::for_painting(painting_request(200, 200));
    spec.margin_px = 1 << 31;
    let reason = spec.validate(&PigmentDb::builtin()).unwrap_err();
    assert!(reason.contains("margin of"), "{reason}");
    assert_eq!(
        (spec.painting_rect().width, spec.painting_rect().height),
        (0, 0)
    );
}

#[test]
fn a_margin_other_than_none_or_the_painting_margin_is_invalid() {
    let db = PigmentDb::builtin();
    let mut spec = CanvasSpec::for_painting(painting_request(200, 200));
    spec.margin_px = 1 << 20;
    spec.width = (1 << 21) + 1;
    spec.height = (1 << 21) + 1;
    let reason = spec.validate(&db).unwrap_err();
    assert!(reason.contains("margin of 1048576 px"), "{reason}");
    let mut wider = CanvasSpec::for_painting(painting_request(200, 200));
    wider.margin_px = 2 * MARGIN_PX;
    wider.width += 2 * MARGIN_PX;
    wider.height += 2 * MARGIN_PX;
    assert!(wider.validate(&db).is_err());
}
