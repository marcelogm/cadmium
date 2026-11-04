use crate::canvas::StateTexture;
use crate::gpu::GpuContext;
use crate::surface::Surface;

use super::{ClearOutcome, PaintSession, new_encoder};

const DRY_TILES_PER_SUBMIT: usize = 64;

impl PaintSession {
    pub fn dry(&mut self, gpu: &GpuContext) -> bool {
        self.settle_pending_step(gpu);
        let tiles = self.canvas.wet_tiles.take();
        self.history
            .begin_removal_step(Surface::Painting, tiles.len());
        for chunk in tiles.chunks(DRY_TILES_PER_SUBMIT) {
            let mut encoder = new_encoder(gpu, "dry");
            self.history
                .encode_save_tiles(gpu, &self.canvas, chunk, &mut encoder);
            self.engine
                .encode_dry_tiles(gpu, &mut self.canvas, chunk, &mut encoder);
            gpu.queue.submit([encoder.finish()]);
        }
        self.history.commit_step()
    }

    pub fn clear_tray(&mut self, gpu: &GpuContext) -> ClearOutcome {
        self.settle_pending_step(gpu);
        let tiles = self.tray.wet_tiles.take();
        if tiles.is_empty() {
            return ClearOutcome::NothingWet;
        }
        self.history.begin_removal_step(Surface::Tray, tiles.len());
        let mut encoder = new_encoder(gpu, "clear_tray");
        self.history
            .encode_save_tiles(gpu, &self.tray, &tiles, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
        let texels = (self.tray.spec.width * self.tray.spec.height) as usize;
        self.tray
            .write_f32(gpu, StateTexture::WetVol, &vec![0.0; texels]);
        self.tray.dirty.mark_all();
        ClearOutcome::Cleared {
            undoable: self.history.commit_step(),
        }
    }
}
