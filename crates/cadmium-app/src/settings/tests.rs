use super::*;
use crate::pressure_curve::PressureCurve;
use crate::test_dir::TempDir;

/// On Wayland the lower button always picked colour and the upper one panned.
#[test]
fn defaults_pick_with_the_lower_button_and_pan_with_the_upper() {
    let settings = Settings::default();
    assert_eq!(settings.lower_pen_button, PenButtonAction::PickColour);
    assert_eq!(settings.upper_pen_button, PenButtonAction::Pan);
}

fn soft_curve() -> PressureCurve {
    PressureCurve::from_points(&[(0.0, 0.1), (0.4, 0.6), (1.0, 0.9)]).unwrap()
}

#[test]
fn settings_round_trip_through_json() {
    let settings = Settings {
        lower_pen_button: PenButtonAction::Rotate,
        upper_pen_button: PenButtonAction::Nothing,
        pressure_curve: soft_curve(),
        ..Settings::default()
    };
    assert_eq!(Settings::from_json(&settings.to_json()), settings);
}

#[test]
fn a_bad_field_falls_back_alone() {
    let read = Settings::from_json(r#"{"lower_pen_button":"teleport","upper_pen_button":"zoom"}"#);
    assert_eq!(read.lower_pen_button, Settings::default().lower_pen_button);
    assert_eq!(read.upper_pen_button, PenButtonAction::Zoom);
}

#[test]
fn unreadable_json_gives_the_defaults() {
    assert_eq!(Settings::from_json("not json"), Settings::default());
    assert_eq!(Settings::from_json("[1, 2]"), Settings::default());
}

#[test]
fn a_missing_file_gives_the_defaults() {
    let store = SettingsStore::load(TempDir::new().path().join("settings.json"));
    assert_eq!(store.get(), Settings::default());
}

#[test]
fn a_change_is_saved_and_read_back() {
    let dir = TempDir::new();
    let path = dir.path().join("nested").join("settings.json");
    let mut store = SettingsStore::load(path.clone());
    store
        .update(|settings| settings.upper_pen_button = PenButtonAction::Zoom)
        .unwrap();
    assert_eq!(
        SettingsStore::load(path).get().upper_pen_button,
        PenButtonAction::Zoom
    );
}

#[test]
fn a_store_without_a_file_changes_in_memory_only() {
    let mut store = SettingsStore::default();
    store
        .update(|settings| settings.lower_pen_button = PenButtonAction::Nothing)
        .unwrap();
    assert_eq!(store.get().lower_pen_button, PenButtonAction::Nothing);
}

#[test]
fn every_action_has_a_label_in_menu_order() {
    let labels: Vec<_> = PenButtonAction::ALL
        .iter()
        .map(|action| action.label())
        .collect();
    assert_eq!(labels, ["Move", "Zoom", "Rotate", "Pick Colour", "Nothing"]);
}

#[test]
fn menu_positions_map_to_actions_and_back() {
    for (index, action) in PenButtonAction::ALL.iter().enumerate() {
        assert_eq!(action.menu_index(), index as u32);
        assert_eq!(PenButtonAction::at_menu_index(index as u32), Some(*action));
    }
    assert_eq!(
        PenButtonAction::at_menu_index(PenButtonAction::ALL.len() as u32),
        None
    );
}

/// Adding an action makes this `match` fail to compile until `ALL` is updated too.
#[test]
fn every_action_is_in_the_menu() {
    for action in PenButtonAction::ALL {
        match action {
            PenButtonAction::Pan
            | PenButtonAction::Zoom
            | PenButtonAction::Rotate
            | PenButtonAction::PickColour
            | PenButtonAction::Nothing => {}
        }
    }
    for action in [
        PenButtonAction::Pan,
        PenButtonAction::Zoom,
        PenButtonAction::Rotate,
        PenButtonAction::PickColour,
        PenButtonAction::Nothing,
    ] {
        assert!(PenButtonAction::ALL.contains(&action));
    }
}

#[test]
fn a_file_from_the_days_of_the_tray_still_reads() {
    let text =
        r#"{"tray_shown": false, "mixing_knife_diameter_px": 120.0, "pencil_hardness": 0.25}"#;
    let settings = Settings::from_json(text);
    assert_eq!(settings.pencil_hardness, 0.25);
    assert_eq!(
        settings.lower_pen_button,
        Settings::default().lower_pen_button
    );
}

#[test]
fn saved_settings_no_longer_mention_the_tray_or_the_mixing_knife() {
    let text = Settings::default().to_json();
    assert!(!text.contains("tray"));
    assert!(!text.contains("mixing_knife"));
}

#[test]
fn the_pencil_starts_at_hb() {
    assert_eq!(Settings::default().pencil_hardness, DEFAULT_PENCIL_HARDNESS);
}

#[test]
fn the_pencil_hardness_round_trips_through_json() {
    let settings = Settings {
        pencil_hardness: 0.25,
        ..Settings::default()
    };
    assert_eq!(Settings::from_json(&settings.to_json()), settings);
}

#[test]
fn a_pencil_hardness_out_of_range_is_clamped_and_garbage_falls_back() {
    let hardness = |text| Settings::from_json(text).pencil_hardness;
    assert_eq!(hardness(r#"{"pencil_hardness": 2.0}"#), 1.0);
    assert_eq!(hardness(r#"{"pencil_hardness": -1.0}"#), 0.0);
    assert_eq!(
        hardness(r#"{"pencil_hardness": "soft"}"#),
        DEFAULT_PENCIL_HARDNESS
    );
    assert_eq!(
        hardness(r#"{"lower_pen_button":"zoom"}"#),
        DEFAULT_PENCIL_HARDNESS
    );
}

#[test]
fn a_changed_hardness_is_saved_and_read_back() {
    let dir = TempDir::new();
    let path = dir.path().join("settings.json");
    let mut store = SettingsStore::load(path.clone());
    store
        .update(|settings| settings.pencil_hardness = 0.9)
        .unwrap();
    assert_eq!(SettingsStore::load(path).get().pencil_hardness, 0.9);
}

#[test]
fn the_pressure_curve_starts_linear() {
    assert_eq!(Settings::default().pressure_curve, PressureCurve::linear());
}

#[test]
fn a_file_from_before_the_pressure_curve_reads_it_as_linear() {
    let read = Settings::from_json(r#"{"lower_pen_button":"zoom","upper_pen_button":"pan"}"#);
    assert_eq!(read.lower_pen_button, PenButtonAction::Zoom);
    assert_eq!(read.pressure_curve, PressureCurve::linear());
}

#[test]
fn a_bad_pressure_curve_falls_back_alone() {
    let read = Settings::from_json(
        r#"{"lower_pen_button":"zoom","pressure_curve":[[0.0,0.0],[0.5,2.0],[1.0,1.0]]}"#,
    );
    assert_eq!(read.lower_pen_button, PenButtonAction::Zoom);
    assert_eq!(read.pressure_curve, PressureCurve::linear());
}

#[test]
fn a_changed_curve_is_saved_and_read_back() {
    let dir = TempDir::new();
    let path = dir.path().join("settings.json");
    let mut store = SettingsStore::load(path.clone());
    store
        .update(|settings| settings.pressure_curve = soft_curve())
        .unwrap();
    assert_eq!(SettingsStore::load(path).get().pressure_curve, soft_curve());
}
