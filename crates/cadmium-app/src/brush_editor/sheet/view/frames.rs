//! When the sheet presents a frame: while it paints or settles, on request, or on a resize.

use std::cell::RefCell;

use cadmium_studio::TestSheet;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::TestSheetView;
use crate::handlers::{announce_gpu_loss_once, run_handlers};
use crate::presenter::FramePresenter;
use crate::state::AppState;
use crate::surface_scale::viewport_px;

impl TestSheetView {
    /// Presents a frame even while the sheet rests, as after Clear.
    pub(in crate::brush_editor) fn queue_present(&self) {
        self.imp().needs_present.set(true);
        self.request_frame();
    }

    pub(super) fn request_frame(&self) {
        let imp = self.imp();
        if imp.tick.borrow().is_some() {
            return;
        }
        let tick = self.add_tick_callback(|view, _| view.advance_frame());
        imp.tick.replace(Some(tick));
    }

    fn advance_frame(&self) -> glib::ControlFlow {
        self.present_if_needed();
        if self.wants_frame() {
            return glib::ControlFlow::Continue;
        }
        self.imp().tick.take();
        glib::ControlFlow::Break
    }

    fn frame_needed(&self, state: &AppState, presenter: &FramePresenter) -> bool {
        let (width_px, height_px) = viewport_px(self);
        if !self.is_mapped() || width_px == 0 || height_px == 0 || state.gpu_is_lost() {
            return false;
        }
        let now_ms = state.now_ms();
        let painting = self.sheet().map(|sheet| sheet_needs_frames(&sheet, now_ms));
        super::frame_needed(
            painting,
            self.imp().needs_present.get(),
            presenter.matches_size(width_px, height_px),
        )
    }

    fn wants_frame(&self) -> bool {
        let imp = self.imp();
        let Some(link) = imp.link.get() else {
            return false;
        };
        let (Ok(state), Ok(presenter)) = (link.state.try_borrow(), imp.presenter.try_borrow())
        else {
            return true;
        };
        self.frame_needed(&state, &presenter)
    }

    fn present_if_needed(&self) {
        if self.present_texture() {
            self.announce_presented();
        }
        self.announce_if_gpu_lost();
    }

    fn announce_presented(&self) {
        let imp = self.imp();
        let fell_back = imp.presenter.borrow_mut().take_fallback_notice();
        if fell_back {
            run_handlers(&imp.fallback_handlers, self);
        }
    }

    fn announce_if_gpu_lost(&self) {
        let imp = self.imp();
        let state = imp.link.get().map(|link| &link.state);
        announce_gpu_loss_once(state, &imp.gpu_lost_announced, &imp.gpu_lost_handlers, self);
    }

    fn present_texture(&self) -> bool {
        let imp = self.imp();
        let (Some(link), Some(native)) = (imp.link.get(), self.native()) else {
            return false;
        };
        let Ok(mut state) = link.state.try_borrow_mut() else {
            return false;
        };
        let mut presenter = imp.presenter.borrow_mut();
        if !self.frame_needed(&state, &presenter) {
            return false;
        }
        let (width_px, height_px) = viewport_px(self);
        let Some(texture) = presenter.present(&mut state, &native, width_px, height_px) else {
            return false;
        };
        imp.needs_present.set(false);
        imp.texture.replace(Some(texture));
        self.queue_draw();
        true
    }
}

/// Whether the sheet still changes at `clock_ms`; a sheet busy in a call right now counts
/// as changing.
fn sheet_needs_frames(sheet: &RefCell<TestSheet>, clock_ms: f64) -> bool {
    match sheet.try_borrow() {
        Ok(sheet) => sheet.needs_frames(clock_ms),
        Err(_) => true,
    }
}
