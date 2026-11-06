use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::StudioView;
use crate::handlers::{announce_gpu_loss_once, run_handlers};
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
        announce_gpu_loss_once(
            imp.state.get(),
            &imp.gpu_lost_announced,
            &imp.gpu_lost_handlers,
            self,
        );
    }

    fn present_if_needed(&self) {
        match self.present_texture() {
            Presented::Frame => self.announce_presented(),
            Presented::StudioClosed => run_handlers(&self.imp().studio_closed_handlers, self),
            Presented::Nothing => {}
        }
        self.drop_frame_if_closed();
        self.announce_if_gpu_lost();
    }

    fn announce_presented(&self) {
        let imp = self.imp();
        let fell_back = imp.presenter.borrow_mut().take_fallback_notice();
        if fell_back {
            run_handlers(&imp.fallback_handlers, self);
        }
    }

    /// Forgets the shown frame once the studio is closed, so a closed studio
    /// shows nothing rather than its last frame.
    fn drop_frame_if_closed(&self) {
        let imp = self.imp();
        let closed = imp
            .state
            .get()
            .and_then(|state| state.try_borrow().ok().map(|state| !state.studio_open()))
            .unwrap_or(false);
        if closed && imp.texture.take().is_some() {
            self.queue_draw();
        }
    }

    fn present_texture(&self) -> Presented {
        let imp = self.imp();
        let Some(state) = imp.state.get() else {
            return Presented::Nothing;
        };
        let Some(native) = self.native() else {
            return Presented::Nothing;
        };
        let Ok(mut state) = state.try_borrow_mut() else {
            return Presented::Nothing;
        };
        let mut presenter = imp.presenter.borrow_mut();
        if !self.frame_needed(&state, &presenter) {
            return Presented::Nothing;
        }
        let (width_px, height_px) = viewport_px(self);
        let texture = presenter.present(&mut state, &native, width_px, height_px);
        if !state.studio_open() {
            // The studio failed mid-frame: nothing was drawn into the texture.
            return Presented::StudioClosed;
        }
        let Some(texture) = texture else {
            return Presented::Nothing;
        };
        imp.needs_present.set(false);
        imp.texture.replace(Some(texture));
        imp.presented_frames.set(imp.presented_frames.get() + 1);
        self.queue_draw();
        Presented::Frame
    }
}

/// What one present did.
enum Presented {
    Nothing,
    Frame,
    /// The studio closed on an error while presenting; nothing was shown.
    StudioClosed,
}
