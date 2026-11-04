use std::fmt;
use std::marker::PhantomData;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use ash::vk;
use cadmium_engine::gpu::GpuContext;
use gtk::gdk;
use gtk::prelude::*;
use wgpu::hal::{self, api::Vulkan};

const POOL_SIZE: usize = 3;
const PRESENT_VK_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;
const PRESENT_WGPU_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const PRESENT_FOURCC: u32 = drm_fourcc::DrmFourcc::Abgr8888 as u32;
const DRM_FORMAT_MOD_LINEAR: u64 = 0;
const DRM_FORMAT_MOD_VENDOR_SHIFT: u64 = 56;
const DRM_FORMAT_MOD_VENDOR_AMD: u64 = 0x02;
const AMD_FMT_MOD_DCC_BIT: u64 = 1 << 13;

const IMAGE_USAGE: vk::ImageUsageFlags = vk::ImageUsageFlags::STORAGE;
const TEXTURE_USAGE: wgpu::TextureUsages = wgpu::TextureUsages::STORAGE_BINDING;
const HAL_TEXTURE_USES: wgpu::TextureUses = wgpu::TextureUses::STORAGE_READ_ONLY
    .union(wgpu::TextureUses::STORAGE_WRITE_ONLY)
    .union(wgpu::TextureUses::STORAGE_READ_WRITE);

const IOC_WRITE: u64 = 1;
const IOC_READ: u64 = 2;
const IOC_DIRECTION_SHIFT: u64 = 30;
const IOC_SIZE_SHIFT: u64 = 16;
const IOC_TYPE_SHIFT: u64 = 8;
const DMA_BUF_IOCTL_TYPE: u8 = b'b';
const DMA_BUF_SYNC_WRITE: u32 = 2;
const DMA_BUF_IOCTL_EXPORT_SYNC_FILE: u64 =
    ioctl_number(IOC_READ | IOC_WRITE, DMA_BUF_IOCTL_TYPE, 2);
const DMA_BUF_IOCTL_IMPORT_SYNC_FILE: u64 = ioctl_number(IOC_WRITE, DMA_BUF_IOCTL_TYPE, 3);

#[repr(C)]
struct DmaBufSyncFile {
    flags: u32,
    fd: i32,
}

const fn ioctl_number(direction: u64, kind: u8, number: u8) -> u64 {
    (direction << IOC_DIRECTION_SHIFT)
        | ((std::mem::size_of::<DmaBufSyncFile>() as u64) << IOC_SIZE_SHIFT)
        | ((kind as u64) << IOC_TYPE_SHIFT)
        | number as u64
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModifierPolicy {
    Any,
    NoCompression,
    LinearOnly,
}

impl ModifierPolicy {
    fn allows(self, modifier: u64) -> bool {
        match self {
            ModifierPolicy::Any => true,
            ModifierPolicy::NoCompression => !is_compressed(modifier),
            ModifierPolicy::LinearOnly => modifier == DRM_FORMAT_MOD_LINEAR,
        }
    }
}

fn is_compressed(modifier: u64) -> bool {
    modifier >> DRM_FORMAT_MOD_VENDOR_SHIFT == DRM_FORMAT_MOD_VENDOR_AMD
        && modifier & AMD_FMT_MOD_DCC_BIT != 0
}

#[derive(Debug)]
pub enum DmabufError {
    Unsupported(String),
    Vulkan(String),
    Gdk(String),
    Io(std::io::Error),
    Poisoned(String),
}

impl fmt::Display for DmabufError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DmabufError::Unsupported(reason) => {
                write!(f, "dmabuf presentation unsupported: {reason}")
            }
            DmabufError::Vulkan(reason) => write!(f, "dmabuf Vulkan failure: {reason}"),
            DmabufError::Gdk(reason) => write!(f, "dmabuf GDK failure: {reason}"),
            DmabufError::Io(error) => write!(f, "dmabuf ioctl failure: {error}"),
            DmabufError::Poisoned(reason) => {
                write!(f, "dmabuf pool stopped after an earlier failure: {reason}")
            }
        }
    }
}

impl std::error::Error for DmabufError {}

impl From<std::io::Error> for DmabufError {
    fn from(error: std::io::Error) -> Self {
        DmabufError::Io(error)
    }
}

fn vulkan_error(context: &str) -> impl FnOnce(vk::Result) -> DmabufError + '_ {
    move |result| DmabufError::Vulkan(format!("{context}: {result}"))
}

