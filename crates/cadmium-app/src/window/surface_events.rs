use std::rc::Rc;

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
const COPY_PRESENTATION_TOAST: &str = "Apresentação por cópia (dmabuf indisponível)";
const NOTHING_TO_PICK_TOAST: &str = "Nada para pegar aqui";
const HISTORY_DISABLED_TOAST: &str = "Desfazer indisponível: memória de vídeo insuficiente";

impl CadmiumWindow {
    #[expect(clippy::excessive_nesting, clippy::too_many_lines)]
    pub(super) fn attach_state(&self, state: SharedState) {
        state.borrow_mut().set_ui_tool(self.tool());
        let canvas = CadmiumCanvas::new(state);
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
            move |_| window.sync_history_actions()
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
        self.imp().canvas_slot.append(&canvas);
        let state = canvas.state();
        if self.imp().canvas.set(canvas).is_err() {
            unreachable!("the canvas is attached once");
        }
        let tray = self.attach_tray(state.clone());
        let brush_panel = BrushPanel::new(self, state.clone());
        self.imp().brush_panel_slot.append(brush_panel.widget());
        self.connect_refresh_panels(Rc::new(move || brush_panel.refresh()));
        let lab_panel = LabPanel::new(state);
        self.imp().lab_panel_slot.append(lab_panel.widget());
        let canvas = self.canvas();
        let pen_panel = Rc::downgrade(&lab_panel);
        let show_pen: Rc<dyn Fn()> = Rc::new(move || {
            if let Some(panel) = pen_panel.upgrade() {
                panel.show_pen();
            }
        });
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
        self.connect_refresh_panels(Rc::new(move || lab_panel.refresh()));
        self.sync_history_actions();
        self.announce_if_history_disabled();
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
        self.imp().tray_slot.append(&tray);
        tray
    }

    fn announce_copy_presentation(&self) {
        if !self.imp().copy_presentation_announced.replace(true) {
            self.show_toast(COPY_PRESENTATION_TOAST);
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

    fn announce_gpu_lost(&self) {
        announce_gpu_lost(
            self,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move || {
                    window.imp().close_confirmed.set(true);
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
