use std::rc::{Rc, Weak};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

use super::CadmiumWindow;
use crate::brush_panel::BrushPanel;
use crate::canvas_widget::CadmiumCanvas;
use crate::dialogs::announce_gpu_lost;
use crate::lab_panel::LabPanel;
use crate::state::SharedState;
use crate::tray_view::TrayView;

const ZOOM_PERCENT: f32 = 100.0;
const COPY_PRESENTATION_TOAST: &str = "Presenting by copy (dmabuf unavailable)";
const NOTHING_TO_PICK_TOAST: &str = "Nothing to pick up here";
const HISTORY_DISABLED_TOAST: &str = "Undo unavailable: not enough video memory";

impl CadmiumWindow {
    pub(super) fn attach_state(&self, state: SharedState) {
        state.borrow_mut().set_tray_tool(self.imp().tray_tool.get());
        state.borrow_mut().set_ui_tool(self.tool());
        let canvas = CadmiumCanvas::new(state);
        self.follow_canvas(&canvas);
        self.imp().canvas_slot.append(&canvas);
        let state = canvas.state();
        if self.imp().canvas.set(canvas).is_err() {
            unreachable!("the canvas is attached once");
        }
        let tray = self.attach_tray(state.clone());
        self.attach_brush_panel(state.clone());
        let lab_panel = LabPanel::new(state);
        self.imp().lab_panel_slot.append(lab_panel.widget());
        self.show_pen_readouts(&lab_panel, &tray);
        self.connect_refresh_panels(Rc::new(move || lab_panel.refresh()));
        self.sync_history_actions();
        self.announce_if_history_disabled();
        self.restore_tray_layout();
        self.sync_load_toggle();
    }

    fn follow_canvas(&self, canvas: &CadmiumCanvas) {
        canvas.connect_view_changed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |canvas| window.show_zoom(canvas.zoom())
        ));
        canvas.connect_colour_picked(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, found_paint| window.after_colour_pick(found_paint)
        ));
        canvas.connect_released(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| window.after_contact()
        ));
        canvas.connect_presented(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| {
                window.sync_history_actions();
                window.refresh_if_brush_measured();
            }
        ));
        canvas.connect_gpu_lost(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| window.announce_gpu_lost()
        ));
        canvas.connect_presentation_fallback(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| window.announce_copy_presentation()
        ));
    }

    fn attach_brush_panel(&self, state: SharedState) {
        let brush_panel = BrushPanel::new(self, state);
        self.imp().brush_panel_slot.append(brush_panel.widget());
        self.connect_refresh_panels(Rc::new(move || brush_panel.refresh()));
    }

    fn show_pen_readouts(&self, lab_panel: &Rc<LabPanel>, tray: &TrayView) {
        let canvas = self.canvas();
        let pen_panel = Rc::downgrade(lab_panel);
        let show_pen: Rc<dyn Fn()> = Rc::new(move || show_pen_of(&pen_panel));
        canvas.connect_presented(glib::clone!(
            #[strong]
            show_pen,
            move |_| show_pen()
        ));
        canvas.connect_pen_observed(glib::clone!(
            #[strong]
            show_pen,
            move |_| show_pen()
        ));
        tray.connect_presented(glib::clone!(
            #[strong]
            show_pen,
            move |_| show_pen()
        ));
        tray.connect_pen_observed(move |_| show_pen());
    }

    fn attach_tray(&self, state: SharedState) -> TrayView {
        let tray = TrayView::new(state);
        tray.connect_presentation_fallback(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| window.announce_copy_presentation()
        ));
        tray.connect_colour_picked(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, found_paint| window.after_colour_pick(found_paint)
        ));
        tray.connect_released(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| window.after_contact()
        ));
        // The deferred brush read can land on a tray frame.
        tray.connect_presented(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| window.refresh_if_brush_measured()
        ));
        self.imp().tray_slot.append(&tray);
        if self.imp().tray.set(tray.clone()).is_err() {
            unreachable!("the tray is attached once");
        }
        tray
    }

    pub(crate) fn announce_copy_presentation(&self) {
        if !self.imp().copy_presentation_announced.replace(true) {
            self.show_toast(COPY_PRESENTATION_TOAST);
        }
    }

    /// Refreshes the panels once a deferred brush read has landed in a frame.
    fn refresh_if_brush_measured(&self) {
        let measured = self
            .state()
            .try_borrow_mut()
            .is_ok_and(|mut state| state.take_brush_measured());
        if measured {
            self.refresh_panels();
        }
    }

    fn after_colour_pick(&self, found_paint: bool) {
        if found_paint {
            self.refresh_panels();
        } else {
            self.show_toast(NOTHING_TO_PICK_TOAST);
        }
    }

    pub(super) fn after_contact(&self) {
        self.refresh_panels();
        self.sync_history_actions();
    }

    pub(super) fn announce_if_history_disabled(&self) {
        let enabled = self.state().borrow().history_is_enabled();
        if !enabled && !self.imp().history_disabled_announced.replace(true) {
            self.show_toast(HISTORY_DISABLED_TOAST);
        }
    }

    /// Announces a lost GPU once, however many views see it, closing the
    /// editor first so the alert is not stacked on it. The brush editor calls it too: its
    /// example sheet has no view whose tick would see the loss.
    pub(crate) fn announce_gpu_lost(&self) {
        if self.imp().gpu_lost_announced.replace(true) {
            return;
        }
        self.close_editor();
        announce_gpu_lost(
            self,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move || {
                    window.imp().closing.borrow_mut().confirm();
                    window.close();
                }
            ),
        );
    }

    fn show_zoom(&self, zoom: f32) {
        let text = format!("{:.0}%", zoom * ZOOM_PERCENT);
        let label = &self.imp().zoom_label;
        if label.label() != text {
            label.set_label(&text);
        }
    }
}

fn show_pen_of(panel: &Weak<LabPanel>) {
    if let Some(panel) = panel.upgrade() {
        panel.show_pen();
    }
}
