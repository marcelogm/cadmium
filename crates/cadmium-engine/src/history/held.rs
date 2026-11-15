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

    /// Keeps the paint the tool holds now, for the redo of a step about to be undone.
    pub(super) fn encode_keep_after(
        &mut self,
        gpu: &GpuContext,
        engine: &WetOilEngine,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if self.still_fits(engine) && self.after.is_none() {
            self.after = engine.encode_capture_held_paint(gpu, encoder);
        }
    }

    pub(super) fn encode_undo(
        &self,
        engine: &mut WetOilEngine,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if self.still_fits(engine) {
            engine.encode_restore_held_paint(&self.before, encoder);
        }
    }

    /// Gives the tool the paint the step left it and lets go of that capture: an undo of
    /// the step takes it again.
    pub(super) fn encode_redo(
        &mut self,
        engine: &mut WetOilEngine,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if !self.still_fits(engine) {
            return;
        }
        if let Some(after) = self.after.take() {
            engine.encode_restore_held_paint(&after, encoder);
        }
    }
}

impl Step {
    fn held_bytes(&self) -> u64 {
        self.held.as_ref().map_or(0, HeldRecord::bytes)
    }

    fn awaits_held_paint(&self) -> bool {
        self.leaves_dry.is_empty() && self.complete && !self.held_asked
    }

    fn forget_held_outside(&mut self, load_epoch: u64) {
        self.held.take_if(|held| held.load_epoch != load_epoch);
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
    /// could not be captured, and the step then restores the canvas alone. Captures of
    /// earlier load epochs, which no step can restore any more, are let go. Past the
    /// held-paint budget the steps to redo lose their captures, then the oldest steps go,
    /// whole.
    pub fn hold_before(&mut self, before: Option<HeldPaint>, load_epoch: u64) {
        self.forget_held_outside(load_epoch);
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

    fn forget_held_outside(&mut self, load_epoch: u64) {
        self.undo_steps
            .iter_mut()
            .chain(&mut self.redo_steps)
            .chain(&mut self.current)
            .for_each(|step| step.forget_held_outside(load_epoch));
    }

    fn fit_held_paint_budget(&mut self) {
        let mut excess_bytes = self
            .held_paint_bytes()
            .saturating_sub(self.held_paint_budget_bytes);
        let mut steps_to_redo = self.redo_steps.iter_mut();
        while excess_bytes > 0
            && let Some(step) = steps_to_redo.next()
        {
            excess_bytes = excess_bytes.saturating_sub(step.held_bytes());
            step.held = None;
        }
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
