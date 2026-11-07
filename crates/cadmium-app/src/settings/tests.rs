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
fn the_tray_starts_shown_under_a_third_of_the_window() {
    let defaults = Settings::default();
    assert!(defaults.tray_shown);
    assert_eq!(defaults.tray_share, DEFAULT_TRAY_SHARE);
}

#[test]
fn tray_layout_round_trips_through_json() {
    let settings = Settings {
        tray_share: 0.5,
        tray_shown: false,
        ..Settings::default()
    };
    assert_eq!(Settings::from_json(&settings.to_json()), settings);
}

#[test]
fn a_tray_share_out_of_range_is_clamped_and_garbage_falls_back() {
    assert_eq!(
        Settings::from_json(r#"{"tray_share": 2.0}"#).tray_share,
        MAX_TRAY_SHARE
    );
    assert_eq!(
        Settings::from_json(r#"{"tray_share": -1.0}"#).tray_share,
        MIN_TRAY_SHARE
    );
    assert_eq!(
        Settings::from_json(r#"{"tray_share": "big"}"#).tray_share,
        DEFAULT_TRAY_SHARE
    );
    assert!(Settings::from_json(r#"{"tray_shown": 3}"#).tray_shown);
}

#[test]
fn the_divider_position_and_the_share_convert_both_ways() {
    assert_eq!(divider_position(0.25, 800), 600);
    assert_eq!(tray_share_at(600, 800), 0.25);
    assert_eq!(tray_share_at(799, 800), MIN_TRAY_SHARE);
    assert_eq!(tray_share_at(0, 800), MAX_TRAY_SHARE);
    assert_eq!(tray_share_at(10, 0), DEFAULT_TRAY_SHARE);
}

#[test]
fn the_saved_tray_share_is_rounded() {
    assert!(
        Settings::default()
            .to_json()
            .contains("\"tray_share\": 0.35,")
    );
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

#[test]
fn the_mixing_knife_size_round_trips_and_starts_at_eighty() {
    assert_eq!(Settings::default().mixing_knife_diameter_px, 80.0);
    let settings = Settings {
        mixing_knife_diameter_px: 150.0,
        ..Settings::default()
    };
    assert_eq!(Settings::from_json(&settings.to_json()), settings);
}

#[test]
fn a_mixing_knife_size_out_of_range_is_clamped_and_a_bad_one_falls_back_alone() {
    let read = |text: &str| Settings::from_json(text).mixing_knife_diameter_px;
    assert_eq!(
        read(r#"{"mixing_knife_diameter_px":9000}"#),
        MAX_MIXING_KNIFE_DIAMETER_PX
    );
    assert_eq!(
        read(r#"{"mixing_knife_diameter_px":0.5}"#),
        MIN_MIXING_KNIFE_DIAMETER_PX
    );
    let settings = Settings::from_json(r#"{"mixing_knife_diameter_px":"wide","tray_shown":false}"#);
    assert_eq!(
        settings.mixing_knife_diameter_px,
        DEFAULT_MIXING_KNIFE_DIAMETER_PX
    );
    assert!(!settings.tray_shown);
}
