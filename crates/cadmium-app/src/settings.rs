use std::path::{Path, PathBuf};

use gtk::glib;
use serde::{Deserialize, Serialize};

use crate::LOG_DOMAIN;
use crate::atomic_file;
use crate::pressure_curve::PressureCurve;

/// What a pen barrel button does on the painting canvas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PenButtonAction {
    /// Drags the canvas along (labelled Move).
    Pan,
    /// Zooms the canvas.
    Zoom,
    /// Turns the canvas.
    Rotate,
    /// Picks the colour under the pen onto the brush.
    PickColour,
    /// Ignores the button.
    Nothing,
}

impl PenButtonAction {
    /// Every action, in the order the Preferences menu lists them.
    pub const ALL: [PenButtonAction; 5] = [
        PenButtonAction::Pan,
        PenButtonAction::Zoom,
        PenButtonAction::Rotate,
        PenButtonAction::PickColour,
        PenButtonAction::Nothing,
    ];

    /// Where this action sits in the Preferences menu.
    pub fn menu_index(self) -> u32 {
        Self::ALL
            .iter()
            .position(|action| *action == self)
            .map(|index| index as u32)
            .expect("every action is listed in ALL")
    }

    /// The action at a Preferences menu position, if there is one.
    pub fn at_menu_index(index: u32) -> Option<PenButtonAction> {
        Self::ALL.get(index as usize).copied()
    }

    /// The name the Preferences menu shows for this action.
    pub fn label(self) -> &'static str {
        match self {
            PenButtonAction::Pan => "Move",
            PenButtonAction::Zoom => "Zoom",
            PenButtonAction::Rotate => "Rotate",
            PenButtonAction::PickColour => "Pick Colour",
            PenButtonAction::Nothing => "Nothing",
        }
    }
}

/// The mixing knife's blade length by default.
pub const DEFAULT_MIXING_KNIFE_DIAMETER_PX: f32 = 80.0;
/// The shortest blade the mixing knife may have, as short as the size slider goes.
pub const MIN_MIXING_KNIFE_DIAMETER_PX: f32 = 4.0;
/// The longest blade the mixing knife may have, as long as the size slider goes.
pub const MAX_MIXING_KNIFE_DIAMETER_PX: f32 = 500.0;

/// A mixing knife size the knife can have; one that isn't a size gives the default.
pub fn mixing_knife_diameter(diameter_px: f32) -> f32 {
    if diameter_px.is_finite() {
        diameter_px.clamp(MIN_MIXING_KNIFE_DIAMETER_PX, MAX_MIXING_KNIFE_DIAMETER_PX)
    } else {
        DEFAULT_MIXING_KNIFE_DIAMETER_PX
    }
}

/// The painter's preferences. They are read through [`Settings::from_json`], which gives each
/// field its own fallback, so this does not derive `Deserialize`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Settings {
    /// The barrel button nearer the tip.
    pub lower_pen_button: PenButtonAction,
    /// The barrel button further from the tip.
    pub upper_pen_button: PenButtonAction,
    /// How the pen's pressure maps onto the brush's.
    pub pressure_curve: PressureCurve,
    /// Whether the tray's sheet is open under the canvas.
    pub tray_shown: bool,
    /// The mixing knife's blade length, in canvas px.
    pub mixing_knife_diameter_px: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            lower_pen_button: PenButtonAction::PickColour,
            upper_pen_button: PenButtonAction::Pan,
            pressure_curve: PressureCurve::linear(),
            tray_shown: true,
            mixing_knife_diameter_px: DEFAULT_MIXING_KNIFE_DIAMETER_PX,
        }
    }
}

impl Settings {
    /// The settings `text` holds; each field it lacks or can't be read falls back to its
    /// default.
    pub fn from_json(text: &str) -> Settings {
        let defaults = Settings::default();
        let Ok(serde_json::Value::Object(fields)) = serde_json::from_str(text) else {
            return defaults;
        };
        let field = |name: &str, fallback: PenButtonAction| {
            fields
                .get(name)
                .and_then(|value| PenButtonAction::deserialize(value).ok())
                .unwrap_or(fallback)
        };
        Settings {
            lower_pen_button: field("lower_pen_button", defaults.lower_pen_button),
            upper_pen_button: field("upper_pen_button", defaults.upper_pen_button),
            pressure_curve: fields
                .get("pressure_curve")
                .and_then(PressureCurve::from_json)
                .unwrap_or(defaults.pressure_curve),
            tray_shown: fields
                .get("tray_shown")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(defaults.tray_shown),
            mixing_knife_diameter_px: fields
                .get("mixing_knife_diameter_px")
                .and_then(serde_json::Value::as_f64)
                .map(|diameter| mixing_knife_diameter(diameter as f32))
                .unwrap_or(defaults.mixing_knife_diameter_px),
        }
    }

    /// The settings as the JSON text kept in the settings file.
    pub fn to_json(self) -> String {
        serde_json::to_string_pretty(&self).expect("settings serialise")
    }
}

/// The settings and the file they are kept in; without a file they live in memory only.
#[derive(Debug, Default)]
pub struct SettingsStore {
    settings: Settings,
    file: Option<PathBuf>,
}

impl SettingsStore {
    /// The default location: `<user config dir>/cadmium/settings.json`.
    pub fn default_path() -> PathBuf {
        glib::user_config_dir()
            .join("cadmium")
            .join("settings.json")
    }

    /// The settings in `file`; a missing or unreadable file gives the defaults.
    pub fn load(file: PathBuf) -> SettingsStore {
        let settings = match std::fs::read_to_string(&file) {
            Ok(text) => Settings::from_json(&text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Settings::default(),
            Err(error) => {
                glib::g_warning!(LOG_DOMAIN, "couldn't read {}: {error}", file.display());
                Settings::default()
            }
        };
        SettingsStore {
            settings,
            file: Some(file),
        }
    }

    /// The current settings.
    pub fn get(&self) -> Settings {
        self.settings
    }

    /// Changes the settings, then saves them to the file, if there is one. The change stands even
    /// if saving fails.
    pub fn update(&mut self, change: impl FnOnce(&mut Settings)) -> std::io::Result<()> {
        change(&mut self.settings);
        match &self.file {
            Some(file) => write_settings(file, self.settings),
            None => Ok(()),
        }
    }
}

/// Writes `settings` to `file` through a temporary file beside it (see [`atomic_file::replace`]).
fn write_settings(file: &Path, settings: Settings) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    atomic_file::replace(file, &file.with_extension("json.tmp"), &settings.to_json())
}

#[cfg(test)]
mod tests;
