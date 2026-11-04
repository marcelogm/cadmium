use cadmium_engine::{canvas::*, pigments::PigmentDb};

use crate::spec;

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
            s.width = MAX_CANVAS_SIZE;
            s.height = 1;
        })
        .is_ok()
    );
}

#[test]
fn spec_validation_rejects_out_of_range_dimensions() {
    assert!(validated(|s| s.width = 0).is_err());
    assert!(validated(|s| s.height = 0).is_err());
    assert!(validated(|s| s.width = MAX_CANVAS_SIZE + 1).is_err());
    assert!(validated(|s| s.height = MAX_CANVAS_SIZE + 1).is_err());
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
