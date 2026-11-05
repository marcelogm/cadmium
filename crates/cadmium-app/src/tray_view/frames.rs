use cadmium_engine::surface::Surface;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::TrayView;
use super::geometry::tray_screen_zoom;
use crate::handlers::run_handlers;
use crate::presenter::FramePresenter;
use crate::state::AppState;
use crate::surface_scale::viewport_px;

impl TrayView {
    pub fn queue_present(&self) {
        self.imp().needs_present.set(true);
        self.request_frame();
    }

    pub(super) fn request_frame(&self) {
        let imp = self.imp();
        if imp.tick.borrow().is_some() {
            return;
        }
        let tick = self.add_tick_callback(|tray, _| tray.advance_frame());
        imp.tick.replace(Some(tick));
    }

    fn advance_frame(&self) -> glib::ControlFlow {
        self.present_if_needed();
        let ring_moving = self.follow_load_ring();
        if ring_moving || self.wants_frame() {
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
        self.imp().needs_present.get()
            || state.session.tray_has_pending_work()
            || state.session.tray_bristles_settling(state.now_ms())
            || state.holds_on(Surface::Tray)
            || !presenter.matches_size(width_px, height_px)
    }

    fn wants_frame(&self) -> bool {
        let imp = self.imp();
        let Some(state) = imp.state.get() else {
            return false;
        };
        let (Ok(state), Ok(presenter)) = (state.try_borrow(), imp.presenter.try_borrow()) else {
            return true;
        };
        self.frame_needed(&state, &presenter)
    }

    fn present_if_needed(&self) {
        if self.present_texture() {
            self.announce_presented();
        }
    }

    fn announce_presented(&self) {
        let imp = self.imp();
        let fell_back = imp.presenter.borrow_mut().take_fallback_notice();
        if fell_back {
            run_handlers(&imp.fallback_handlers, self);
        }
        run_handlers(&imp.presented_handlers, self);
    }

    fn present_texture(&self) -> bool {
        let imp = self.imp();
        let Some(state) = imp.state.get() else {
            return false;
        };
        let Some(native) = self.native() else {
            return false;
        };
        let Ok(mut state) = state.try_borrow_mut() else {
            return false;
        };
        let mut presenter = imp.presenter.borrow_mut();
        if !self.frame_needed(&state, &presenter) {
            return false;
        }
        let (width_px, height_px) = viewport_px(self);
        state.set_tray_zoom(tray_screen_zoom((width_px, height_px), self.device_scale()));
        let Some(texture) = presenter.present(&mut state, &native, width_px, height_px) else {
            return false;
        };
        imp.needs_present.set(false);
        imp.texture.replace(Some(texture));
        self.queue_draw();
        true
    }
}
