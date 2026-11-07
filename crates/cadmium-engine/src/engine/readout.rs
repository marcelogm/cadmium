use super::{
    BLADE_LOAD_VALUES, BODY_BYTES, PIGMENT_CHANNELS, TIP_CHANNELS, TIP_SUM_CHANNELS,
    VOLUME_CHANNEL, WetOilEngine,
};
use crate::canvas::PaintTotals;
use crate::compute::read_buffer;
use crate::gpu::{GpuContext, GpuLost, new_encoder, read_mapped, staging_buffer};
use crate::pigments::Mixture;

const TIP_TEXEL_COUNT_SUM: usize = TIP_CHANNELS;
const MIN_TIP_MEAN_VOLUME_UM: f32 = 1e-3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrushReadout {
    pub tip_mean: Mixture,
    pub reserve: PaintTotals,
    pub knife: PaintTotals,
    pub face: PaintTotals,
}

impl WetOilEngine {
    pub fn tip_mean_mixture(&self, gpu: &GpuContext) -> Result<Mixture, GpuLost> {
        let mut encoder = new_encoder(gpu, "brush_sum_tip");
        self.encode_sum_tip(gpu, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
        Ok(tip_mean_from_sums(&read_buffer(
            gpu,
            &self.buffers.tip_sums,
        )?))
    }

    pub fn reserve_totals(&self, gpu: &GpuContext) -> Result<PaintTotals, GpuLost> {
        Ok(reserve_from_body(&read_buffer(gpu, &self.buffers.body)?))
    }

    pub fn read_brush(&self, gpu: &GpuContext) -> Result<BrushReadout, GpuLost> {
        let sources = [
            &self.buffers.tip_sums,
            &self.buffers.body,
            &self.buffers.blade_load,
            &self.buffers.face_load,
        ];
        let staging = staging_buffer(
            gpu,
            "brush_readout",
            sources.iter().map(|source| source.size()).sum(),
        );
        let mut encoder = new_encoder(gpu, "brush_readout");
        self.encode_sum_tip(gpu, &mut encoder);
        let mut offset = 0;
        for source in sources {
            encoder.copy_buffer_to_buffer(source, 0, &staging, offset, source.size());
            offset += source.size();
        }
        gpu.queue.submit([encoder.finish()]);
        read_mapped(gpu, &staging, |bytes| {
            let values: &[f32] = bytemuck::cast_slice(bytes);
            let (sums, rest) = values.split_at(TIP_SUM_CHANNELS);
            let (body, loads) = rest.split_at(BODY_BYTES as usize / std::mem::size_of::<f32>());
            let (blade, face) = loads.split_at(BLADE_LOAD_VALUES);
            BrushReadout {
                tip_mean: tip_mean_from_sums(sums),
                reserve: reserve_from_body(body),
                knife: cell_totals(blade),
                face: cell_totals(face),
            }
        })
    }

    pub fn brush_totals(&self, gpu: &GpuContext) -> Result<PaintTotals, GpuLost> {
        let mut totals = PaintTotals::default();
        for texel in read_buffer(gpu, &self.buffers.tip)?
            .as_chunks::<TIP_CHANNELS>()
            .0
        {
            accumulate(&mut totals, texel);
        }
        accumulate(
            &mut totals,
            &read_buffer(gpu, &self.buffers.body)?[..TIP_CHANNELS],
        );
        Ok(totals)
    }

    pub fn tip_volumes_um(&self, gpu: &GpuContext) -> Result<Vec<f32>, GpuLost> {
        let tip = read_buffer(gpu, &self.buffers.tip)?;
        Ok(tip
            .as_chunks::<TIP_CHANNELS>()
            .0
            .iter()
            .map(|texel| texel[VOLUME_CHANNEL])
            .collect())
    }

    fn encode_sum_tip(&self, gpu: &GpuContext, encoder: &mut wgpu::CommandEncoder) {
        self.dip_pass
            .encode_sum_tip(gpu, self.tip_radius, &self.buffers, encoder);
    }
}

fn tip_mean_from_sums(sums: &[f32]) -> Mixture {
    let volume = sums[VOLUME_CHANNEL];
    let texels = sums[TIP_TEXEL_COUNT_SUM];
    if volume < MIN_TIP_MEAN_VOLUME_UM || texels <= 0.0 {
        return Mixture {
            volume_um: 0.0,
            fractions: [0.0; PIGMENT_CHANNELS],
        };
    }
    Mixture {
        volume_um: volume / texels,
        fractions: std::array::from_fn(|slot| sums[slot] / volume),
    }
}

fn reserve_from_body(body: &[f32]) -> PaintTotals {
    let mut totals = PaintTotals::default();
    accumulate(&mut totals, &body[..TIP_CHANNELS]);
    totals
}

pub(super) fn accumulate(totals: &mut PaintTotals, amounts: &[f32]) {
    for (slot, amount) in amounts[..PIGMENT_CHANNELS].iter().enumerate() {
        totals.pigment[slot] += f64::from(*amount);
    }
    totals.volume += f64::from(amounts[VOLUME_CHANNEL]);
}

/// The paint held over a blade's cells, `TIP_CHANNELS` values to a cell.
pub(super) fn cell_totals(load: &[f32]) -> PaintTotals {
    let mut totals = PaintTotals::default();
    for cell in load.as_chunks::<TIP_CHANNELS>().0 {
        accumulate(&mut totals, cell);
    }
    totals
}
