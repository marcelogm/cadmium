use super::bristle_contact::ContactPass;
use super::{DabResources, PlacedDab, bind_at_dab, dab_record_binding, dab_record_entry, dispatch};
use crate::canvas::StateTexture;
use crate::compute::{ComputeStage, buffer_binding, storage_entry, texture_binding, texture_entry};
use crate::engine::placement::Placement;
use crate::engine::shader::{ADVECT_SNIPPETS, SPREAD_SNIPPETS, shader_source};
use crate::gpu::GpuContext;

const CONTACT_MASK_BINDING: u32 = 8;
const CONTACT_EXTENT_BINDING: u32 = 9;
const DRY_HEIGHT_BINDING: u32 = 10;
const CONTACT_REFERENCE_BINDING: u32 = 11;
const TILE_CHANGES_BINDING: u32 = 8;

struct ScratchSurface {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

struct Scratch {
    size: [u32; 2],
    pig0: ScratchSurface,
    pig1: ScratchSurface,
    vol: ScratchSurface,
    motion: ScratchSurface,
}

impl Scratch {
    fn new(gpu: &GpuContext, size: [u32; 2]) -> Scratch {
        Scratch {
            size,
            pig0: scratch_surface(gpu, "scratch_pig0", size, wgpu::TextureFormat::Rgba16Float),
            pig1: scratch_surface(gpu, "scratch_pig1", size, wgpu::TextureFormat::Rgba16Float),
            vol: scratch_surface(gpu, "scratch_vol", size, wgpu::TextureFormat::R32Float),
            motion: scratch_surface(
                gpu,
                "scratch_motion",
                size,
                wgpu::TextureFormat::Rgba32Float,
            ),
        }
    }
}

fn scratch_surface(
    gpu: &GpuContext,
    label: &str,
    [width, height]: [u32; 2],
    format: wgpu::TextureFormat,
) -> ScratchSurface {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::STORAGE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    ScratchSurface {
        _texture: texture,
        view,
    }
}

pub(super) struct SpreadBindGroups {
    copy: wgpu::BindGroup,
    advect: wgpu::BindGroup,
}

pub(super) struct SpreadPass {
    copy_compute: ComputeStage,
    advect_compute: ComputeStage,
    copy: wgpu::ComputePipeline,
    advect: wgpu::ComputePipeline,
    scratch: Option<Scratch>,
}

impl SpreadPass {
    pub(super) fn new(gpu: &GpuContext) -> SpreadPass {
        let copy_compute = ComputeStage::new(
            gpu,
            "region_copy",
            &shader_source(
                &SPREAD_SNIPPETS,
                include_str!("../../shaders/region_copy.wgsl"),
            ),
            &copy_entries(),
        );
        let advect_compute = ComputeStage::new(
            gpu,
            "advect",
            &shader_source(&ADVECT_SNIPPETS, include_str!("../../shaders/advect.wgsl")),
            &advect_entries(),
        );
        SpreadPass {
            copy: copy_compute.pipeline(gpu, "region_copy", "main"),
            advect: advect_compute.pipeline(gpu, "advect", "main"),
            copy_compute,
            advect_compute,
            scratch: None,
        }
    }

    pub(super) fn fit_scratch(&mut self, gpu: &GpuContext, placements: &[Placement]) {
        let needed = placements.iter().fold([0, 0], |size, placement| {
            let region = placement.record.canvas_size;
            [size[0].max(region[0]), size[1].max(region[1])]
        });
        let held = self.scratch.as_ref().map_or([0; 2], |scratch| scratch.size);
        if needed[0] > held[0] || needed[1] > held[1] {
            let grown = [needed[0].max(held[0]), needed[1].max(held[1])];
            self.scratch = Some(Scratch::new(gpu, grown));
        }
    }

