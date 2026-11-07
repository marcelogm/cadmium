use super::readout::cell_totals;
use super::reservoir::{dip_fractions, dip_thickness, holds_paint, level_after};
use super::{
    BLADE_CELLS, KNIFE_DIP_LENGTH_PX, KNIFE_LAID_CAPACITY_UM, MAX_TIP_THICKNESS_UM,
    PIGMENT_CHANNELS, TIP_CHANNELS, TIP_HALF, VOLUME_CHANNEL, WetOilEngine,
};
use crate::brush::FULL_RESERVOIR_VOLUME_UM;
use crate::canvas::PaintTotals;
use crate::compute::read_buffer;
use crate::gpu::{GpuContext, GpuLost};
use crate::pigments::Mixture;

impl WetOilEngine {
    pub(super) fn draw_up_into_knife(
        &mut self,
        gpu: &GpuContext,
        take: impl FnOnce(f32) -> Result<PaintTotals, GpuLost>,
    ) -> Result<(PaintTotals, f32), GpuLost> {
        let mut load = read_buffer(gpu, &self.buffers.blade_load)?;
        let shares = blade_shares(self.tip_radius);
        let held = cell_totals(&load).volume as f32;
        let full_dip = self.knife_full_dip_volume(FULL_RESERVOIR_VOLUME_UM);
        let room = (full_dip - held).min(blade_room(&load, &shares)).max(0.0);
        let taken = take(room)?;
        if holds_paint(&taken) {
            add_to_blade(&mut load, &shares, &taken);
            gpu.queue
                .write_buffer(&self.buffers.blade_load, 0, bytemuck::cast_slice(&load));
        }
        Ok((taken, level_after(held, held + room, &taken)))
    }

    pub fn dip_knife(
        &mut self,
        gpu: &GpuContext,
        mixture: &Mixture,
        amount: f32,
    ) -> Result<(), GpuLost> {
        let Some(thickness) = dip_thickness(mixture) else {
            return Ok(());
        };
        if !amount.is_finite() || amount <= 0.0 {
            return Ok(());
        }
        let fractions = dip_fractions(mixture);
        let mut load = read_buffer(gpu, &self.buffers.blade_load)?;
        let reach = self.tip_radius.ceil() as i32 + 1;
        for offset in -reach..=reach {
            let cover = blade_cell_cover(offset, self.tip_radius);
            let cell = (TIP_HALF as i32 + offset).clamp(0, BLADE_CELLS as i32 - 1) as usize;
            let values = &mut load[cell * TIP_CHANNELS..(cell + 1) * TIP_CHANNELS];
            let room = (KNIFE_LAID_CAPACITY_UM - values[VOLUME_CHANNEL]).max(0.0);
            let added = (cover * amount * thickness * KNIFE_DIP_LENGTH_PX).min(room);
            add_to_cell(values, &fractions, added);
        }
        gpu.queue
            .write_buffer(&self.buffers.blade_load, 0, bytemuck::cast_slice(&load));
        Ok(())
    }

    pub fn knife_full_dip_volume(&self, thickness_um: f32) -> f32 {
        if !thickness_um.is_finite() || thickness_um <= 0.0 {
            return 0.0;
        }
        let thickness = thickness_um.min(MAX_TIP_THICKNESS_UM);
        let reach = self.tip_radius.ceil() as i32 + 1;
        (-reach..=reach)
            .map(|offset| {
                (blade_cell_cover(offset, self.tip_radius) * thickness * KNIFE_DIP_LENGTH_PX)
                    .min(KNIFE_LAID_CAPACITY_UM)
            })
            .sum()
    }

    pub fn clean_knife(&mut self, gpu: &GpuContext) {
        self.submit(gpu, "clean_knife", |engine, encoder| {
            engine.encode_clean_knife(encoder);
        });
    }

    pub fn encode_clean_knife(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.clear_buffer(&self.buffers.blade_load, 0, None);
        encoder.clear_buffer(&self.buffers.blade_delta, 0, None);
    }

    pub fn knife_totals(&self, gpu: &GpuContext) -> Result<PaintTotals, GpuLost> {
        Ok(cell_totals(&read_buffer(gpu, &self.buffers.blade_load)?))
    }
}

fn blade_cell_cover(offset: i32, half_length: f32) -> f32 {
    let centre = offset as f32 + 0.5;
    (half_length - centre.abs() + 0.5).clamp(0.0, 1.0)
}

fn blade_shares(half_length: f32) -> Vec<(usize, f32)> {
    let reach = half_length.ceil() as i32 + 1;
    let covers: Vec<(usize, f32)> = (-reach..=reach)
        .map(|offset| {
            let cell = (TIP_HALF as i32 + offset).clamp(0, BLADE_CELLS as i32 - 1) as usize;
            (cell, blade_cell_cover(offset, half_length))
        })
        .filter(|(_, cover)| *cover > 0.0)
        .collect();
    let total_cover: f32 = covers.iter().map(|(_, cover)| cover).sum();
    covers
        .into_iter()
        .map(|(cell, cover)| (cell, cover / total_cover))
        .collect()
}

fn blade_room(load: &[f32], shares: &[(usize, f32)]) -> f32 {
    shares
        .iter()
        .map(|&(cell, share)| {
            (KNIFE_LAID_CAPACITY_UM - load[cell * TIP_CHANNELS + VOLUME_CHANNEL]).max(0.0) / share
        })
        .fold(f32::INFINITY, f32::min)
}

fn add_to_blade(load: &mut [f32], shares: &[(usize, f32)], taken: &PaintTotals) {
    for &(cell, share) in shares {
        let share = f64::from(share);
        let values = &mut load[cell * TIP_CHANNELS..(cell + 1) * TIP_CHANNELS];
        for (slot, amount) in taken.pigment.iter().enumerate() {
            values[slot] += (amount * share) as f32;
        }
        values[VOLUME_CHANNEL] += (taken.volume * share) as f32;
    }
}

fn add_to_cell(values: &mut [f32], fractions: &[f32; PIGMENT_CHANNELS], added: f32) {
    for (slot, fraction) in fractions.iter().enumerate() {
        values[slot] += added * fraction;
    }
    values[VOLUME_CHANNEL] += added;
}

#[cfg(test)]
mod tests;
