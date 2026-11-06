use super::*;

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cadmium-settings-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn defaults_keep_the_pen_buttons_as_they_always_were() {
    let settings = Settings::default();
    assert_eq!(settings.lower_pen_button, PenButtonAction::Pan);
    assert_eq!(settings.upper_pen_button, PenButtonAction::PickColour);
}

#[test]
fn settings_round_trip_through_json() {
    let settings = Settings {
        lower_pen_button: PenButtonAction::Rotate,
        upper_pen_button: PenButtonAction::Nothing,
    };
    assert_eq!(Settings::from_json(&settings.to_json()), settings);
}

#[test]
fn a_bad_field_falls_back_alone() {
    let read = Settings::from_json(r#"{"lower_pen_button":"teleport","upper_pen_button":"zoom"}"#);
    assert_eq!(read.lower_pen_button, PenButtonAction::Pan);
    assert_eq!(read.upper_pen_button, PenButtonAction::Zoom);
}

#[test]
fn unreadable_json_gives_the_defaults() {
    assert_eq!(Settings::from_json("not json"), Settings::default());
    assert_eq!(Settings::from_json("[1, 2]"), Settings::default());
}

#[test]
fn a_missing_file_gives_the_defaults() {
    let store = SettingsStore::load(scratch_dir("missing").join("settings.json"));
    assert_eq!(store.get(), Settings::default());
}

#[test]
fn a_change_is_saved_and_read_back() {
    let path = scratch_dir("saved").join("nested").join("settings.json");
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
