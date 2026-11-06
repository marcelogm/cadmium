use super::*;
use crate::state::test_support::painting_state;

#[test]
fn each_slot_is_described_by_its_pigment() {
    let state = painting_state();
    let db = &state.session.db;
    let slots = &state.session.canvas.spec.pigment_slots;
    let infos = slot_infos(db, slots);
    assert_eq!(infos.len(), slots.len());
    let pigment = db.get(&slots[0]).unwrap();
    assert_eq!(infos[0].name, pigment.name);
    assert_eq!(
        infos[0].subtitle,
        format!("{} · {}", pigment.colour_index, pigment.provenance)
    );
    assert_eq!(infos[0].masstone, db.masstone_srgb8(pigment.id));
    assert_eq!(infos[0].glaze_over_white, db.glaze_srgb8(pigment.id, false));
    assert_eq!(infos[0].glaze_over_black, db.glaze_srgb8(pigment.id, true));
}

#[test]
fn unknown_pigments_are_skipped() {
    let state = painting_state();
    let infos = slot_infos(&state.session.db, &["no-such-pigment".to_string()]);
    assert!(infos.is_empty());
}

#[test]
fn the_snapshot_reads_the_asked_parameters_in_order() {
    let state = painting_state();
    let params = state.session.engine.params();
    let expected = [
        params.get("velocity_cutoff"),
        params.get("transfer_fraction"),
    ];
    assert_ne!(expected[0], expected[1]);
    let snapshot = Snapshot::of(&state, ["velocity_cutoff", "transfer_fraction"]);
    assert_eq!(snapshot.values, expected);
    assert_eq!(snapshot.slots, state.session.canvas.spec.pigment_slots);
}
