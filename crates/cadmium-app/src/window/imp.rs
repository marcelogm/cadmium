use std::cell::{Cell, OnceCell, RefCell};

use super::tray_divider::TrayDivider;
use super::*;
use crate::tools::TrayTool;
use crate::writes::Closing;

/// The open brush editor's dialog, and what stops the editor before the window closes it.
pub struct KeptEditor {
    pub dialog: adw::Dialog,
    pub stop: Box<dyn Fn()>,
}

#[derive(Default, gtk::CompositeTemplate)]
#[template(resource = "/io/github/marcelogm/Cadmium/window.ui")]
pub struct CadmiumWindow {
    #[template_child]
    pub split_view: TemplateChild<adw::OverlaySplitView>,
    #[template_child]
    pub content_header: TemplateChild<adw::HeaderBar>,
    #[template_child]
    pub tool_group: TemplateChild<adw::ToggleGroup>,
    #[template_child]
    pub toast_overlay: TemplateChild<adw::ToastOverlay>,
    #[template_child]
    pub canvas_overlay: TemplateChild<gtk::Overlay>,
    #[template_child]
    pub canvas_slot: TemplateChild<gtk::Box>,
    #[template_child]
    pub zoom_osd: TemplateChild<gtk::Box>,
    #[template_child]
    pub zoom_label: TemplateChild<gtk::Label>,
    #[template_child]
    pub primary_menu: TemplateChild<gtk::MenuButton>,
    #[template_child]
    pub sidebar_stack: TemplateChild<adw::ViewStack>,
    #[template_child]
    pub tray_paned: TemplateChild<gtk::Paned>,
    #[template_child]
    pub tray_pane: TemplateChild<gtk::Box>,
    #[template_child]
    pub tray_tool_group: TemplateChild<adw::ToggleGroup>,
    #[template_child]
    pub tray_toggle: TemplateChild<gtk::ToggleButton>,
    #[template_child]
    pub tray_slot: TemplateChild<gtk::Box>,
    #[template_child]
    pub brush_panel_slot: TemplateChild<gtk::Box>,
    #[template_child]
    pub lab_panel_slot: TemplateChild<gtk::Box>,
    pub tool: Cell<UiTool>,
    pub tray_tool: Cell<TrayTool>,
    pub(super) tray_divider: Cell<TrayDivider>,
    /// Whether placing the divider is already queued for the paned's new height.
    pub(super) divider_queued: Cell<bool>,
    pub canvas: OnceCell<CadmiumCanvas>,
    /// Whether the window has claimed the Space press now held down.
    pub space_claimed: Cell<bool>,
    pub panel_refreshers: RefCell<Vec<PanelRefresher>>,
    pub closing: RefCell<Closing>,
    pub document_generation: Cell<u64>,
    pub write_wait_timeout: RefCell<Option<glib::SourceId>>,
    pub write_wait_alert: RefCell<Option<adw::AlertDialog>>,
    pub history_disabled_announced: Cell<bool>,
    pub copy_presentation_announced: Cell<bool>,
    pub gpu_lost_announced: Cell<bool>,
    /// The open brush editor, so a lost GPU can close it.
    pub editor: RefCell<Option<KeptEditor>>,
    /// The rack is still painting thumbnails; exists so smoke screenshots wait for them.
    pub thumbnails_pending: Cell<bool>,
    /// The brush editor is still painting its example strokes; smoke screenshots wait for them.
    pub examples_pending: Cell<bool>,
}

#[glib::object_subclass]
impl ObjectSubclass for CadmiumWindow {
    const NAME: &'static str = "CadmiumWindow";
    type Type = super::CadmiumWindow;
    type ParentType = adw::ApplicationWindow;

    fn class_init(klass: &mut Self::Class) {
        klass.bind_template();
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for CadmiumWindow {
    fn constructed(&self) {
        self.parent_constructed();
        let window = self.obj();
        window.fill_tool_group();
        window.fill_tray_tool_group();
        window.setup_actions();
        window.setup_tool_shortcuts();
        window.setup_digit_keys();
        window.setup_navigation_keys();
        window.follow_tool_group();
        window.follow_tray_tool_group();
        window.keep_window_buttons_reachable();
        self.tool_group
            .set_active_name(Some(UiTool::default().name()));
    }
}

impl WidgetImpl for CadmiumWindow {}
impl WindowImpl for CadmiumWindow {
    fn close_request(&self) -> glib::Propagation {
        self.obj().remember_tray_share();
        if self.obj().may_close() {
            self.parent_close_request()
        } else {
            glib::Propagation::Stop
        }
    }
}
impl ApplicationWindowImpl for CadmiumWindow {}
impl AdwApplicationWindowImpl for CadmiumWindow {}
