use cadmium_engine::brush::{BrushSettings, BrushShape};
use cadmium_engine::params::*;
use cadmium_engine::stroke::Tool;

#[test]
fn defaults_match_specs() {
    let p = SimParams::default();
    for spec in SimParams::specs() {
        assert_eq!(p.get(spec.key), spec.default);
    }
}

#[test]
fn set_clamps_to_range() {
    let mut p = SimParams::default();
    p.set("transfer_fraction", 5.0);
    assert_eq!(p.get("transfer_fraction"), 1.0);
    p.set("light_elevation_deg", 0.0);
    assert_eq!(p.get("light_elevation_deg"), 5.0);
}

#[test]
fn specs_cover_every_lab_parameter_once() {
    let specs = SimParams::specs();
    assert_eq!(specs.len(), 19);
    for (i, spec) in specs.iter().enumerate() {
        assert!(spec.min <= spec.default && spec.default <= spec.max);
        assert!(specs[i + 1..].iter().all(|other| other.key != spec.key));
    }
    let bristles = specs.iter().find(|s| s.key == "bristle_count").unwrap();
    assert_eq!(bristles.group, ParamGroup::Footprint);
    assert_eq!(
        (bristles.min, bristles.max, bristles.default),
        (4.0, 64.0, 24.0)
    );
    let reserve = specs.iter().find(|s| s.key == "reserve_factor").unwrap();
    assert_eq!(
        (reserve.label, reserve.group),
        ("Reserva", ParamGroup::Transfer)
    );
    assert_eq!(
        (reserve.min, reserve.max, reserve.default),
        (1.0, 12.0, 3.0)
    );
}

#[test]
fn reservoir_mixture_leaves_room_for_medium() {
    let b = BrushSettings {
        medium: 0.25,
        recipe: vec![(0, 1.0), (1, 3.0)],
        ..BrushSettings::default()
    };
    let m = b.reservoir_mixture(2);
    assert!((m.fractions[0] - 0.1875).abs() < 1e-6);
    assert!((m.fractions[1] - 0.5625).abs() < 1e-6);
}

#[test]
fn reservoir_mixture_ignores_slots_outside_the_canvas() {
    let b = BrushSettings {
        medium: 0.0,
        recipe: vec![(0, 1.0), (3, 1.0)],
        ..BrushSettings::default()
    };
    let m = b.reservoir_mixture(2);
    assert!((m.fractions[0] - 1.0).abs() < 1e-6);
    assert_eq!(m.fractions[3], 0.0);
}

#[test]
fn reservoir_volume_follows_load() {
    let b = BrushSettings::default();
    let m = b.reservoir_mixture(1);
    assert!((m.volume_um - 160.0).abs() < 1e-4);
}

#[test]
fn default_brush_is_a_round_brush_with_the_first_slot() {
    let b = BrushSettings::default();
    assert_eq!(b.tool, Tool::Brush);
    assert_eq!(b.shape, BrushShape::Round);
    assert_eq!(b.geometry.max_diameter_px, 40.0);
    assert_eq!(b.recipe, vec![(0, 1.0)]);
}

#[test]
fn set_ignores_nan() {
    let mut p = SimParams::default();
    p.set("transfer_fraction", f32::NAN);
    assert_eq!(p.get("transfer_fraction"), 0.1);
}

#[test]
fn reservoir_mixture_keeps_some_pigment_however_much_medium() {
    let b = BrushSettings {
        medium: 3.0,
        ..BrushSettings::default()
    };
    let m = b.reservoir_mixture(1);
    assert!((m.fractions[0] - 0.05).abs() < 1e-6);
    let b = BrushSettings {
        medium: -1.0,
        ..BrushSettings::default()
    };
    assert!((b.reservoir_mixture(1).fractions[0] - 1.0).abs() < 1e-6);
}

#[test]
fn reservoir_volume_never_exceeds_a_full_load() {
    let over = BrushSettings {
        load: 4.0,
        ..BrushSettings::default()
    };
    assert!((over.reservoir_mixture(1).volume_um - 200.0).abs() < 1e-4);
    let under = BrushSettings {
        load: -1.0,
        ..BrushSettings::default()
    };
    assert_eq!(under.reservoir_mixture(1).volume_um, 0.0);
}
