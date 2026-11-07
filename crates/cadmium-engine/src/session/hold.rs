use crate::canvas::{Canvas, PaintTotals, tiles_in_rect};
use crate::draw_up::{DrawUp, DrawUpRegion, DrawUpRequest, DrawnUp, draw_up_region};
use crate::engine::{PaintEngine, Tip};
use crate::gpu::{GpuContext, GpuLost, new_encoder};
use crate::history::History;
use crate::surface::Surface;
use crate::tube::{Dwell, Nozzle, SqueezeRecord, dwell_after, squeeze_region};

use super::{PaintSession, surface_canvas};

pub(super) struct Hold {
    surface: Surface,
    dwell: Option<Dwell>,
}

impl PaintSession {
    pub fn begin_hold(&mut self, gpu: &GpuContext, surface: Surface) {
        self.settle_pending_step(gpu);
        self.history.begin_step(surface);
        self.hold = Some(Hold {
            surface,
            dwell: None,
        });
    }

    pub fn end_hold(&mut self) {
        if self.hold.take().is_some() {
            self.history.commit_step();
        }
    }

    pub fn is_holding(&self) -> bool {
        self.hold.is_some()
    }

    pub fn squeeze(&mut self, gpu: &GpuContext, nozzle: Nozzle) -> bool {
        let Some(hold) = self.hold.as_mut() else {
            return false;
        };
        let dwell = dwell_after(hold.dwell, &nozzle);
        let canvas = surface_canvas(hold.surface, &mut self.canvas, &mut self.tray);
        let params = self.engine.params();
        let Some(region) = squeeze_region(canvas, params, &nozzle, dwell) else {
            return false;
        };
        hold.dwell = Some(dwell);
        let tiles = tiles_in_rect(canvas.spec.width, canvas.spec.height, region.rect());
        let mut encoder = new_encoder(gpu, "squeeze");
        self.history
            .encode_save_tiles(gpu, canvas, &tiles, &mut encoder);
        let record = SqueezeRecord::new(&nozzle, &region, params);
        self.tube.encode_squeeze(gpu, canvas, &record, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
        true
    }

    pub fn draw_up(
        &mut self,
        gpu: &GpuContext,
        request: DrawUpRequest,
    ) -> Result<DrawnUp, GpuLost> {
        self.flush(gpu);
        let Some(Hold { surface, .. }) = self.hold else {
            return Ok(DrawnUp::default());
        };
        let profile = self.brush.tool.profile();
        // The mixing knife's face takes paint only by scraping it up.
        if profile.holder.is_none() || profile.holds_on_face() {
            return Ok(DrawnUp::default());
        }
        self.resize_tip();
        let into_knife = self.holds_knife();
        let region = draw_up_region(self.canvas_of(surface), self.engine.params(), &request);
        let canvas = surface_canvas(surface, &mut self.canvas, &mut self.tray);
        let source = region.map(|region| DrawUpSource {
            drawer: &self.drawer,
            history: &mut self.history,
            canvas,
            region,
        });
        self.engine.select_tip(Tip::Painting);
        let (taken, level) = self
            .engine
            .draw_up(gpu, into_knife, |room_um| match source {
                Some(source) => source.take(gpu, room_um),
                None => Ok(PaintTotals::default()),
            })?;
        Ok(DrawnUp { taken, level })
    }
}

struct DrawUpSource<'a> {
    drawer: &'a DrawUp,
    history: &'a mut History,
    canvas: &'a mut Canvas,
    region: DrawUpRegion,
}

impl DrawUpSource<'_> {
    fn take(self, gpu: &GpuContext, room_um: f32) -> Result<PaintTotals, GpuLost> {
        let DrawUpSource {
            drawer,
            history,
            canvas,
            region,
        } = self;
        let nothing = PaintTotals::default();
        if room_um.is_nan() || room_um <= 0.0 {
            return Ok(nothing);
        }
        let tiles = tiles_in_rect(canvas.spec.width, canvas.spec.height, region.rect());
        if !tiles.iter().any(|tile| canvas.wet_tiles.contains(*tile)) {
            return Ok(nothing);
        }
        let measured = drawer.measure(gpu, canvas, &region, room_um)?;
        if measured.taken.volume.is_nan() || measured.taken.volume <= 0.0 {
            return Ok(nothing);
        }
        let mut encoder = new_encoder(gpu, "draw_up");
        history.encode_save_tiles(gpu, canvas, &tiles, &mut encoder);
        drawer.encode_take(canvas, &measured, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
        Ok(measured.taken)
    }
}
