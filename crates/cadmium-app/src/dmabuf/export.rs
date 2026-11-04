use std::os::fd::{FromRawFd, OwnedFd};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use ash::vk;
use cadmium_engine::gpu::GpuContext;
use wgpu::hal::{self, api::Vulkan};

use super::{DmabufError, DmabufSlot, IMAGE_USAGE, PRESENT_VK_FORMAT, vulkan_error};

const PRESENT_WGPU_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const TEXTURE_USAGE: wgpu::TextureUsages = wgpu::TextureUsages::STORAGE_BINDING;
const HAL_TEXTURE_USES: wgpu::TextureUses = wgpu::TextureUses::STORAGE_READ_ONLY
    .union(wgpu::TextureUses::STORAGE_WRITE_ONLY)
    .union(wgpu::TextureUses::STORAGE_READ_WRITE);

pub(super) struct SlotExporter<'a> {
    pub(super) gpu: &'a GpuContext,
    pub(super) hal_device: &'a hal::vulkan::Device,
    pub(super) raw_device: &'a ash::Device,
    pub(super) instance: &'a ash::Instance,
    pub(super) physical_device: vk::PhysicalDevice,
    pub(super) width: u32,
    pub(super) height: u32,
}

struct ExportedImage {
    image: vk::Image,
    memory: vk::DeviceMemory,
    modifier: u64,
    dmabuf: OwnedFd,
    stride: u32,
    offset: u32,
}

