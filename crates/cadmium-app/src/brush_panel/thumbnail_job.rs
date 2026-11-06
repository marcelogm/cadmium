//! Fills the rack's thumbnails one brush per idle tick, so start-up never stalls.

use std::rc::{Rc, Weak};

use cadmium_studio::{BrushStudio, StudioError};
use gtk::{gdk, glib};

use super::{BrushPanel, rack};
use crate::LOG_DOMAIN;
use crate::thumbnails::{self, ThumbnailKey, rack_indices};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ThumbnailJob {
    Idle,
    Pending,
    /// The studio failed once; the rack keeps its names only.
    Failed,
}

impl ThumbnailJob {
    /// Whether a refresh that finds a thumbnail missing starts the idle job;
    /// never while the studio is open, which pauses the job.
    fn starts(self, anything_missing: bool, studio_open: bool) -> bool {
        self == ThumbnailJob::Idle && anything_missing && !studio_open
    }

    fn keeps_running(self) -> bool {
        self == ThumbnailJob::Pending
    }
}

/// How one idle tick ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TickOutcome {
    NothingMissing,
    Painted,
    Failed,
    /// The studio is open; the job waits for it to close.
    Paused,
}

impl TickOutcome {
    fn job_after(self) -> ThumbnailJob {
        match self {
            TickOutcome::NothingMissing => ThumbnailJob::Idle,
            TickOutcome::Painted => ThumbnailJob::Pending,
            TickOutcome::Failed => ThumbnailJob::Failed,
            TickOutcome::Paused => ThumbnailJob::Idle,
        }
    }
}

impl BrushPanel {
    pub(super) fn cached_thumbnails(&self, keys: &[ThumbnailKey]) -> Vec<Option<gdk::Texture>> {
        let cache = self.thumbnails.borrow();
        keys.iter().map(|key| cache.get(key).cloned()).collect()
    }

    /// Starts the idle job when a shown brush has no thumbnail and none is running.
    pub(super) fn schedule_thumbnails(self: &Rc<Self>, keys: &[ThumbnailKey]) {
        let anything_missing = {
            let cache = self.thumbnails.borrow();
            keys.iter().any(|key| cache.get(key).is_none())
        };
        let studio_open = self.state.borrow().studio_open();
        if !self
            .thumbnail_job
            .get()
            .starts(anything_missing, studio_open)
        {
            return;
        }
        self.set_thumbnail_job(ThumbnailJob::Pending);
        let panel = Rc::downgrade(self);
        glib::idle_add_local(move || thumbnail_tick(&panel));
    }

    /// Pauses while the studio is open; closing it refreshes the panels, which
    /// starts the job again.
    fn tick(&self) -> TickOutcome {
        if self.state.borrow().studio_open() {
            return TickOutcome::Paused;
        }
        self.paint_next_thumbnail()
    }

    /// Paints one missing thumbnail; holds no borrow of the state or the cache meanwhile.
    fn paint_next_thumbnail(&self) -> TickOutcome {
        let Some(brush) = thumbnails::next_missing(&self.state, &self.thumbnails.borrow()) else {
            return TickOutcome::NothingMissing;
        };
        let painted = self.take_studio().and_then(|mut studio| {
            let painted = thumbnails::render(&self.state, &mut studio, &brush);
            self.studio.replace(Some(studio));
            painted
        });
        match painted {
            Ok((key, thumbnail)) => {
                self.show_thumbnail(key, thumbnails::texture(thumbnail));
                TickOutcome::Painted
            }
            Err(error) => {
                glib::g_warning!(LOG_DOMAIN, "no brush thumbnails: {error}");
                TickOutcome::Failed
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
        if let Some((_, keys)) = self.shown_brushes.borrow().as_ref() {
            rack_indices(keys, key)
                .for_each(|index| rack::show_thumbnail(&self.rack.flow, index, &texture));
        }
        self.thumbnails.borrow_mut().insert(key, texture);
    }

    fn set_thumbnail_job(&self, job: ThumbnailJob) {
        self.thumbnail_job.set(job);
        if let Some(window) = self.window.upgrade() {
            window.set_thumbnails_pending(job == ThumbnailJob::Pending);
        }
    }
}

fn thumbnail_tick(panel: &Weak<BrushPanel>) -> glib::ControlFlow {
    let Some(panel) = panel.upgrade() else {
        return glib::ControlFlow::Break;
    };
    let job = panel.tick().job_after();
    panel.set_thumbnail_job(job);
    if job.keeps_running() {
        glib::ControlFlow::Continue
    } else {
        glib::ControlFlow::Break
    }
}

#[cfg(test)]
mod tests;
