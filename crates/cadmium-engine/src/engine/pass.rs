use super::Buffers;
use super::dab_record::{DAB_RECORD_SIZE, DAB_RECORD_STRIDE};
use super::face_load::FaceSettings;
use super::placement::Placement;
use crate::canvas::Canvas;
use crate::compute::uniform_entry;
use crate::gpu::GpuContext;
use crate::tool::DabStage;
use apply::ApplyPass;
use blade::BladePass;
use bristle_contact::ContactPass;
use bristle_step::BristleStepPass;
use capillary::CapillaryPass;
use exchange::ExchangePass;
use face::FacePass;
use graphite::GraphitePass;
use refill::RefillPass;
use spread::{HairContact, SpreadBindGroups, SpreadPass};
use touched_tiles::TouchedTilesPass;
use wipe::WipePass;

mod apply;
mod blade;
pub(super) mod bristle_contact;
mod bristle_step;
mod capillary;
pub(super) mod dip;
pub(super) mod dry;
mod exchange;
mod face;
pub(super) mod face_sum;
mod graphite;
pub(super) mod pick;
mod refill;
mod spread;
mod touched_tiles;
mod wipe;

pub(super) struct DabResources<'a> {
    pub(super) gpu: &'a GpuContext,
    pub(super) canvas: &'a Canvas,
    pub(super) dab_records: &'a wgpu::Buffer,
    pub(super) buffers: &'a Buffers,
    /// The face's settings, for a batch that works the mixing knife's face.
    pub(super) face_settings: Option<FaceSettings>,
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
    bristles: BristleStepPass,
    contact: ContactPass,
    exchange: ExchangePass,
    touched: TouchedTilesPass,
    apply: ApplyPass,
    capillary: CapillaryPass,
    refill: RefillPass,
    spread: SpreadPass,
    blade: BladePass,
    face: FacePass,
    wipe: WipePass,
    graphite: GraphitePass,
}

pub(super) struct DabBindGroups {
    bristles: wgpu::BindGroup,
    contact: wgpu::BindGroup,
    exchange: wgpu::BindGroup,
    touched: wgpu::BindGroup,
    apply: wgpu::BindGroup,
    refill: wgpu::BindGroup,
    wipe: wgpu::BindGroup,
    graphite: wgpu::BindGroup,
    blade: wgpu::BindGroup,
    face: Option<wgpu::BindGroup>,
    spread: SpreadBindGroups,
}

impl DabPasses {
    pub(super) fn new(gpu: &GpuContext) -> DabPasses {
        let refill = RefillPass::new(gpu);
        DabPasses {
            bristles: BristleStepPass::new(gpu),
            contact: ContactPass::new(gpu),
            exchange: ExchangePass::new(gpu),
            touched: TouchedTilesPass::new(gpu),
            apply: ApplyPass::new(gpu),
            capillary: CapillaryPass::new(gpu, &refill.compute),
            refill,
            spread: SpreadPass::new(gpu),
            blade: BladePass::new(gpu),
            face: FacePass::new(gpu),
            wipe: WipePass::new(gpu),
            graphite: GraphitePass::new(gpu),
        }
    }

    pub(super) fn fit_scratch(&mut self, gpu: &GpuContext, placements: &[Placement]) {
        self.spread.fit_scratch(gpu, placements);
        self.contact.fit(gpu, placements);
    }

    /// The bind groups for one batch of dabs, `pencil_lead` holding that batch's lead.
    pub(super) fn bind_groups(
        &self,
        resources: &DabResources<'_>,
        pencil_lead: &wgpu::Buffer,
    ) -> DabBindGroups {
        DabBindGroups {
            bristles: self.bristles.bind_group(resources),
            contact: self.contact.bind_group(resources),
            exchange: self.exchange.bind_group(resources, &self.contact),
            touched: self.touched.bind_group(resources, &self.contact),
            apply: self.apply.bind_group(resources),
            refill: self.refill.bind_group(resources),
            wipe: self.wipe.bind_group(resources),
            graphite: self.graphite.bind_group(resources, pencil_lead),
            blade: self.blade.bind_group(resources),
            face: resources
                .face_settings
                .map(|settings| self.face.bind_group(resources, settings)),
            spread: self.spread.bind_groups(
                resources,
                HairContact {
                    contact: &self.contact,
                    reference: self.exchange.contact_reference(),
                },
            ),
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

    pub(super) fn encode_bristles_alone(
        &self,
        resources: &DabResources<'_>,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let bind_group = self.bristles.bind_group(resources);
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("bristle_settle"),
            timestamp_writes: None,
        });
        self.bristles.encode_first(&mut pass, &bind_group);
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

    /// Works the face; a batch whose tool doesn't work it has no face bind group.
    fn encode_face(
        &self,
        bind_groups: &DabBindGroups,
        pass: &mut wgpu::ComputePass<'_>,
        dab: PlacedDab<'_>,
    ) {
        let Some(face) = &bind_groups.face else {
            return;
        };
        self.face.encode(pass, face, dab);
    }

    fn encode_stage(
        &self,
        stage: DabStage,
        bind_groups: &DabBindGroups,
        pass: &mut wgpu::ComputePass<'_>,
        dab: PlacedDab<'_>,
    ) {
        match stage {
            DabStage::Bristles => self.bristles.encode(pass, &bind_groups.bristles, dab),
            DabStage::Contact => self.contact.encode(pass, &bind_groups.contact, dab),
            DabStage::Exchange => {
                self.exchange.encode(pass, &bind_groups.exchange, dab);
                self.touched.encode(pass, &bind_groups.touched, dab);
            }
            DabStage::Apply => self.apply.encode(pass, &bind_groups.apply, dab),
            DabStage::Capillary => self.capillary.encode(pass, &bind_groups.refill, dab),
            DabStage::Refill => self.refill.encode(pass, &bind_groups.refill, dab),
            DabStage::Spread => self.spread.encode(pass, &bind_groups.spread, dab),
            DabStage::Blade => self.blade.encode(pass, &bind_groups.blade, dab),
            DabStage::Face => self.encode_face(bind_groups, pass, dab),
            DabStage::Wipe => self.wipe.encode(pass, &bind_groups.wipe, dab),
            DabStage::Graphite => self.graphite.encode(pass, &bind_groups.graphite, dab),
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
