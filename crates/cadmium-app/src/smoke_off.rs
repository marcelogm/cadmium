use crate::window::CadmiumWindow;

pub struct SmokeConfig;

impl SmokeConfig {
    pub fn from_env() -> Option<SmokeConfig> {
        None
    }

    pub fn run(self, _app: &adw::Application, _window: &CadmiumWindow) {}
}
