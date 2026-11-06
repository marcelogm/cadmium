use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};

use crate::LOG_DOMAIN;
use crate::canvas_widget::CadmiumCanvas;
use crate::state::SharedState;
use crate::tools::UiTool;

mod actions;
mod document;
mod imp;
mod surface_events;
mod tool_toggles;

pub type PanelRefresher = Rc<dyn Fn()>;

glib::wrapper! {
    pub struct CadmiumWindow(ObjectSubclass<imp::CadmiumWindow>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl CadmiumWindow {
    pub fn new(app: &adw::Application, state: SharedState) -> Self {
        let window: CadmiumWindow = glib::Object::builder().property("application", app).build();
        window.attach_state(state);
        window
    }

    pub fn tool(&self) -> UiTool {
        self.imp().tool.get()
    }

    pub fn canvas(&self) -> CadmiumCanvas {
        self.imp()
            .canvas
            .get()
            .expect("window is created with its canvas")
            .clone()
    }

    pub fn state(&self) -> SharedState {
        self.canvas().state()
    }

    pub fn show_toast(&self, title: &str) {
        self.imp().toast_overlay.add_toast(adw::Toast::new(title));
    }

    pub fn connect_refresh_panels(&self, refresher: PanelRefresher) {
        self.imp().panel_refreshers.borrow_mut().push(refresher);
    }

    pub fn refresh_panels(&self) {
        let refreshers = self.imp().panel_refreshers.borrow().clone();
        for refresh in refreshers {
            refresh();
        }
    }

    /// Whether the brush rack is still painting thumbnails; smoke screenshots wait for it.
    pub fn thumbnails_pending(&self) -> bool {
        self.imp().thumbnails_pending.get()
    }

    /// Set by the brush panel's thumbnail job, only so smoke screenshots can wait for it.
    pub fn set_thumbnails_pending(&self, pending: bool) {
        self.imp().thumbnails_pending.set(pending);
    }

    pub fn has_editor(&self) -> bool {
        self.imp().editor.borrow().is_some()
    }

    pub fn keep_editor(&self, editor: &adw::Dialog) {
        self.imp().editor.replace(Some(editor.clone()));
    }

    fn close_editor(&self) {
        let editor = self.imp().editor.take();
        if let Some(editor) = editor {
            editor.force_close();
        }
    }

    /// Closes the editor's studio and refreshes the panels, which restarts the
    /// thumbnails it paused; on a lost GPU the panels are left alone, so the
    /// thumbnail job does not start on a dead device.
    pub fn after_editor_closed(&self) {
        self.imp().editor.take();
        let gpu_lost = {
            let state = self.state();
            let mut state = state.borrow_mut();
            state.close_studio();
            state.gpu_is_lost()
        };
        if !gpu_lost {
            self.refresh_panels();
        }
    }

    pub fn show_sidebar_page(&self, name: &str) {
        let imp = self.imp();
        imp.split_view.set_show_sidebar(true);
        if imp.sidebar_stack.child_by_name(name).is_some() {
            imp.sidebar_stack.set_visible_child_name(name);
        } else {
            glib::g_warning!(LOG_DOMAIN, "no sidebar page named {name:?}");
        }
    }

    pub fn show_tray(&self) {
        let imp = self.imp();
        imp.split_view.set_show_sidebar(true);
        imp.tray_sheet.set_open(true);
    }

    fn keep_window_buttons_reachable(&self) {
        let imp = self.imp();
        imp.split_view
            .bind_property(
                "show-sidebar",
                &*imp.content_header,
                "show-end-title-buttons",
            )
            .invert_boolean()
            .sync_create()
            .build();
    }
}
