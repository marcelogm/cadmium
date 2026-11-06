//! Which painting of the examples counts: a draft already shown, or already being painted,
//! is not painted again, and a newer painting supersedes the one under way.

use crate::thumbnails::ThumbnailKey;

/// One painting of the examples, numbered so that a later one supersedes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::brush_editor) struct Order {
    key: ThumbnailKey,
    generation: u64,
}

#[derive(Debug, Default)]
pub(in crate::brush_editor) struct ExampleQueue {
    /// What the picture shows, once a painting finished.
    shown: Option<ThumbnailKey>,
    /// The painting under way.
    painting: Option<Order>,
    generation: u64,
}

impl ExampleQueue {
    /// The order to paint a draft whose strokes depend on `key`; `None` when it is being
    /// painted, or shown with nothing else under way.
    pub(in crate::brush_editor) fn fire(&mut self, key: ThumbnailKey) -> Option<Order> {
        let under_way = self.painting.map(|order| order.key);
        if under_way == Some(key) || (under_way.is_none() && self.shown == Some(key)) {
            return None;
        }
        self.generation += 1;
        let order = Order {
            key,
            generation: self.generation,
        };
        self.painting = Some(order);
        Some(order)
    }

    /// Whether `order` is the painting under way; a superseded one stops.
    pub(in crate::brush_editor) fn is_current(&self, order: Order) -> bool {
        self.painting == Some(order)
    }

    /// `order` finished painting: true when its picture is the one to show.
    pub(in crate::brush_editor) fn finished(&mut self, order: Order) -> bool {
        if !self.is_current(order) {
            return false;
        }
        self.painting = None;
        self.shown = Some(order.key);
        true
    }

    /// `order` failed: nothing is under way any more.
    pub(in crate::brush_editor) fn failed(&mut self, order: Order) {
        if self.is_current(order) {
            self.painting = None;
        }
    }
}

#[cfg(test)]
mod tests;
