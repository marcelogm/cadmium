use wgpu::util::DeviceExt;

use crate::gpu::GpuContext;
use crate::pigments::{MAX_SLOTS, PigmentDb, mixture_reflectance};
use crate::spectral::{BANDS, BandValues, km_infinite, reflectance_srgb8};

use super::{BANDS_PER_VEC4, Canvas, Primer, StateTexture, weave_heights};

const PRIMER_WHITE_ID: &str = "titanium_white";
const IMPRIMATURA_PIGMENT_ID: &str = "burnt_sienna";
const IMPRIMATURA_FRACTION: f32 = 0.5;
const IMPRIMATURA_THICKNESS_UM: f32 = 12.0;
const TRAY_REFLECTANCE: f32 = 0.75;

const INIT_WORKGROUP_SIZE: u32 = 8;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct InitParams {
    primer_bands_low: [f32; 4],
    primer_bands_high: [f32; 4],
}

pub fn primer_reflectance(db: &PigmentDb, primer: Primer) -> BandValues {
    let white = db
        .get(PRIMER_WHITE_ID)
        .expect("titanium white is in the builtin pigment database");
    let gesso: BandValues =
        std::array::from_fn(|band| km_infinite(white.absorption[band], white.scattering[band]));
    match primer {
        Primer::WhiteGesso => gesso,
        Primer::SiennaImprimatura => {
            let mut fractions = [0.0; MAX_SLOTS];
            fractions[0] = IMPRIMATURA_FRACTION;
            mixture_reflectance(
                db,
                &[IMPRIMATURA_PIGMENT_ID.to_string()],
                &fractions,
                IMPRIMATURA_THICKNESS_UM,
                &gesso,
            )
        }
        Primer::Tray => [TRAY_REFLECTANCE; BANDS],
    }
}

pub fn primer_srgb8(db: &PigmentDb, primer: Primer) -> [u8; 3] {
    reflectance_srgb8(&primer_reflectance(db, primer))
}

impl Canvas {
    #[expect(clippy::too_many_lines)]
    pub(super) fn initialise(&self, gpu: &GpuContext, db: &PigmentDb) {
        let primer = primer_reflectance(db, self.spec.primer);
        let params = InitParams {
            primer_bands_low: std::array::from_fn(|band| primer[band]),
            primer_bands_high: std::array::from_fn(|band| primer[BANDS_PER_VEC4 + band]),
        };
        let params_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("canvas_init_params"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("canvas_init"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("../shaders/canvas_init.wgsl").into(),
                ),
            });
        let pipeline = gpu
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("canvas_init"),
                layout: None,
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });
        let mut entries = vec![wgpu::BindGroupEntry {
            binding: 0,
            resource: params_buffer.as_entire_binding(),
        }];
        for (binding, which) in (1..).zip(StateTexture::ALL) {
            entries.push(wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::TextureView(self.view(which)),
            });
        }
        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("canvas_init"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &entries,
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("canvas_init"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("canvas_init"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(
                self.spec.width.div_ceil(INIT_WORKGROUP_SIZE),
                self.spec.height.div_ceil(INIT_WORKGROUP_SIZE),
                1,
            );
        }
        gpu.queue.submit([encoder.finish()]);
        self.write_f32(
            gpu,
            StateTexture::DryHeight,
            &weave_heights(self.spec.weave, self.spec.width, self.spec.height),
        );
    }
}
