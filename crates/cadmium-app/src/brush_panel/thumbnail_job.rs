//! Fills the rack's thumbnails one brush per idle tick, so start-up never stalls.

use std::rc::{Rc, Weak};

use cadmium_studio::{BrushStudio, StudioError};
use gtk::{gdk, glib};

use super::{BrushPanel, ThumbnailJob, rack};
use crate::LOG_DOMAIN;
use crate::thumbnails::{self, ThumbnailKey};

impl BrushPanel {
    pub(super) fn cached_thumbnails(&self, keys: &[ThumbnailKey]) -> Vec<Option<gdk::Texture>> {
        let cache = self.thumbnails.borrow();
        keys.iter().map(|key| cache.get(key).cloned()).collect()
    }

    /// Starts the idle job when a shown brush has no thumbnail and none is running.
    pub(super) fn schedule_thumbnails(self: &Rc<Self>, keys: &[ThumbnailKey]) {
        if self.thumbnail_job.get() != ThumbnailJob::Idle {
            return;
        }
        let cache = self.thumbnails.borrow();
        if keys.iter().all(|key| cache.get(key).is_some()) {
            return;
        }
        drop(cache);
        self.set_thumbnail_job(ThumbnailJob::Pending);
        let panel = Rc::downgrade(self);
        glib::idle_add_local(move || thumbnail_tick(&panel));
    }

    /// Paints one missing thumbnail; holds no borrow of the state or the cache meanwhile.
    fn paint_next_thumbnail(&self) -> glib::ControlFlow {
        let Some(brush) = thumbnails::next_missing(&self.state, &self.thumbnails.borrow()) else {
            return self.stop_thumbnails(ThumbnailJob::Idle);
        };
        let painted = self.take_studio().and_then(|mut studio| {
            let painted = thumbnails::render(&self.state, &mut studio, &brush);
            self.studio.replace(Some(studio));
            painted
        });
        match painted {
            Ok((key, thumbnail)) => {
                self.show_thumbnail(key, thumbnails::texture(thumbnail));
                glib::ControlFlow::Continue
            }
            Err(error) => {
                glib::g_warning!(LOG_DOMAIN, "no brush thumbnails: {error}");
                self.stop_thumbnails(ThumbnailJob::Failed)
            }
        }
    }

    fn take_studio(&self) -> Result<BrushStudio, StudioError> {
        match self.studio.take() {
            Some(studio) => Ok(studio),
            None => thumbnails::open_studio(&self.state),
        }
    }

    fn show_thumbnail(&self, key: ThumbnailKey, texture: gdk::Texture) {
        let shown = self.shown_brushes.borrow();
        let keys = shown.as_ref().map_or(&[][..], |(_, keys)| keys.as_slice());
        for (index, _) in keys.iter().enumerate().filter(|(_, shown)| **shown == key) {
            rack::show_thumbnail(&self.rack.flow, index, &texture);
        }
        drop(shown);
        self.thumbnails.borrow_mut().insert(key, texture);
    }

    fn stop_thumbnails(&self, job: ThumbnailJob) -> glib::ControlFlow {
        self.set_thumbnail_job(job);
        glib::ControlFlow::Break
    }

    fn set_thumbnail_job(&self, job: ThumbnailJob) {
        self.thumbnail_job.set(job);
        if let Some(window) = self.window.upgrade() {
            window.set_thumbnails_pending(job == ThumbnailJob::Pending);
        }
    }
}

fn thumbnail_tick(panel: &Weak<BrushPanel>) -> glib::ControlFlow {
    match panel.upgrade() {
        Some(panel) => panel.paint_next_thumbnail(),
        None => glib::ControlFlow::Break,
    }
}
