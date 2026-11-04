use std::fmt;
use std::marker::PhantomData;
use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use ash::vk;

mod export;
mod modifiers;
mod pool;
mod sync;
#[cfg(test)]
mod tests;

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
