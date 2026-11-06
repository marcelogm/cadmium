use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::StudioView;
use crate::handlers::run_handlers;
use crate::presenter::FramePresenter;
use crate::state::AppState;
use crate::surface_scale::viewport_px;

impl StudioView {
    /// Presents a frame even while the studio is paused, as after a camera move.
    pub fn queue_present(&self) {
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
        let playing = state
            .studio()
            .is_some_and(|studio| studio.player.is_playing());
        super::frame_needed(
            state.studio_open(),
            playing,
            self.imp().needs_present.get(),
            presenter.matches_size(width_px, height_px),
        )
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

    fn announce_if_gpu_lost(&self) {
        let imp = self.imp();
        let lost = imp
            .state
            .get()
            .and_then(|state| state.try_borrow().ok().map(|state| state.gpu_is_lost()))
            .unwrap_or(false);
        if lost && !imp.gpu_lost_announced.replace(true) {
            run_handlers(&imp.gpu_lost_handlers, self);
        }
    }

    fn present_if_needed(&self) {
        self.present_texture();
        self.announce_if_gpu_lost();
    }

    fn present_texture(&self) {
        let imp = self.imp();
        let Some(state) = imp.state.get() else {
            return;
        };
        let Some(native) = self.native() else {
            return;
        };
        let Ok(mut state) = state.try_borrow_mut() else {
            return;
        };
        let mut presenter = imp.presenter.borrow_mut();
        if !self.frame_needed(&state, &presenter) {
            return;
        }
        let (width_px, height_px) = viewport_px(self);
        let Some(texture) = presenter.present(&mut state, &native, width_px, height_px) else {
            return;
        };
        imp.needs_present.set(false);
        imp.texture.replace(Some(texture));
        imp.presented_frames.set(imp.presented_frames.get() + 1);
        self.queue_draw();
    }
}
