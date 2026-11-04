use std::cell::{Cell, OnceCell, RefCell};

use super::*;
use crate::writes::WritesInFlight;

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
    pub tray_sheet: TemplateChild<adw::BottomSheet>,
    #[template_child]
    pub tray_slot: TemplateChild<gtk::Box>,
    #[template_child]
    pub brush_panel_slot: TemplateChild<gtk::Box>,
    #[template_child]
    pub lab_panel_slot: TemplateChild<gtk::Box>,
    pub tool: Cell<UiTool>,
    pub canvas: OnceCell<CadmiumCanvas>,
    pub panel_refreshers: RefCell<Vec<PanelRefresher>>,
    pub writes_in_flight: RefCell<WritesInFlight>,
    pub document_generation: Cell<u64>,
    pub close_after_writes: Cell<bool>,
    pub write_wait_timeout: RefCell<Option<glib::SourceId>>,
    pub write_wait_alert: RefCell<Option<adw::AlertDialog>>,
    pub abandon_writes: Cell<bool>,
    pub close_confirmed: Cell<bool>,
    pub asking_to_close: Cell<bool>,
    pub history_disabled_announced: Cell<bool>,
    pub copy_presentation_announced: Cell<bool>,
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
        window.setup_actions();
        window.setup_tool_shortcuts();
        window.follow_tool_group();
        window.keep_window_buttons_reachable();
        self.tool_group
            .set_active_name(Some(UiTool::default().name()));
    }
}

impl WidgetImpl for CadmiumWindow {}
impl WindowImpl for CadmiumWindow {
    fn close_request(&self) -> glib::Propagation {
        if self.obj().may_close() {
            self.parent_close_request()
        } else {
            glib::Propagation::Stop
        }
    }
}
impl ApplicationWindowImpl for CadmiumWindow {}
impl AdwApplicationWindowImpl for CadmiumWindow {}
