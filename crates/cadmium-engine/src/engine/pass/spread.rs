use super::{DabResources, PlacedDab, bind_at_dab, dab_record_binding, dab_record_entry, dispatch};
use crate::canvas::{Canvas, StateTexture};
use crate::compute::{ComputeStage, texture_entry};
use crate::engine::placement::{Placement, max_dab_region_side};
use crate::engine::shader::{DAB_SNIPPETS, shader_source};
use crate::gpu::GpuContext;

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
        use wgpu::StorageTextureAccess::{ReadOnly, WriteOnly};
        use wgpu::TextureFormat::{R32Float, Rgba16Float, Rgba32Float};

        let spreading_entries = |motion_access| {
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
        };
        let copy_compute = ComputeStage::new(
            gpu,
            "region_copy",
            &shader_source(
                &DAB_SNIPPETS,
                include_str!("../../shaders/region_copy.wgsl"),
            ),
            &spreading_entries(WriteOnly),
        );
        let advect_compute = ComputeStage::new(
            gpu,
            "advect",
            &shader_source(&DAB_SNIPPETS, include_str!("../../shaders/advect.wgsl")),
            &spreading_entries(ReadOnly),
        );
        SpreadPass {
            copy: copy_compute.pipeline(gpu, "region_copy", "main"),
            advect: advect_compute.pipeline(gpu, "advect", "main"),
            copy_compute,
            advect_compute,
            scratch: None,
        }
    }

    pub(super) fn fit_scratch(
        &mut self,
        gpu: &GpuContext,
        canvas: &Canvas,
        placements: &[Placement],
    ) {
        let side = max_dab_region_side();
        let size = [canvas.spec.width.min(side), canvas.spec.height.min(side)];
        if self
            .scratch
            .as_ref()
            .is_none_or(|scratch| scratch.size != size)
        {
            self.scratch = Some(Scratch::new(gpu, size));
        }
        debug_assert!(placements.iter().all(|placement| {
            let scratch_size = self.scratch.as_ref().map_or([0; 2], |scratch| scratch.size);
            placement.fits_within(scratch_size)
        }));
    }

    pub(super) fn bind_groups(&self, resources: &DabResources<'_>) -> SpreadBindGroups {
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
        SpreadBindGroups {
            copy: spreading_bind_group(
                &self.copy_compute,
                resources,
                wet_views,
                scratch_views,
                motion_view,
            ),
            advect: spreading_bind_group(
                &self.advect_compute,
                resources,
                scratch_views,
                wet_views,
                motion_view,
            ),
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

fn spreading_bind_group(
    compute: &ComputeStage,
    resources: &DabResources<'_>,
    from: [&wgpu::TextureView; 3],
    to: [&wgpu::TextureView; 3],
    motion: &wgpu::TextureView,
) -> wgpu::BindGroup {
    let views = from.into_iter().chain(to).chain([motion]);
    let entries: Vec<wgpu::BindGroupEntry> =
        std::iter::once(dab_record_binding(resources.dab_records))
            .chain(
                (1..)
                    .zip(views)
                    .map(|(binding, view)| wgpu::BindGroupEntry {
                        binding,
                        resource: wgpu::BindingResource::TextureView(view),
                    }),
            )
            .collect();
    compute.bind_group(resources.gpu, &entries)
}
