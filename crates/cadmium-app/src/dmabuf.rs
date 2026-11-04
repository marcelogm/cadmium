use std::fmt;
use std::marker::PhantomData;
use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use ash::vk;
use cadmium_engine::gpu::GpuContext;
use gtk::gdk;
use gtk::prelude::*;
use wgpu::hal::{self, api::Vulkan};

use export::SlotExporter;
use modifiers::{
    DRM_FORMAT_MOD_LINEAR, format_modifiers, shared_modifiers, vulkan_exportable_storage_modifiers,
};
use sync::{export_sync_file, import_sync_file, supports_sync_fd_semaphores};

mod export;
mod modifiers;
mod sync;
#[cfg(test)]
mod tests;

const POOL_SIZE: usize = 3;
const PRESENT_VK_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;
const PRESENT_FOURCC: u32 = drm_fourcc::DrmFourcc::Abgr8888 as u32;

const IMAGE_USAGE: vk::ImageUsageFlags = vk::ImageUsageFlags::STORAGE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModifierPolicy {
    Any,
    NoCompression,
    LinearOnly,
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

fn take_lent_dmabuf(lent: &Mutex<Option<Arc<OwnedFd>>>) -> Option<Arc<OwnedFd>> {
    lent.lock().unwrap_or_else(PoisonError::into_inner).take()
}

fn hal_queue(
    gpu: &GpuContext,
) -> Result<impl std::ops::Deref<Target = hal::vulkan::Queue>, DmabufError> {
    unsafe { gpu.queue.as_hal::<Vulkan>() }
        .ok_or_else(|| DmabufError::Unsupported("GPU queue is not Vulkan".into()))
}
