use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

use cadmium_engine::canvas::CanvasSpec;

use super::CadmiumWindow;
use crate::dialogs::confirm_discard;
use crate::files::OUT_OF_VIDEO_MEMORY;
use crate::new_painting_dialog::NewPaintingDialog;

const CANNOT_START_PAINTING: &str = "Não foi possível criar a pintura";

impl CadmiumWindow {
    pub(super) fn open_new_painting_dialog(&self) {
        if self.visible_dialog().is_some() {
            return;
        }
        let dialog = {
            let state = self.state();
            let state = state.borrow();
            NewPaintingDialog::new(&state.session.db, &state.session.canvas.spec)
        };
        dialog.connect_create(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |dialog, spec| {
                if !window.work_would_be_lost() {
                    dialog.close();
                    window.start_painting(spec);
                    return;
                }
                confirm_discard(
                    dialog,
                    glib::clone!(
                        #[weak]
                        window,
                        #[weak]
                        dialog,
                        move || {
                            dialog.close();
                            window.start_painting(spec.clone());
                        }
                    ),
                );
            }
        ));
        dialog.present(Some(self));
    }

    fn start_painting(&self, spec: CanvasSpec) {
        let started = self.state().borrow_mut().start_painting(spec);
        self.after_document_replaced();
        if started.is_err() {
            self.show_toast(&format!("{CANNOT_START_PAINTING}: {OUT_OF_VIDEO_MEMORY}"));
        }
    }

    pub fn after_document_replaced(&self) {
        let generation = &self.imp().document_generation;
        generation.set(generation.get() + 1);
        let canvas = self.canvas();
        canvas.zoom_fit();
        canvas.queue_present();
        self.sync_history_actions();
        self.refresh_panels();
        self.show_document_title();
        self.announce_if_history_disabled();
    }
}
