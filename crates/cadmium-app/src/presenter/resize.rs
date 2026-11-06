use std::time::{Duration, Instant};

const RESIZE_SETTLE: Duration = Duration::from_millis(150);

/// Waits for the frame size to hold still before the pool is rebuilt.
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

    /// Whether the pool, of `pool_size` (none yet when `None`), must be rebuilt for
    /// frames of `size`: at once when there is none, and after the size holds still when
    /// it differs. A pool of the right size also forgets any pending resize.
    pub(super) fn needs_new_pool(
        &mut self,
        pool_size: Option<(u32, u32)>,
        size: (u32, u32),
        now: Instant,
    ) -> bool {
        match pool_size {
            None => true,
            Some(pool_size) if pool_size == size => {
                self.clear();
                false
            }
            Some(_) => self.has_settled(size, now),
        }
    }

    pub(super) fn clear(&mut self) {
        self.pending = None;
    }
}

#[cfg(test)]
mod tests;
