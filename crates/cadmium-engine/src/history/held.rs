use crate::engine::{HeldPaint, WetOilEngine};
use crate::gpu::GpuContext;

use super::{History, Step};

/// The paint the tool held around one step: before it, and, once the step was undone,
/// after it. Good only while the tool stays in the load epoch the step was painted in.
pub(super) struct HeldRecord {
    before: HeldPaint,
    after: Option<HeldPaint>,
    load_epoch: u64,
}

impl HeldRecord {
    fn bytes(&self) -> u64 {
        self.before.bytes() + self.after.as_ref().map_or(0, HeldPaint::bytes)
    }

    fn still_fits(&self, engine: &WetOilEngine) -> bool {
        self.load_epoch == engine.load_epoch()
    }

    pub(super) fn encode_undo(
        &mut self,
        gpu: &GpuContext,
        engine: &mut WetOilEngine,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if !self.still_fits(engine) {
            return;
        }
        if self.after.is_none() {
            self.after = engine.encode_capture_held_paint(gpu, encoder);
        }
        engine.encode_restore_held_paint(&self.before, encoder);
    }

    pub(super) fn encode_redo(
        &self,
        engine: &mut WetOilEngine,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if !self.still_fits(engine) {
            return;
        }
        if let Some(after) = &self.after {
            engine.encode_restore_held_paint(after, encoder);
        }
    }
}

impl Step {
    fn held_bytes(&self) -> u64 {
        self.held.as_ref().map_or(0, HeldRecord::bytes)
    }

    fn awaits_held_paint(&self) -> bool {
        !self.removes_wet_paint && self.complete && !self.held_asked
    }
}

impl History {
    /// Whether the open step is a stroke's and has not yet been offered the paint the
    /// tool held before it.
    pub fn awaits_held_paint(&self) -> bool {
        self.is_enabled()
            && self.held_paint_budget_bytes > 0
            && self.current.as_ref().is_some_and(Step::awaits_held_paint)
    }

    /// Gives the open step the paint the tool held before its first dabs; `None` when it
    /// could not be captured, and the step then restores the canvas alone. Past the
    /// held-paint budget the oldest steps go, whole.
    pub fn hold_before(&mut self, before: Option<HeldPaint>, load_epoch: u64) {
        let Some(step) = self.current.as_mut() else {
            return;
        };
        step.held_asked = true;
        let Some(before) = before.filter(|held| held.bytes() <= self.held_paint_budget_bytes)
        else {
            return;
        };
        step.held = Some(HeldRecord {
            before,
            after: None,
            load_epoch,
        });
        self.fit_held_paint_budget();
    }

    fn fit_held_paint_budget(&mut self) {
        while self.held_paint_bytes() > self.held_paint_budget_bytes
            && let Some(oldest) = self.undo_steps.pop_front()
        {
            self.release(oldest);
        }
        if self.held_paint_bytes() > self.held_paint_budget_bytes
            && let Some(step) = self.current.as_mut()
        {
            step.held = None;
        }
    }

    /// The video memory held by every capture of tool paint the steps keep.
    pub fn held_paint_bytes(&self) -> u64 {
        self.undo_steps
            .iter()
            .chain(&self.redo_steps)
            .chain(&self.current)
            .map(Step::held_bytes)
            .sum()
    }
}
