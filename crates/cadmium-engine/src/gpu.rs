use std::fmt;
use std::sync::Arc;

use wgpu::hal::{self, api::Vulkan};

pub struct GpuContext {
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub supports_dmabuf_export: bool,
    pub supports_timestamps: bool,
}

#[derive(Debug)]
pub enum GpuError {
    NoVulkanAdapter,
    MissingFeature(&'static str),
    DeviceCreation(String),
}

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GpuError::NoVulkanAdapter => write!(f, "no Vulkan GPU adapter found"),
            GpuError::MissingFeature(feature) => {
                write!(f, "GPU adapter lacks required feature {feature}")
            }
            GpuError::DeviceCreation(reason) => {
                write!(f, "failed to create GPU device: {reason}")
            }
        }
    }
}

impl std::error::Error for GpuError {}

const DMABUF_EXPORT_FEATURES: wgpu::Features =
    wgpu::Features::VULKAN_EXTERNAL_MEMORY_DMA_BUF.union(wgpu::Features::VULKAN_EXTERNAL_MEMORY_FD);

const TIMESTAMP_FEATURES: wgpu::Features =
    wgpu::Features::TIMESTAMP_QUERY.union(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS);

const DMABUF_SYNC_EXTENSIONS: [&std::ffi::CStr; 2] = [
    ash::khr::external_semaphore_fd::NAME,
    ash::ext::queue_family_foreign::NAME,
];

impl GpuContext {
    pub fn new() -> Result<GpuContext, GpuError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(select_adapter(&instance))?;

        let adapter_features = adapter.features();
        if !adapter_features.contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES) {
            return Err(GpuError::MissingFeature(
                "TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES",
            ));
        }
        if !adapter
            .get_texture_format_features(wgpu::TextureFormat::Rgba16Float)
            .flags
            .contains(wgpu::TextureFormatFeatureFlags::STORAGE_READ_WRITE)
        {
            return Err(GpuError::MissingFeature("rgba16float storage read-write"));
        }
        let supports_dmabuf_export = adapter_features.contains(DMABUF_EXPORT_FEATURES)
            && supports_dmabuf_sync_extensions(&adapter);
        let supports_timestamps = adapter_features.contains(TIMESTAMP_FEATURES);

        let mut required_features = wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
        if supports_dmabuf_export {
            required_features |= DMABUF_EXPORT_FEATURES;
        }
        if supports_timestamps {
            required_features |= TIMESTAMP_FEATURES;
        }
        let descriptor = wgpu::DeviceDescriptor {
            label: Some("cadmium"),
            required_features,
            required_limits: adapter.limits(),
            memory_hints: wgpu::MemoryHints::Performance,
            ..Default::default()
        };

        let (device, queue) = if supports_dmabuf_export {
            open_device_with_dmabuf_sync(&adapter, &descriptor)?
        } else {
            pollster::block_on(adapter.request_device(&descriptor))
                .map_err(|e| GpuError::DeviceCreation(e.to_string()))?
        };

        device.on_uncaptured_error(Arc::new(|e| eprintln!("cadmium: wgpu error: {e}")));

        Ok(GpuContext {
            adapter,
            device,
            queue,
            supports_dmabuf_export,
            supports_timestamps,
        })
    }
}

fn is_hardware_vulkan(adapter: &wgpu::Adapter) -> bool {
    let info = adapter.get_info();
    info.backend == wgpu::Backend::Vulkan && info.device_type != wgpu::DeviceType::Cpu
}

fn device_type_preference(device_type: wgpu::DeviceType) -> u8 {
    match device_type {
        wgpu::DeviceType::DiscreteGpu => 0,
        wgpu::DeviceType::IntegratedGpu => 1,
        wgpu::DeviceType::VirtualGpu => 2,
        wgpu::DeviceType::Other => 3,
        wgpu::DeviceType::Cpu => 4,
    }
}

async fn select_adapter(instance: &wgpu::Instance) -> Result<wgpu::Adapter, GpuError> {
    let preferred = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        })
        .await;
    if let Ok(adapter) = preferred
        && is_hardware_vulkan(&adapter)
    {
        return Ok(adapter);
    }
    instance
        .enumerate_adapters(wgpu::Backends::VULKAN)
        .await
        .into_iter()
        .filter(is_hardware_vulkan)
        .min_by_key(|adapter| device_type_preference(adapter.get_info().device_type))
        .ok_or(GpuError::NoVulkanAdapter)
}

fn supports_dmabuf_sync_extensions(adapter: &wgpu::Adapter) -> bool {
    let Some(hal_adapter) = (unsafe { adapter.as_hal::<Vulkan>() }) else {
        return false;
    };
    let capabilities = hal_adapter.physical_device_capabilities();
    DMABUF_SYNC_EXTENSIONS
        .iter()
        .all(|extension| capabilities.supports_extension(extension))
}

fn open_device_with_dmabuf_sync(
    adapter: &wgpu::Adapter,
    descriptor: &wgpu::DeviceDescriptor<'_>,
) -> Result<(wgpu::Device, wgpu::Queue), GpuError> {
    let open = unsafe {
        let hal_adapter = adapter
            .as_hal::<Vulkan>()
            .ok_or_else(|| GpuError::DeviceCreation("adapter is not Vulkan".into()))?;
        hal_adapter.open_with_callback(
            descriptor.required_features,
            &descriptor.required_limits,
            &descriptor.memory_hints,
            Some(Box::new(|args: hal::vulkan::CreateDeviceCallbackArgs| {
                args.extensions.extend(DMABUF_SYNC_EXTENSIONS);
            })),
        )
    }
    .map_err(|e| GpuError::DeviceCreation(e.to_string()))?;
    unsafe { adapter.create_device_from_hal(open, descriptor) }
        .map_err(|e| GpuError::DeviceCreation(e.to_string()))
}
