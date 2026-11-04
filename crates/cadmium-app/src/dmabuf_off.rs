use std::fmt;

use cadmium_engine::gpu::GpuContext;
use gtk::gdk;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModifierPolicy {
    Any,
    NoCompression,
    LinearOnly,
}

#[derive(Debug)]
pub enum DmabufError {
    Unsupported,
}

impl fmt::Display for DmabufError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "dmabuf presentation exists only on Linux")
    }
}

impl std::error::Error for DmabufError {}

pub enum DmabufPool {}

impl DmabufPool {
    pub fn new(
        _gpu: &GpuContext,
        _display: &gdk::Display,
        _width: u32,
        _height: u32,
        _policy: ModifierPolicy,
    ) -> Result<DmabufPool, DmabufError> {
        Err(DmabufError::Unsupported)
    }

    pub fn modifier(&self) -> u64 {
        match *self {}
    }

    pub fn size(&self) -> (u32, u32) {
        match *self {}
    }

    pub fn present_with(
        &mut self,
        _gpu: &GpuContext,
        _display: &gdk::Display,
        _encode: impl FnOnce(&mut wgpu::CommandEncoder, &wgpu::Texture),
    ) -> Result<Option<gdk::Texture>, DmabufError> {
        match *self {}
    }
}
