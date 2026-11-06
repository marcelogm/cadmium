use std::time::{Duration, Instant};

use cadmium_engine::gpu::GpuContext;
use gtk::prelude::*;
use gtk::{gdk, glib};

use super::FramePresenter;
use super::mode::PresentMode;
use crate::LOG_DOMAIN;
use crate::dmabuf::{DmabufError, DmabufPool, ModifierPolicy};
use crate::state::AppState;

const RESIZE_SETTLE: Duration = Duration::from_millis(150);

#[derive(Debug, Default)]
pub(super) struct ResizeDebounce {
    pending: Option<((u32, u32), Instant)>,
}

impl ResizeDebounce {
    fn has_settled(&mut self, size: (u32, u32), now: Instant) -> bool {
        match self.pending {
            Some((pending, since)) if pending == size => now.duration_since(since) >= RESIZE_SETTLE,
            _ => {
                self.pending = Some((size, now));
                false
            }
        }
    }

    pub(super) fn clear(&mut self) {
        self.pending = None;
    }
}

impl FramePresenter {
    pub(super) fn present_by_dmabuf(
        &mut self,
        state: &mut AppState,
        native: &gtk::Native,
        policy: ModifierPolicy,
        size: (u32, u32),
    ) -> Result<Option<gdk::Texture>, DmabufError> {
        let gpu = state.gpu.clone();
        let display = native.display();
        if self.pool_needs_replacing(size) {
            self.replace_pool(&gpu, &display, policy, size)?;
        }
        let Some(pool) = self.pool.as_mut() else {
            return Ok(None);
        };
        let source = self.source.clone();
        pool.present_with(&gpu, &display, |encoder, target| {
            source.encode(state, &gpu, target, encoder);
        })
    }

    fn pool_needs_replacing(&mut self, size: (u32, u32)) -> bool {
        match self.pool.as_ref().map(DmabufPool::size) {
            None => true,
            Some(pool_size) if pool_size == size => {
                self.resize.clear();
                false
            }
            Some(_) => self.resize.has_settled(size, Instant::now()),
        }
    }

    fn replace_pool(
        &mut self,
        gpu: &GpuContext,
        display: &gdk::Display,
        policy: ModifierPolicy,
        size: (u32, u32),
    ) -> Result<(), DmabufError> {
        self.pool = None;
        self.resize.clear();
        let pool = DmabufPool::new(gpu, display, size.0, size.1, policy)?;
        self.announce_pool(&pool, policy);
        self.last_modifier = Some(pool.modifier());
        self.pool = Some(pool);
        Ok(())
    }

    fn announce_pool(&mut self, pool: &DmabufPool, policy: ModifierPolicy) {
        if !self.pool_announced {
            glib::g_message!(
                LOG_DOMAIN,
                "presenting by {} with modifier 0x{:016x}: {}",
                PresentMode::Dmabuf(policy).name(),
                pool.modifier(),
                self.mode_reason
            );
            self.pool_announced = true;
        }
    }
}

#[cfg(test)]
mod tests;
