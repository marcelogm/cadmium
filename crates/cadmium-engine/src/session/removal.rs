use crate::gpu::{GpuContext, new_encoder};

use super::PaintSession;

const DRY_TILES_PER_SUBMIT: usize = 64;

impl PaintSession {
    pub fn dry(&mut self, gpu: &GpuContext) -> bool {
        self.settle_pending_step(gpu);
        let tiles = self.canvas.wet_tiles.take();
        self.history.begin_removal_step(tiles.len());
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
}
