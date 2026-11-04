use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};

use ash::vk;

use super::{DmabufError, DmabufPool, vulkan_error};

const IOC_WRITE: u64 = 1;
const IOC_READ: u64 = 2;
const IOC_DIRECTION_SHIFT: u64 = 30;
const IOC_SIZE_SHIFT: u64 = 16;
const IOC_TYPE_SHIFT: u64 = 8;
const DMA_BUF_IOCTL_TYPE: u8 = b'b';
const DMA_BUF_SYNC_WRITE: u32 = 2;
pub(super) const DMA_BUF_IOCTL_EXPORT_SYNC_FILE: u64 =
    ioctl_number(IOC_READ | IOC_WRITE, DMA_BUF_IOCTL_TYPE, 2);
pub(super) const DMA_BUF_IOCTL_IMPORT_SYNC_FILE: u64 =
    ioctl_number(IOC_WRITE, DMA_BUF_IOCTL_TYPE, 3);

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

pub(super) fn supports_sync_fd_semaphores(
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

impl DmabufPool {
    pub(super) fn import_temporary_sync_file(
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

    pub(super) fn export_frame_fence(
        &self,
        semaphore: vk::Semaphore,
    ) -> Result<Option<OwnedFd>, DmabufError> {
        let info = vk::SemaphoreGetFdInfoKHR::default()
            .semaphore(semaphore)
            .handle_type(vk::ExternalSemaphoreHandleTypeFlags::SYNC_FD);
        let raw = unsafe { self.semaphore_fd.get_semaphore_fd(&info) }
            .map_err(vulkan_error("export frame fence"))?;
        Ok((raw >= 0).then(|| unsafe { OwnedFd::from_raw_fd(raw) }))
    }
}

pub(super) fn export_sync_file(dmabuf: &OwnedFd) -> Result<OwnedFd, DmabufError> {
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

pub(super) fn import_sync_file(dmabuf: &OwnedFd, fence: &OwnedFd) -> Result<(), DmabufError> {
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