struct DmabufSlot {
    texture: wgpu::Texture,
    dmabuf: Arc<OwnedFd>,
    stride: u32,
    offset: u32,
    frame_ready: vk::Semaphore,
    readers_done: vk::Semaphore,
    released_by_gtk: Arc<AtomicBool>,
    gpu_done: Arc<AtomicBool>,
}

impl DmabufSlot {
    fn is_free(&self) -> bool {
        self.released_by_gtk.load(Ordering::Acquire) && self.gpu_done.load(Ordering::Acquire)
    }
}

pub struct DmabufPool {
    slots: Vec<DmabufSlot>,
    fourcc: u32,
    modifier: u64,
    width: u32,
    height: u32,
    device: wgpu::Device,
    raw_device: ash::Device,
    semaphore_fd: ash::khr::external_semaphore_fd::Device,
    poisoned: Option<String>,
    single_submitter: PhantomData<*const ()>,
}

impl DmabufPool {
    #[expect(clippy::excessive_nesting, clippy::too_many_lines)]
    pub fn new(
        gpu: &GpuContext,
        display: &gdk::Display,
        width: u32,
        height: u32,
        policy: ModifierPolicy,
    ) -> Result<DmabufPool, DmabufError> {
        if !gpu.supports_dmabuf_export {
            return Err(DmabufError::Unsupported(
                "GPU device lacks dmabuf export or sync_file semaphores".into(),
            ));
        }
        if width == 0 || height == 0 {
            return Err(DmabufError::Unsupported(format!(
                "empty presentation size {width}x{height}"
            )));
        }
        let hal_device = unsafe { gpu.device.as_hal::<Vulkan>() }
            .ok_or_else(|| DmabufError::Unsupported("GPU device is not Vulkan".into()))?;
        let raw_device = hal_device.raw_device().clone();
        let instance = hal_device.shared_instance().raw_instance().clone();
        let physical_device = hal_device.raw_physical_device();

        if !supports_sync_fd_semaphores(&instance, physical_device) {
            return Err(DmabufError::Unsupported(
                "binary semaphores cannot import and export sync_file".into(),
            ));
        }
        let vulkan_modifiers =
            vulkan_exportable_storage_modifiers(&instance, physical_device, width, height);
        let gtk_formats = display.dmabuf_formats();
        let candidates = shared_modifiers(
            &vulkan_modifiers,
            |modifier| gtk_formats.contains(PRESENT_FOURCC, modifier),
            policy,
        );
        if candidates.is_empty() {
            return Err(DmabufError::Unsupported(format!(
                "no AB24 modifier shared by Vulkan {} and GTK under {policy:?}",
                format_modifiers(&vulkan_modifiers)
            )));
        }

        let semaphore_fd = ash::khr::external_semaphore_fd::Device::new(&instance, &raw_device);
        let mut pool = DmabufPool {
            slots: Vec::with_capacity(POOL_SIZE),
            fourcc: PRESENT_FOURCC,
            modifier: DRM_FORMAT_MOD_LINEAR,
            width,
            height,
            device: gpu.device.clone(),
            raw_device: raw_device.clone(),
            semaphore_fd,
            poisoned: None,
            single_submitter: PhantomData,
        };
        let exporter = SlotExporter {
            gpu,
            hal_device: &hal_device,
            raw_device: &raw_device,
            instance: &instance,
            physical_device,
            width,
            height,
        };
        for index in 0..POOL_SIZE {
            let modifiers = if index == 0 {
                candidates.as_slice()
            } else {
                std::slice::from_ref(&pool.modifier)
            };
            let (slot, modifier) = exporter.create_slot(modifiers)?;
            if index == 0 {
                pool.modifier = modifier;
            } else if modifier != pool.modifier {
                return Err(DmabufError::Vulkan(format!(
                    "slot modifier 0x{modifier:016x} differs from 0x{:016x}",
                    pool.modifier
                )));
            }
            pool.slots.push(slot);
        }
        Ok(pool)
    }