impl SlotExporter<'_> {
    pub(super) fn create_slot(&self, modifiers: &[u64]) -> Result<(DmabufSlot, u64), DmabufError> {
        let exported = self.export_image(modifiers)?;
        let modifier = exported.modifier;
        let texture = self.wrap_texture(exported.image, exported.memory);
        let semaphores = self.create_semaphores();
        let (frame_ready, readers_done) = match semaphores {
            Ok(pair) => pair,
            Err(error) => {
                drop(texture);
                return Err(error);
            }
        };
        let slot = DmabufSlot {
            texture,
            dmabuf: Arc::new(exported.dmabuf),
            stride: exported.stride,
            offset: exported.offset,
            frame_ready,
            readers_done,
            released_by_gtk: Arc::new(AtomicBool::new(true)),
            gpu_done: Arc::new(AtomicBool::new(true)),
        };
        Ok((slot, modifier))
    }

    fn export_image(&self, modifiers: &[u64]) -> Result<ExportedImage, DmabufError> {
        let mut modifier_list =
            vk::ImageDrmFormatModifierListCreateInfoEXT::default().drm_format_modifiers(modifiers);
        let mut external = vk::ExternalMemoryImageCreateInfo::default()
            .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
        let info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(PRESENT_VK_FORMAT)
            .extent(vk::Extent3D {
                width: self.width,
                height: self.height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT)
            .usage(IMAGE_USAGE)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .push_next(&mut modifier_list)
            .push_next(&mut external);
        let image = unsafe { self.raw_device.create_image(&info, None) }
            .map_err(vulkan_error("create dmabuf image"))?;
        let memory = match self.allocate_dedicated_memory(image) {
            Ok(memory) => memory,
            Err(error) => {
                unsafe { self.raw_device.destroy_image(image, None) };
                return Err(error);
            }
        };
        self.describe_export(image, memory).inspect_err(|_| unsafe {
            self.raw_device.destroy_image(image, None);
            self.raw_device.free_memory(memory, None);
        })
    }

    fn allocate_dedicated_memory(&self, image: vk::Image) -> Result<vk::DeviceMemory, DmabufError> {
        let requirements = unsafe { self.raw_device.get_image_memory_requirements(image) };
        let properties = unsafe {
            self.instance
                .get_physical_device_memory_properties(self.physical_device)
        };
        let allowed = |index: &u32| requirements.memory_type_bits & (1 << index) != 0;
        let memory_type = (0..properties.memory_type_count)
            .filter(allowed)
            .find(|&index| {
                properties.memory_types[index as usize]
                    .property_flags
                    .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
            })
            .or_else(|| (0..properties.memory_type_count).find(allowed))
            .ok_or_else(|| DmabufError::Vulkan("no memory type for dmabuf image".into()))?;
        let mut export = vk::ExportMemoryAllocateInfo::default()
            .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
        let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
        let info = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(memory_type)
            .push_next(&mut export)
            .push_next(&mut dedicated);
        let memory = unsafe { self.raw_device.allocate_memory(&info, None) }
            .map_err(vulkan_error("allocate dmabuf memory"))?;
        if let Err(result) = unsafe { self.raw_device.bind_image_memory(image, memory, 0) } {
            unsafe { self.raw_device.free_memory(memory, None) };
            return Err(DmabufError::Vulkan(format!("bind dmabuf memory: {result}")));
        }
        Ok(memory)
    }

    fn describe_export(
        &self,
        image: vk::Image,
        memory: vk::DeviceMemory,
    ) -> Result<ExportedImage, DmabufError> {
        let mut modifier_properties = vk::ImageDrmFormatModifierPropertiesEXT::default();
        unsafe {
            ash::ext::image_drm_format_modifier::Device::new(self.instance, self.raw_device)
                .get_image_drm_format_modifier_properties(image, &mut modifier_properties)
        }
        .map_err(vulkan_error("read chosen modifier"))?;
        let layout = unsafe {
            self.raw_device.get_image_subresource_layout(
                image,
                vk::ImageSubresource {
                    aspect_mask: vk::ImageAspectFlags::MEMORY_PLANE_0_EXT,
                    mip_level: 0,
                    array_layer: 0,
                },
            )
        };
        let stride = u32::try_from(layout.row_pitch).map_err(|_| {
            DmabufError::Vulkan(format!("row pitch {} too large", layout.row_pitch))
        })?;
        let offset = u32::try_from(layout.offset).map_err(|_| {
            DmabufError::Vulkan(format!("plane offset {} too large", layout.offset))
        })?;
        let info = vk::MemoryGetFdInfoKHR::default()
            .memory(memory)
            .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
        let raw = unsafe {
            ash::khr::external_memory_fd::Device::new(self.instance, self.raw_device)
                .get_memory_fd(&info)
        }
        .map_err(vulkan_error("export dmabuf fd"))?;
        Ok(ExportedImage {
            image,
            memory,
            modifier: modifier_properties.drm_format_modifier,
            dmabuf: unsafe { OwnedFd::from_raw_fd(raw) },
            stride,
            offset,
        })
    }

    fn wrap_texture(&self, image: vk::Image, memory: vk::DeviceMemory) -> wgpu::Texture {
        let size = wgpu::Extent3d {
            width: self.width,
            height: self.height,
            depth_or_array_layers: 1,
        };
        let hal_texture = self.hal_texture(image, memory, size);
        unsafe {
            self.gpu.device.create_texture_from_hal::<Vulkan>(
                hal_texture,
                &wgpu::TextureDescriptor {
                    label: Some("dmabuf_present"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: PRESENT_WGPU_FORMAT,
                    usage: TEXTURE_USAGE,
                    view_formats: &[],
                },
                wgpu::TextureUses::UNINITIALIZED,
            )
        }
    }

    fn hal_texture(
        &self,
        image: vk::Image,
        memory: vk::DeviceMemory,
        size: wgpu::Extent3d,
    ) -> hal::vulkan::Texture {
        let hal_descriptor = hal::TextureDescriptor {
            label: Some("dmabuf_present"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: PRESENT_WGPU_FORMAT,
            usage: HAL_TEXTURE_USES,
            memory_flags: hal::MemoryFlags::empty(),
            view_formats: vec![],
        };
        let owner = self.raw_device.clone();
        let release_image: hal::DropCallback = Box::new(move || unsafe {
            owner.destroy_image(image, None);
            owner.free_memory(memory, None);
        });
        unsafe {
            self.hal_device.texture_from_raw(
                image,
                &hal_descriptor,
                Some(release_image),
                hal::vulkan::TextureMemory::External,
            )
        }
    }

    fn create_semaphores(&self) -> Result<(vk::Semaphore, vk::Semaphore), DmabufError> {
        let mut exportable = vk::ExportSemaphoreCreateInfo::default()
            .handle_types(vk::ExternalSemaphoreHandleTypeFlags::SYNC_FD);
        let frame_ready = unsafe {
            self.raw_device.create_semaphore(
                &vk::SemaphoreCreateInfo::default().push_next(&mut exportable),
                None,
            )
        }
        .map_err(vulkan_error("create frame semaphore"))?;
        match unsafe {
            self.raw_device
                .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
        } {
            Ok(readers_done) => Ok((frame_ready, readers_done)),
            Err(result) => {
                unsafe { self.raw_device.destroy_semaphore(frame_ready, None) };
                Err(DmabufError::Vulkan(format!(
                    "create reader semaphore: {result}"
                )))
            }
        }
    }
}
