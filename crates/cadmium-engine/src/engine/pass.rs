use super::Buffers;
use super::dab_record::{DAB_RECORD_SIZE, DAB_RECORD_STRIDE};
use super::placement::Placement;
use crate::canvas::Canvas;
use crate::compute::uniform_entry;
use crate::gpu::GpuContext;
use crate::tool::DabStage;
use apply::ApplyPass;
use blade::BladePass;
use capillary::CapillaryPass;
use exchange::ExchangePass;
use refill::RefillPass;
use spread::{SpreadBindGroups, SpreadPass};
use wipe::WipePass;

mod apply;
mod blade;
mod capillary;
pub(super) mod dip;
pub(super) mod dry;
mod exchange;
pub(super) mod pick;
mod refill;
mod spread;
mod wipe;

pub(super) struct DabResources<'a> {
    pub(super) gpu: &'a GpuContext,
    pub(super) canvas: &'a Canvas,
    pub(super) dab_records: &'a wgpu::Buffer,
    pub(super) buffers: &'a Buffers,
}

#[derive(Clone, Copy)]
struct PlacedDab<'a> {
    placement: &'a Placement,
    offset: wgpu::DynamicOffset,
}

impl<'a> PlacedDab<'a> {
    fn at(index: usize, placement: &'a Placement) -> PlacedDab<'a> {
        PlacedDab {
            placement,
            offset: (index as u64 * DAB_RECORD_STRIDE) as wgpu::DynamicOffset,
        }
    }
}

pub(super) struct DabPasses {
    exchange: ExchangePass,
    apply: ApplyPass,
    capillary: CapillaryPass,
    refill: RefillPass,
    spread: SpreadPass,
    blade: BladePass,
    wipe: WipePass,
}

pub(super) struct DabBindGroups {
    exchange: wgpu::BindGroup,
    apply: wgpu::BindGroup,
    refill: wgpu::BindGroup,
    wipe: wgpu::BindGroup,
    blade: wgpu::BindGroup,
    spread: SpreadBindGroups,
}

impl DabPasses {
    pub(super) fn new(gpu: &GpuContext) -> DabPasses {
        let refill = RefillPass::new(gpu);
        DabPasses {
            exchange: ExchangePass::new(gpu),
            apply: ApplyPass::new(gpu),
            capillary: CapillaryPass::new(gpu, &refill.compute),
            refill,
            spread: SpreadPass::new(gpu),
            blade: BladePass::new(gpu),
            wipe: WipePass::new(gpu),
        }
    }

    pub(super) fn fit_scratch(
        &mut self,
        gpu: &GpuContext,
        canvas: &Canvas,
        placements: &[Placement],
    ) {
        self.spread.fit_scratch(gpu, canvas, placements);
    }

    pub(super) fn bind_groups(&self, resources: &DabResources<'_>) -> DabBindGroups {
        DabBindGroups {
            exchange: self.exchange.bind_group(resources),
            apply: self.apply.bind_group(resources),
            refill: self.refill.bind_group(resources),
            wipe: self.wipe.bind_group(resources),
            blade: self.blade.bind_group(resources),
            spread: self.spread.bind_groups(resources),
        }
    }

    pub(super) fn encode(
        &self,
        bind_groups: &DabBindGroups,
        pass: &mut wgpu::ComputePass<'_>,
        placements: &[Placement],
    ) {
        for (index, placement) in placements.iter().enumerate() {
            self.encode_dab(bind_groups, pass, PlacedDab::at(index, placement));
        }
    }

    fn encode_dab(
        &self,
        bind_groups: &DabBindGroups,
        pass: &mut wgpu::ComputePass<'_>,
        dab: PlacedDab<'_>,
    ) {
        for stage in dab.placement.profile.dab_stages {
            self.encode_stage(*stage, bind_groups, pass, dab);
        }
    }

    fn encode_stage(
        &self,
        stage: DabStage,
        bind_groups: &DabBindGroups,
        pass: &mut wgpu::ComputePass<'_>,
        dab: PlacedDab<'_>,
    ) {
        match stage {
            DabStage::Exchange => self.exchange.encode(pass, &bind_groups.exchange, dab),
            DabStage::Apply => self.apply.encode(pass, &bind_groups.apply, dab),
            DabStage::Capillary => self.capillary.encode(pass, &bind_groups.refill, dab),
            DabStage::Refill => self.refill.encode(pass, &bind_groups.refill, dab),
            DabStage::Spread => self.spread.encode(pass, &bind_groups.spread, dab),
            DabStage::Blade => self.blade.encode(pass, &bind_groups.blade, dab),
            DabStage::Wipe => self.wipe.encode(pass, &bind_groups.wipe, dab),
        }
    }
}

fn dab_record_entry() -> wgpu::BindGroupLayoutEntry {
    uniform_entry(0, Some(DAB_RECORD_SIZE))
}

fn dab_record_binding(buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding: 0,
        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer,
            offset: 0,
            size: wgpu::BufferSize::new(DAB_RECORD_SIZE),
        }),
    }
}

fn bind_at_dab(pass: &mut wgpu::ComputePass<'_>, bind_group: &wgpu::BindGroup, dab: PlacedDab<'_>) {
    pass.set_bind_group(0, bind_group, &[dab.offset]);
}

fn dispatch(pass: &mut wgpu::ComputePass<'_>, pipeline: &wgpu::ComputePipeline, [x, y]: [u32; 2]) {
    pass.set_pipeline(pipeline);
    pass.dispatch_workgroups(x, y, 1);
}
