pub(crate) const CLEAN_LABEL: &str = "Clean";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ToolWording {
    pub(crate) pigments: &'static str,
    pub(crate) loaded_title: &'static str,
    pub(crate) loaded_tooltip: &'static str,
    pub(crate) level_tooltip: &'static str,
    pub(crate) clean: &'static str,
    pub(crate) clean_button: &'static str,
    pub(crate) size_title: &'static str,
    pub(crate) size_subtitle: Option<&'static str>,
    pub(crate) reload_hint: &'static str,
    pub(crate) clean_each_hint: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VisibleControls {
    pub(crate) pigments: bool,
    pub(crate) loaded: bool,
    pub(crate) size: bool,
    pub(crate) load: bool,
    pub(crate) medium: bool,
    pub(crate) plate_gap: bool,
    pub(crate) shape: bool,
    pub(crate) stroke_loading: bool,
    pub(crate) clean: bool,
    pub(crate) hint: bool,
}

impl VisibleControls {
    pub(crate) fn tool_group(self) -> bool {
        self.size || self.loaded || self.shape
    }
}

pub(super) const NO_CONTROLS: VisibleControls = VisibleControls {
    pigments: false,
    loaded: false,
    size: false,
    load: false,
    medium: false,
    plate_gap: false,
    shape: false,
    stroke_loading: false,
    clean: false,
    hint: false,
};