    pub fn modifier(&self) -> u64 {
        self.modifier
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    #[cfg(any(debug_assertions, feature = "smoke"))]
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    pub fn present_with(
        &mut self,
        gpu: &GpuContext,
        display: &gdk::Display,
        encode: impl FnOnce(&mut wgpu::CommandEncoder, &wgpu::Texture),
    ) -> Result<Option<gdk::Texture>, DmabufError> {
        if let Some(reason) = &self.poisoned {
            return Err(DmabufError::Poisoned(reason.clone()));
        }
        let _ = gpu.device.poll(wgpu::PollType::Poll);
        let Some(index) = self.slots.iter().position(DmabufSlot::is_free) else {
            return Ok(None);
        };
        self.present_into(index, gpu, display, encode)
            .map(Some)
            .inspect_err(|error| self.poisoned = Some(error.to_string()))
    }

    #[expect(clippy::excessive_nesting)]
    fn present_into(
        &self,
        index: usize,
        gpu: &GpuContext,
        display: &gdk::Display,
        encode: impl FnOnce(&mut wgpu::CommandEncoder, &wgpu::Texture),
    ) -> Result<gdk::Texture, DmabufError> {
        let slot = &self.slots[index];
        let gtk_reads = export_sync_file(&slot.dmabuf)?;
        self.import_temporary_sync_file(slot.readers_done, gtk_reads)?;

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("dmabuf_present"),
            });
        encode(&mut encoder, &slot.texture);
        let commands = encoder.finish();

        let fenced = hal_queue(gpu).map(|queue| {
            queue.add_wait_semaphore(
                slot.readers_done,
                None,
                vk::PipelineStageFlags::ALL_COMMANDS,
            );
            queue.add_signal_semaphore(slot.frame_ready, None);
        });
        gpu.queue.submit([commands]);
        fenced?;
        {
            let queue = hal_queue(gpu)?;
            let wait_left_behind = queue.remove_wait_semaphore(slot.readers_done);
            let signal_left_behind = queue.remove_signal_semaphore(slot.frame_ready);
            if wait_left_behind || signal_left_behind {
                return Err(DmabufError::Vulkan(
                    "queue submit did not consume the dmabuf semaphores".into(),
                ));
            }
        }

        slot.released_by_gtk.store(false, Ordering::Release);
        slot.gpu_done.store(false, Ordering::Release);
        let gpu_done = Arc::clone(&slot.gpu_done);
        gpu.queue
            .on_submitted_work_done(move || gpu_done.store(true, Ordering::Release));

        let frame_fence = self.export_frame_fence(slot.frame_ready)?;
        if let Some(fence) = frame_fence {
            import_sync_file(&slot.dmabuf, &fence)?;
        }
        self.build_texture(display, slot)
    }

    fn import_temporary_sync_file(
        &self,
        semaphore: vk::Semaphore,
        sync_file: OwnedFd,
    ) -> Result<(), DmabufError> {
        let raw = sync_file.into_raw_fd();
        let info = vk::ImportSemaphoreFdInfoKHR::default()
            .semaphore(semaphore)
            .flags(vk::SemaphoreImportFlags::TEMPORARY)
            .handle_type(vk::ExternalSemaphoreHandleTypeFlags::SYNC_FD)
            .fd(raw);
        unsafe { self.semaphore_fd.import_semaphore_fd(&info) }.map_err(|result| {
            drop(unsafe { OwnedFd::from_raw_fd(raw) });
            DmabufError::Vulkan(format!("import GTK read fence: {result}"))
        })
    }

    fn export_frame_fence(&self, semaphore: vk::Semaphore) -> Result<Option<OwnedFd>, DmabufError> {
        let info = vk::SemaphoreGetFdInfoKHR::default()
            .semaphore(semaphore)
            .handle_type(vk::ExternalSemaphoreHandleTypeFlags::SYNC_FD);
        let raw = unsafe { self.semaphore_fd.get_semaphore_fd(&info) }
            .map_err(vulkan_error("export frame fence"))?;
        Ok((raw >= 0).then(|| unsafe { OwnedFd::from_raw_fd(raw) }))
    }

    fn build_texture(
        &self,
        display: &gdk::Display,
        slot: &DmabufSlot,
    ) -> Result<gdk::Texture, DmabufError> {
        let released_by_gtk = Arc::clone(&slot.released_by_gtk);
        let lent_to_gtk = Arc::new(Mutex::new(Some(Arc::clone(&slot.dmabuf))));
        let held_by_release = Arc::clone(&lent_to_gtk);
        let builder = gdk::DmabufTextureBuilder::new()
            .set_display(display)
            .set_width(self.width)
            .set_height(self.height)
            .set_fourcc(self.fourcc)
            .set_modifier(self.modifier)
            .set_n_planes(1)
            .set_offset(0, slot.offset)
            .set_stride(0, slot.stride);
        let builder = unsafe { builder.set_fd(0, slot.dmabuf.as_raw_fd()) };
        unsafe {
            builder.build_with_release_func(move || {
                released_by_gtk.store(true, Ordering::Release);
                take_lent_dmabuf(&held_by_release);
            })
        }
        .map_err(|error| {
            take_lent_dmabuf(&lent_to_gtk);
            DmabufError::Gdk(error.to_string())
        })
    }
}

