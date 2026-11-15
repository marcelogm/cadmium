use std::collections::VecDeque;

use crate::brush::BrushSettings;
use crate::pigments::Mixture;
use crate::stroke::Dab;
use crate::tool::Holder;

pub(super) enum Queued {
    Stroke(QueuedStroke),
    Brush(BrushChange),
    Knife(KnifeLoad),
}

pub(super) struct QueuedStroke {
    pub(super) brush: BrushSettings,
    pub(super) dabs: VecDeque<Dab>,
    /// The last dab already encoded, which the next one sweeps from.
    pub(super) last_painted_dab: Option<Dab>,
    pub(super) begun: bool,
    pub(super) ended: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SweptDab {
    pub(super) dab: Dab,
    pub(super) swept_from: Option<Dab>,
}

/// A change to the brush encoded on the GPU, with everything it depends on read when it
/// was asked for: the brush settings may have moved on by the time it runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum BrushChange {
    ResizeTip { radius_px: f32 },
    Reload { mixture: Mixture },
    RestoreReserve { mixture: Mixture },
    Dip { mixture: Mixture, amount: f32 },
    Clean { holder: Option<Holder> },
}

/// A load of the knife, which reads the blade back and writes it from the CPU: it runs
/// only once every earlier piece of work is submitted, never sharing an encoder with them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum KnifeLoad {
    Dip { mixture: Mixture, amount: f32 },
    TopUp { mixture: Mixture },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WalkStep {
    Begin,
    Dabs(usize),
    End,
    Command,
    Barrier,
    Wait,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct WalkProgress {
    pub(super) dab_limit: Option<usize>,
    pub(super) dabs_painted: usize,
    pub(super) encoded: bool,
}

impl WalkProgress {
    pub(super) fn limited_to(dab_limit: Option<usize>) -> WalkProgress {
        WalkProgress {
            dab_limit,
            dabs_painted: 0,
            encoded: false,
        }
    }

    pub(super) fn unlimited() -> WalkProgress {
        WalkProgress::limited_to(None)
    }

    pub(super) fn took(self, step: WalkStep) -> WalkProgress {
        match step {
            WalkStep::Dabs(count) => WalkProgress {
                dabs_painted: self.dabs_painted + count,
                encoded: true,
                ..self
            },
            WalkStep::Command => WalkProgress {
                encoded: true,
                ..self
            },
            _ => self,
        }
    }

    fn dabs_left(&self) -> Option<usize> {
        self.dab_limit
            .map(|limit| limit.saturating_sub(self.dabs_painted))
    }
}

impl QueuedStroke {
    pub(super) fn new(brush: BrushSettings) -> QueuedStroke {
        QueuedStroke {
            brush,
            dabs: VecDeque::new(),
            last_painted_dab: None,
            begun: false,
            ended: false,
        }
    }

    /// Takes the next `count` dabs to paint, each with the dab it sweeps from: the one
    /// painted before it in this stroke, which the stroke's first dab does not have.
    pub(super) fn take_dabs(&mut self, count: usize) -> Vec<SweptDab> {
        self.dabs
            .drain(..count)
            .map(|dab| SweptDab {
                dab,
                swept_from: self.last_painted_dab.replace(dab),
            })
            .collect()
    }

    fn is_drawing_idle(&self) -> bool {
        self.begun && !self.ended && self.dabs.is_empty()
    }

    fn awaits_frames(&self) -> bool {
        !self.begun || self.ended || !self.dabs.is_empty()
    }
}

impl Queued {
    pub(super) fn awaits_frames(&self) -> bool {
        match self {
            Queued::Stroke(stroke) => stroke.awaits_frames(),
            Queued::Brush(_) | Queued::Knife(_) => true,
        }
    }

    pub(super) fn holds_work(&self) -> bool {
        match self {
            Queued::Stroke(stroke) => !stroke.dabs.is_empty(),
            Queued::Brush(_) | Queued::Knife(_) => true,
        }
    }

    pub(super) fn holds_back_brush_changes(&self) -> bool {
        !matches!(self, Queued::Stroke(stroke) if stroke.is_drawing_idle())
    }

    pub(super) fn is_painted_out(&self) -> bool {
        matches!(self, Queued::Stroke(stroke) if stroke.begun && stroke.ended && stroke.dabs.is_empty())
    }
}

pub(super) fn next_step(head: Option<&Queued>, progress: WalkProgress) -> WalkStep {
    match head {
        None => WalkStep::Wait,
        Some(Queued::Stroke(stroke)) => next_stroke_step(stroke, progress.dabs_left()),
        Some(Queued::Brush(_)) => WalkStep::Command,
        Some(Queued::Knife(_)) if progress.encoded => WalkStep::Wait,
        Some(Queued::Knife(_)) => WalkStep::Barrier,
    }
}

fn next_stroke_step(stroke: &QueuedStroke, dabs_left: Option<usize>) -> WalkStep {
    if !stroke.begun {
        return WalkStep::Begin;
    }
    if stroke.dabs.is_empty() {
        return if stroke.ended {
            WalkStep::End
        } else {
            WalkStep::Wait
        };
    }
    match dabs_left.map_or(stroke.dabs.len(), |left| left.min(stroke.dabs.len())) {
        0 => WalkStep::Wait,
        count => WalkStep::Dabs(count),
    }
}

#[cfg(test)]
mod tests;
