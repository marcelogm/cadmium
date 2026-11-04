use std::marker::PhantomData;
use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, PoisonError};

use ash::vk;
use cadmium_engine::gpu::GpuContext;
use gtk::gdk;
use wgpu::hal::{self, api::Vulkan};

use super::export::SlotExporter;
use super::modifiers::{
    DRM_FORMAT_MOD_LINEAR, candidate_modifiers, vulkan_exportable_storage_modifiers,
};
use super::sync::{export_sync_file, import_sync_file, supports_sync_fd_semaphores};
use super::{DmabufError, DmabufPool, DmabufSlot, ModifierPolicy, PRESENT_FOURCC};

const POOL_SIZE: usize = 3;

impl DmabufSlot {
    fn is_free(&self) -> bool {
        self.released_by_gtk.load(Ordering::Acquire) && self.gpu_done.load(Ordering::Acquire)
    }

    fn submit_fenced(
        &self,
        gpu: &GpuContext,
        commands: wgpu::CommandBuffer,
    ) -> Result<(), DmabufError> {
        let fenced = hal_queue(gpu).map(|queue| {
            queue.add_wait_semaphore(
                self.readers_done,
                None,
                vk::PipelineStageFlags::ALL_COMMANDS,
            );
            queue.add_signal_semaphore(self.frame_ready, None);
        });
        gpu.queue.submit([commands]);
        fenced?;
        let queue = hal_queue(gpu)?;
        let wait_left_behind = queue.remove_wait_semaphore(self.readers_done);
        let signal_left_behind = queue.remove_signal_semaphore(self.frame_ready);
        if wait_left_behind || signal_left_behind {
            return Err(DmabufError::Vulkan(
                "queue submit did not consume the dmabuf semaphores".into(),
            ));
        }
        Ok(())
    }
}

impl DmabufPool {
    pub fn new(
        gpu: &GpuContext,
        display: &gdk::Display,
        width: u32,
        height: u32,
        policy: ModifierPolicy,
    ) -> Result<DmabufPool, DmabufError> {
        ensure_supported(gpu, width, height)?;
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
        let candidates = candidate_modifiers(&vulkan_modifiers, display, policy)?;

        let mut pool = DmabufPool::without_slots(gpu, &instance, &raw_device, width, height);
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
            pool.add_slot(&exporter, &candidates, index)?;
        }
        Ok(pool)
    }

    fn without_slots(
        gpu: &GpuContext,
        instance: &ash::Instance,
        raw_device: &ash::Device,
        width: u32,
        height: u32,
    ) -> DmabufPool {
        let semaphore_fd = ash::khr::external_semaphore_fd::Device::new(instance, raw_device);
        DmabufPool {
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
        }
    }

    fn add_slot(
        &mut self,
        exporter: &SlotExporter<'_>,
        candidates: &[u64],
        index: usize,
    ) -> Result<(), DmabufError> {
        let modifiers = if index == 0 {
            candidates
        } else {
            std::slice::from_ref(&self.modifier)
        };
        let (slot, modifier) = exporter.create_slot(modifiers)?;
        if index == 0 {
            self.modifier = modifier;
        } else if modifier != self.modifier {
            return Err(DmabufError::Vulkan(format!(
                "slot modifier 0x{modifier:016x} differs from 0x{:016x}",
                self.modifier
            )));
        }
        self.slots.push(slot);
        Ok(())
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
        slot.submit_fenced(gpu, commands)?;

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

fn ensure_supported(gpu: &GpuContext, width: u32, height: u32) -> Result<(), DmabufError> {
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
    Ok(())
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