impl Drop for DmabufPool {
    fn drop(&mut self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        let semaphores: Vec<vk::Semaphore> = self
            .slots
            .iter()
            .flat_map(|slot| [slot.frame_ready, slot.readers_done])
            .collect();
        self.slots.clear();
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        for semaphore in semaphores {
            unsafe { self.raw_device.destroy_semaphore(semaphore, None) };
        }
    }
}

struct SlotExporter<'a> {
    gpu: &'a GpuContext,
    hal_device: &'a hal::vulkan::Device,
    raw_device: &'a ash::Device,
    instance: &'a ash::Instance,
    physical_device: vk::PhysicalDevice,
    width: u32,
    height: u32,
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
    fn create_slot(&self, modifiers: &[u64]) -> Result<(DmabufSlot, u64), DmabufError> {
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

    #[expect(clippy::too_many_lines)]
    fn wrap_texture(&self, image: vk::Image, memory: vk::DeviceMemory) -> wgpu::Texture {
        let size = wgpu::Extent3d {
            width: self.width,
            height: self.height,
            depth_or_array_layers: 1,
        };
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
        let hal_texture = unsafe {
            self.hal_device.texture_from_raw(
                image,
                &hal_descriptor,
                Some(release_image),
                hal::vulkan::TextureMemory::External,
            )
        };
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

fn take_lent_dmabuf(lent: &Mutex<Option<Arc<OwnedFd>>>) -> Option<Arc<OwnedFd>> {
    lent.lock().unwrap_or_else(PoisonError::into_inner).take()
}

fn hal_queue(
    gpu: &GpuContext,
) -> Result<impl std::ops::Deref<Target = hal::vulkan::Queue>, DmabufError> {
    unsafe { gpu.queue.as_hal::<Vulkan>() }
        .ok_or_else(|| DmabufError::Unsupported("GPU queue is not Vulkan".into()))
}

fn shared_modifiers(
    vulkan_modifiers: &[u64],
    gtk_accepts: impl Fn(u64) -> bool,
    policy: ModifierPolicy,
) -> Vec<u64> {
    let mut shared: Vec<u64> = vulkan_modifiers
        .iter()
        .copied()
        .filter(|&modifier| policy.allows(modifier))
        .filter(|&modifier| gtk_accepts(modifier))
        .collect();
    shared.sort_by_key(|&modifier| modifier == DRM_FORMAT_MOD_LINEAR);
    shared.dedup();
    shared
}

fn format_modifiers(modifiers: &[u64]) -> String {
    let listed: Vec<String> = modifiers.iter().map(|m| format!("0x{m:016x}")).collect();
    format!("[{}]", listed.join(", "))
}

fn supports_sync_fd_semaphores(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
) -> bool {
    let info = vk::PhysicalDeviceExternalSemaphoreInfo::default()
        .handle_type(vk::ExternalSemaphoreHandleTypeFlags::SYNC_FD);
    let mut properties = vk::ExternalSemaphoreProperties::default();
    unsafe {
        instance.get_physical_device_external_semaphore_properties(
            physical_device,
            &info,
            &mut properties,
        )
    };
    properties.external_semaphore_features.contains(
        vk::ExternalSemaphoreFeatureFlags::EXPORTABLE
            | vk::ExternalSemaphoreFeatureFlags::IMPORTABLE,
    )
}

fn vulkan_exportable_storage_modifiers(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    width: u32,
    height: u32,
) -> Vec<u64> {
    let count = {
        let mut list = vk::DrmFormatModifierPropertiesListEXT::default();
        let mut properties = vk::FormatProperties2::default().push_next(&mut list);
        unsafe {
            instance.get_physical_device_format_properties2(
                physical_device,
                PRESENT_VK_FORMAT,
                &mut properties,
            )
        };
        list.drm_format_modifier_count as usize
    };
    let mut entries = vec![vk::DrmFormatModifierPropertiesEXT::default(); count];
    {
        let mut list = vk::DrmFormatModifierPropertiesListEXT::default()
            .drm_format_modifier_properties(&mut entries);
        let mut properties = vk::FormatProperties2::default().push_next(&mut list);
        unsafe {
            instance.get_physical_device_format_properties2(
                physical_device,
                PRESENT_VK_FORMAT,
                &mut properties,
            )
        };
    }
    single_plane_storage_modifiers(&entries, |modifier| {
        is_exportable_at_size(instance, physical_device, modifier, width, height)
    })
}

fn single_plane_storage_modifiers(
    entries: &[vk::DrmFormatModifierPropertiesEXT],
    exportable: impl Fn(u64) -> bool,
) -> Vec<u64> {
    entries
        .iter()
        .filter(|entry| entry.drm_format_modifier_plane_count == 1)
        .filter(|entry| {
            entry
                .drm_format_modifier_tiling_features
                .contains(vk::FormatFeatureFlags::STORAGE_IMAGE)
        })
        .map(|entry| entry.drm_format_modifier)
        .filter(|&modifier| exportable(modifier))
        .collect()
}

fn is_exportable_at_size(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    modifier: u64,
    width: u32,
    height: u32,
) -> bool {
    let mut modifier_info = vk::PhysicalDeviceImageDrmFormatModifierInfoEXT::default()
        .drm_format_modifier(modifier)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    let mut external_info = vk::PhysicalDeviceExternalImageFormatInfo::default()
        .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
    let info = vk::PhysicalDeviceImageFormatInfo2::default()
        .format(PRESENT_VK_FORMAT)
        .ty(vk::ImageType::TYPE_2D)
        .tiling(vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT)
        .usage(IMAGE_USAGE)
        .push_next(&mut modifier_info)
        .push_next(&mut external_info);
    let mut external_properties = vk::ExternalImageFormatProperties::default();
    let (supported, max_extent) = {
        let mut properties =
            vk::ImageFormatProperties2::default().push_next(&mut external_properties);
        let result = unsafe {
            instance.get_physical_device_image_format_properties2(
                physical_device,
                &info,
                &mut properties,
            )
        };
        (
            result.is_ok(),
            properties.image_format_properties.max_extent,
        )
    };
    supported
        && max_extent.width >= width
        && max_extent.height >= height
        && external_properties
            .external_memory_properties
            .external_memory_features
            .contains(vk::ExternalMemoryFeatureFlags::EXPORTABLE)
}

fn export_sync_file(dmabuf: &OwnedFd) -> Result<OwnedFd, DmabufError> {
    let mut request = DmaBufSyncFile {
        flags: DMA_BUF_SYNC_WRITE,
        fd: -1,
    };
    dma_buf_ioctl(
        dmabuf.as_raw_fd(),
        DMA_BUF_IOCTL_EXPORT_SYNC_FILE,
        &mut request,
    )?;
    if request.fd < 0 {
        return Err(DmabufError::Io(std::io::Error::other(
            "EXPORT_SYNC_FILE returned no fence",
        )));
    }
    Ok(unsafe { OwnedFd::from_raw_fd(request.fd) })
}

fn import_sync_file(dmabuf: &OwnedFd, fence: &OwnedFd) -> Result<(), DmabufError> {
    let mut request = DmaBufSyncFile {
        flags: DMA_BUF_SYNC_WRITE,
        fd: fence.as_raw_fd(),
    };
    dma_buf_ioctl(
        dmabuf.as_raw_fd(),
        DMA_BUF_IOCTL_IMPORT_SYNC_FILE,
        &mut request,
    )
}

fn dma_buf_ioctl(
    fd: RawFd,
    request: u64,
    argument: &mut DmaBufSyncFile,
) -> Result<(), DmabufError> {
    loop {
        let result =
            unsafe { libc::ioctl(fd, request as libc::Ioctl, argument as *mut DmaBufSyncFile) };
        if result == 0 {
            return Ok(());
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::Interrupted {
            return Err(DmabufError::Io(error));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AMD_DCC_MODIFIER: u64 = 0x0200_0000_0008_2305;
    const AMD_TILED_MODIFIER: u64 = 0x0200_0000_0000_0305;
    const AMD_GFX10_DCC_MODIFIER: u64 = 0x0200_0000_0004_3b03;
    const DCC_METADATA_PLANES: u32 = 2;

    trait SendProbe<Marker> {
        fn probe() {}
    }
    impl<T: ?Sized> SendProbe<()> for T {}
    struct ImplementsSend;
    impl<T: ?Sized + Send> SendProbe<ImplementsSend> for T {}

    fn modifier_entry(
        modifier: u64,
        planes: u32,
        features: vk::FormatFeatureFlags,
    ) -> vk::DrmFormatModifierPropertiesEXT {
        vk::DrmFormatModifierPropertiesEXT {
            drm_format_modifier: modifier,
            drm_format_modifier_plane_count: planes,
            drm_format_modifier_tiling_features: features,
        }
    }

    #[test]
    fn pool_submits_stay_on_the_main_thread() {
        <DmabufPool as SendProbe<_>>::probe();
    }

    #[test]
    fn dcc_before_gfx12_needs_a_metadata_plane_and_cannot_be_one_dmabuf_plane() {
        let storage = vk::FormatFeatureFlags::STORAGE_IMAGE;
        let entries = [
            modifier_entry(AMD_GFX10_DCC_MODIFIER, DCC_METADATA_PLANES, storage),
            modifier_entry(AMD_DCC_MODIFIER, 1, storage),
            modifier_entry(DRM_FORMAT_MOD_LINEAR, 1, storage),
        ];
        assert_eq!(
            single_plane_storage_modifiers(&entries, |_| true),
            vec![AMD_DCC_MODIFIER, DRM_FORMAT_MOD_LINEAR]
        );
    }

    #[test]
    fn modifiers_without_storage_or_export_are_not_presentable() {
        let entries = [
            modifier_entry(AMD_TILED_MODIFIER, 1, vk::FormatFeatureFlags::SAMPLED_IMAGE),
            modifier_entry(AMD_DCC_MODIFIER, 1, vk::FormatFeatureFlags::STORAGE_IMAGE),
            modifier_entry(
                DRM_FORMAT_MOD_LINEAR,
                1,
                vk::FormatFeatureFlags::STORAGE_IMAGE,
            ),
        ];
        assert_eq!(
            single_plane_storage_modifiers(&entries, |modifier| modifier != AMD_DCC_MODIFIER),
            vec![DRM_FORMAT_MOD_LINEAR]
        );
    }

    #[test]
    fn no_compression_policy_drops_amd_dcc_but_keeps_plain_tiling() {
        let shared = shared_modifiers(
            &[AMD_DCC_MODIFIER, AMD_TILED_MODIFIER, DRM_FORMAT_MOD_LINEAR],
            |_| true,
            ModifierPolicy::NoCompression,
        );
        assert_eq!(shared, vec![AMD_TILED_MODIFIER, DRM_FORMAT_MOD_LINEAR]);
    }

    #[test]
    fn dma_buf_sync_file_ioctls_match_the_kernel_uapi() {
        assert_eq!(DMA_BUF_IOCTL_EXPORT_SYNC_FILE, 0xc008_6202);
        assert_eq!(DMA_BUF_IOCTL_IMPORT_SYNC_FILE, 0x4008_6203);
    }

    #[test]
    fn present_fourcc_is_ab24() {
        assert_eq!(PRESENT_FOURCC.to_le_bytes(), *b"AB24");
    }

    #[test]
    fn linear_modifier_is_tried_last() {
        let shared = shared_modifiers(
            &[DRM_FORMAT_MOD_LINEAR, AMD_DCC_MODIFIER, AMD_TILED_MODIFIER],
            |_| true,
            ModifierPolicy::Any,
        );
        assert_eq!(
            shared,
            vec![AMD_DCC_MODIFIER, AMD_TILED_MODIFIER, DRM_FORMAT_MOD_LINEAR]
        );
    }

    #[test]
    fn modifiers_gtk_cannot_import_are_dropped() {
        let shared = shared_modifiers(
            &[AMD_DCC_MODIFIER, AMD_TILED_MODIFIER, DRM_FORMAT_MOD_LINEAR],
            |modifier| modifier != AMD_DCC_MODIFIER,
            ModifierPolicy::Any,
        );
        assert_eq!(shared, vec![AMD_TILED_MODIFIER, DRM_FORMAT_MOD_LINEAR]);
    }

    #[test]
    fn linear_only_policy_keeps_just_the_linear_modifier() {
        let shared = shared_modifiers(
            &[AMD_DCC_MODIFIER, DRM_FORMAT_MOD_LINEAR],
            |_| true,
            ModifierPolicy::LinearOnly,
        );
        assert_eq!(shared, vec![DRM_FORMAT_MOD_LINEAR]);
    }
}