    pub(super) fn bind_groups(
        &self,
        resources: &DabResources<'_>,
        contact: HairContact<'_>,
    ) -> SpreadBindGroups {
        let scratch = self
            .scratch
            .as_ref()
            .expect("scratch surfaces are fitted before spreading");
        let scratch_views = [&scratch.pig0.view, &scratch.pig1.view, &scratch.vol.view];
        let motion_view = &scratch.motion.view;
        let canvas = resources.canvas;
        let wet_views = [
            canvas.view(StateTexture::WetPig0),
            canvas.view(StateTexture::WetPig1),
            canvas.view(StateTexture::WetVol),
        ];
        let copy = spreading_bindings(
            resources,
            SpreadViews {
                from: wet_views,
                to: scratch_views,
                motion: motion_view,
            },
            Some(contact),
        );
        let mut advect = spreading_bindings(
            resources,
            SpreadViews {
                from: scratch_views,
                to: wet_views,
                motion: motion_view,
            },
            None,
        );
        advect.push(buffer_binding(
            TILE_CHANGES_BINDING,
            resources.canvas.tile_changes().buffer(),
        ));
        SpreadBindGroups {
            copy: self.copy_compute.bind_group(resources.gpu, &copy),
            advect: self.advect_compute.bind_group(resources.gpu, &advect),
        }
    }

    pub(super) fn encode(
        &self,
        pass: &mut wgpu::ComputePass<'_>,
        bind_groups: &SpreadBindGroups,
        dab: PlacedDab<'_>,
    ) {
        bind_at_dab(pass, &bind_groups.copy, dab);
        dispatch(pass, &self.copy, dab.placement.canvas_groups);
        bind_at_dab(pass, &bind_groups.advect, dab);
        dispatch(pass, &self.advect, dab.placement.spread_groups);
    }
}

fn spreading_entries(motion_access: wgpu::StorageTextureAccess) -> [wgpu::BindGroupLayoutEntry; 8] {
    use wgpu::StorageTextureAccess::{ReadOnly, WriteOnly};
    use wgpu::TextureFormat::{R32Float, Rgba16Float, Rgba32Float};

    [
        dab_record_entry(),
        texture_entry(1, ReadOnly, Rgba16Float),
        texture_entry(2, ReadOnly, Rgba16Float),
        texture_entry(3, ReadOnly, R32Float),
        texture_entry(4, WriteOnly, Rgba16Float),
        texture_entry(5, WriteOnly, Rgba16Float),
        texture_entry(6, WriteOnly, R32Float),
        texture_entry(7, motion_access, Rgba32Float),
    ]
}

fn advect_entries() -> Vec<wgpu::BindGroupLayoutEntry> {
    let mut entries = spreading_entries(wgpu::StorageTextureAccess::ReadOnly).to_vec();
    entries.push(storage_entry(TILE_CHANGES_BINDING, false));
    entries
}

fn copy_entries() -> Vec<wgpu::BindGroupLayoutEntry> {
    let mut entries = spreading_entries(wgpu::StorageTextureAccess::WriteOnly).to_vec();
    entries.extend([
        storage_entry(CONTACT_MASK_BINDING, true),
        storage_entry(CONTACT_EXTENT_BINDING, true),
        texture_entry(
            DRY_HEIGHT_BINDING,
            wgpu::StorageTextureAccess::ReadOnly,
            wgpu::TextureFormat::R32Float,
        ),
        storage_entry(CONTACT_REFERENCE_BINDING, true),
    ]);
    entries
}

#[derive(Clone, Copy)]
pub(super) struct HairContact<'a> {
    pub(super) contact: &'a ContactPass,
    pub(super) reference: &'a wgpu::Buffer,
}

struct SpreadViews<'a> {
    from: [&'a wgpu::TextureView; 3],
    to: [&'a wgpu::TextureView; 3],
    motion: &'a wgpu::TextureView,
}

fn spreading_bindings<'a>(
    resources: &'a DabResources<'_>,
    views: SpreadViews<'a>,
    hairs: Option<HairContact<'a>>,
) -> Vec<wgpu::BindGroupEntry<'a>> {
    let textures = views.from.into_iter().chain(views.to).chain([views.motion]);
    std::iter::once(dab_record_binding(resources.dab_records))
        .chain(
            (1..)
                .zip(textures)
                .map(|(binding, view)| wgpu::BindGroupEntry {
                    binding,
                    resource: wgpu::BindingResource::TextureView(view),
                }),
        )
        .chain(hairs.into_iter().flat_map(|hairs| {
            [
                buffer_binding(CONTACT_MASK_BINDING, &hairs.contact.mask),
                buffer_binding(CONTACT_EXTENT_BINDING, &hairs.contact.extent),
                texture_binding(
                    DRY_HEIGHT_BINDING,
                    resources.canvas,
                    StateTexture::DryHeight,
                ),
                buffer_binding(CONTACT_REFERENCE_BINDING, hairs.reference),
            ]
        }))
        .collect()
}
