use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use wgpu::hal::{self, api::Vulkan};

pub struct GpuContext {
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub supports_dmabuf_export: bool,
    lost: Arc<AtomicBool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GpuLost;

impl fmt::Display for GpuLost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the GPU device was lost")
    }
}

impl std::error::Error for GpuLost {}

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
        require_storage_features(&adapter, adapter_features)?;
        let supports_dmabuf_export = cfg!(target_os = "linux")
            && adapter_features.contains(DMABUF_EXPORT_FEATURES)
            && supports_dmabuf_sync_extensions(&adapter);
        let (device, queue) = open_device(&adapter, supports_dmabuf_export)?;
        let lost = watch_device(&device);
        Ok(GpuContext {
            adapter,
            device,
            queue,
            supports_dmabuf_export,
            lost,
        })
    }

    pub fn is_lost(&self) -> bool {
        self.lost.load(Ordering::SeqCst)
    }

    fn mark_lost(&self) -> GpuLost {
        self.lost.store(true, Ordering::SeqCst);
        GpuLost
    }
}

fn require_storage_features(
    adapter: &wgpu::Adapter,
    adapter_features: wgpu::Features,
) -> Result<(), GpuError> {
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
    Ok(())
}

fn open_device(
    adapter: &wgpu::Adapter,
    supports_dmabuf_export: bool,
) -> Result<(wgpu::Device, wgpu::Queue), GpuError> {
    let mut required_features = wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
    if supports_dmabuf_export {
        required_features |= DMABUF_EXPORT_FEATURES;
    }
    let descriptor = wgpu::DeviceDescriptor {
        label: Some("cadmium"),
        required_features,
        required_limits: adapter.limits(),
        memory_hints: wgpu::MemoryHints::Performance,
        ..Default::default()
    };
    if supports_dmabuf_export {
        open_device_with_dmabuf_sync(adapter, &descriptor)
    } else {
        pollster::block_on(adapter.request_device(&descriptor))
            .map_err(|e| GpuError::DeviceCreation(e.to_string()))
    }
}

fn watch_device(device: &wgpu::Device) -> Arc<AtomicBool> {
    device.on_uncaptured_error(Arc::new(|e| eprintln!("cadmium: wgpu error: {e}")));
    let lost = Arc::new(AtomicBool::new(false));
    let lost_flag = lost.clone();
    device.set_device_lost_callback(move |_, _| lost_flag.store(true, Ordering::SeqCst));
    lost
}

pub(crate) fn new_encoder(gpu: &GpuContext, label: &str) -> wgpu::CommandEncoder {
    gpu.device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) })
}

pub fn staging_buffer(gpu: &GpuContext, label: &str, size: u64) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    })
}

pub fn read_mapped<T>(
    gpu: &GpuContext,
    staging: &wgpu::Buffer,
    read: impl FnOnce(&[u8]) -> T,
) -> Result<T, GpuLost> {
    map_and_wait(gpu, &[staging])?;
    read_view(gpu, staging, read)
}

pub fn read_all_mapped<T>(
    gpu: &GpuContext,
    stagings: &[&wgpu::Buffer],
    mut read: impl FnMut(usize, &[u8]) -> T,
) -> Result<Vec<T>, GpuLost> {
    map_and_wait(gpu, stagings)?;
    stagings
        .iter()
        .enumerate()
        .map(|(index, staging)| read_view(gpu, staging, |bytes| read(index, bytes)))
        .collect()
}

fn map_and_wait(gpu: &GpuContext, stagings: &[&wgpu::Buffer]) -> Result<(), GpuLost> {
    if gpu.is_lost() {
        return Err(GpuLost);
    }
    let receivers: Vec<_> = stagings
        .iter()
        .map(|staging| {
            let (sender, receiver) = mpsc::channel();
            staging
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = sender.send(result);
                });
            receiver
        })
        .collect();
    let waited = gpu.device.poll(wgpu::PollType::wait_indefinitely()).is_ok();
    let mapped = waited
        && receivers
            .iter()
            .all(|receiver| matches!(receiver.try_recv(), Ok(Ok(()))));
    if !mapped || gpu.is_lost() {
        return Err(gpu.mark_lost());
    }
    Ok(())
}

fn read_view<T>(
    gpu: &GpuContext,
    staging: &wgpu::Buffer,
    read: impl FnOnce(&[u8]) -> T,
) -> Result<T, GpuLost> {
    let value = match staging.slice(..).get_mapped_range() {
        Ok(view) => Ok(read(&view)),
        Err(_) => Err(gpu.mark_lost()),
    };
    staging.unmap();
    value
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TexelRows {
    pub row_bytes: u32,
    pub padded_row_bytes: u32,
    pub rows: u32,
}

impl TexelRows {
    pub fn new(row_bytes: u32, rows: u32) -> TexelRows {
        TexelRows {
            row_bytes,
            padded_row_bytes: row_bytes.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT),
            rows,
        }
    }

    pub fn unpadded(row_bytes: u32, rows: u32) -> TexelRows {
        TexelRows {
            row_bytes,
            padded_row_bytes: row_bytes,
            rows,
        }
    }

    pub fn is_padded(&self) -> bool {
        self.padded_row_bytes != self.row_bytes
    }

    pub fn padded_size(&self) -> u64 {
        u64::from(self.padded_row_bytes) * u64::from(self.rows)
    }

    pub fn unpadded_size(&self) -> usize {
        self.row_bytes as usize * self.rows as usize
    }

    pub fn layout(&self) -> wgpu::TexelCopyBufferLayout {
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(self.padded_row_bytes),
            rows_per_image: Some(self.rows),
        }
    }

    pub fn unpad(&self, padded: &[u8]) -> Vec<u8> {
        let mut texels = Vec::with_capacity(self.unpadded_size());
        for row in padded
            .chunks_exact(self.padded_row_bytes as usize)
            .take(self.rows as usize)
        {
            texels.extend_from_slice(&row[..self.row_bytes as usize]);
        }
        texels
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
