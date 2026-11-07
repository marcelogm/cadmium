/// The GPU time a frame's dabs aim to fit in, leaving the rest of a 60 Hz frame for
/// shading, presenting and the toolkit's own drawing.
pub const FRAME_GPU_TARGET_MS: f64 = 12.0;
const FIRST_COST_PER_DAB_MS: f64 = 0.5;
const LEARNING_SHARE: f64 = 0.25;
const FEWEST_DABS_PER_FRAME: usize = 1;

/// How many dabs a frame can paint and stay near `FRAME_GPU_TARGET_MS`, learnt from what
/// the frames before it cost.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DabBudget {
    cost_per_dab_ms: f64,
}

impl Default for DabBudget {
    fn default() -> Self {
        DabBudget {
            cost_per_dab_ms: FIRST_COST_PER_DAB_MS,
        }
    }
}

impl DabBudget {
    pub fn allowance(&self) -> usize {
        ((FRAME_GPU_TARGET_MS / self.cost_per_dab_ms).floor() as usize).max(FEWEST_DABS_PER_FRAME)
    }

    pub fn observe(&mut self, dabs: usize, gpu_ms: f64) {
        if dabs == 0 || !gpu_ms.is_finite() || gpu_ms <= 0.0 {
            return;
        }
        let cost_per_dab_ms = gpu_ms / dabs as f64;
        self.cost_per_dab_ms += LEARNING_SHARE * (cost_per_dab_ms - self.cost_per_dab_ms);
    }

    pub fn cost_per_dab_ms(&self) -> f64 {
        self.cost_per_dab_ms
    }
}

#[cfg(test)]
mod tests;
