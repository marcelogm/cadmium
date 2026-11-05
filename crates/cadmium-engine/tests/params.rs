use cadmium_engine::brush::{BrushSettings, HairSpec, HeadSpec};
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

fn assert_each_spec_is_unique_with_its_default_in_range(specs: &[ParamSpec]) {
    for (i, spec) in specs.iter().enumerate() {
        assert!(spec.min <= spec.default && spec.default <= spec.max);
        assert!(specs[i + 1..].iter().all(|other| other.key != spec.key));
    }
}

#[test]
fn specs_cover_every_lab_parameter_once() {
    let specs = SimParams::specs();
    assert_eq!(specs.len(), 37);
    assert_each_spec_is_unique_with_its_default_in_range(specs);
    let gone = [
        "bristle_count",
        "bristle_contrast",
        "tilt_elongation",
        "bristle_density",
        "bristle_length_round",
        "bristle_length_flat",
        "bristle_length_fan",
    ];
    for gone in gone {
        assert!(specs.iter().all(|spec| spec.key != gone), "{gone}");
    }
    let depth = specs.iter().find(|s| s.key == "penetration_um").unwrap();
    assert_eq!(depth.group, ParamGroup::Footprint);
    assert_eq!((depth.min, depth.max, depth.default), (0.0, 300.0, 120.0));
    let reserve = specs.iter().find(|s| s.key == "reserve_factor").unwrap();
    assert_eq!(
        (reserve.label, reserve.group),
        ("Reserve", ParamGroup::Transfer)
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
    assert_eq!(b.head, HeadSpec::ROUND);
    assert_eq!(b.hair, HairSpec::default());
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

#[test]
fn the_bristle_simulation_is_tuned_from_its_own_group() {
    let keys = [
        "bristle_stiffness",
        "bristle_damping",
        "bristle_press_depth",
        "bristle_static_friction",
        "bristle_kinetic_friction",
        "bristle_drag_anisotropy",
        "bristle_plasticity",
        "bristle_clumping",
        "bristle_pointing",
        "bristle_point_release",
        "bristle_point_film",
        "bristle_wet_support",
        "bristle_load_limit",
    ];
    for key in keys {
        let spec = SimParams::specs().iter().find(|s| s.key == key).unwrap();
        assert_eq!(spec.group, ParamGroup::Bristles, "{key}");
    }
}

#[test]
fn bristle_contact_tunables_span_their_physical_ranges() {
    let ranges = [
        ("bristle_static_friction", 0.0, 2.0),
        ("bristle_kinetic_friction", 0.0, 2.0),
        ("bristle_drag_anisotropy", 0.0, 1.0),
        ("bristle_plasticity", 0.0, 20.0),
        ("bristle_clumping", 0.0, 1.0),
        ("bristle_pointing", 0.0, 1.0),
        ("bristle_point_release", 0.01, 1.0),
        ("bristle_point_film", 0.01, 1.0),
        ("bristle_wet_support", 0.0, 1.0),
        ("bristle_load_limit", 1.0, 5.0),
    ];
    for (key, min, max) in ranges {
        let spec = SimParams::specs().iter().find(|s| s.key == key).unwrap();
        assert_eq!((spec.min, spec.max), (min, max), "{key}");
    }
}
