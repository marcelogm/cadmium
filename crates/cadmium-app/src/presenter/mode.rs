use gtk::glib;

use crate::LOG_DOMAIN;
use crate::dmabuf::ModifierPolicy;

const PRESENT_MODE_VARIABLE: &str = "CADMIUM_PRESENT";
const VULKAN_RENDERER_MARK: &str = "Vulkan";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentMode {
    Readback,
    Dmabuf(ModifierPolicy),
}

impl PresentMode {
    pub fn from_name(name: &str) -> Option<PresentMode> {
        match name {
            "readback" => Some(PresentMode::Readback),
            "dmabuf" => Some(PresentMode::Dmabuf(ModifierPolicy::Any)),
            "dmabuf-nodcc" => Some(PresentMode::Dmabuf(ModifierPolicy::NoCompression)),
            "linear" => Some(PresentMode::Dmabuf(ModifierPolicy::LinearOnly)),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            PresentMode::Readback => "readback",
            PresentMode::Dmabuf(ModifierPolicy::Any) => "dmabuf",
            PresentMode::Dmabuf(ModifierPolicy::NoCompression) => "dmabuf-nodcc",
            PresentMode::Dmabuf(ModifierPolicy::LinearOnly) => "linear",
        }
    }

    pub(super) fn forced_by_environment() -> Option<PresentMode> {
        let requested = std::env::var(PRESENT_MODE_VARIABLE).ok()?;
        let mode = PresentMode::from_name(&requested);
        if mode.is_none() {
            glib::g_warning!(LOG_DOMAIN, "ignoring {PRESENT_MODE_VARIABLE}={requested:?}");
        }
        mode
    }
}

pub fn choose_mode(
    forced: Option<PresentMode>,
    gpu_exports_dmabuf: bool,
    renderer_name: &str,
) -> PresentMode {
    match forced {
        Some(mode) => mode,
        None if gpu_exports_dmabuf && renderer_name.contains(VULKAN_RENDERER_MARK) => {
            PresentMode::Dmabuf(ModifierPolicy::Any)
        }
        None => PresentMode::Readback,
    }
}

pub(super) fn mode_reason(forced: bool, gpu_exports_dmabuf: bool, renderer_name: &str) -> String {
    if forced {
        format!("forced by {PRESENT_MODE_VARIABLE}")
    } else if !gpu_exports_dmabuf {
        "the GPU cannot export dmabufs".into()
    } else if !renderer_name.contains(VULKAN_RENDERER_MARK) {
        format!("GTK renders with {renderer_name}, not Vulkan")
    } else {
        format!("GTK renders with {renderer_name}")
    }
}

pub(super) fn describe_modifier(modifier: Option<u64>) -> String {
    modifier.map_or_else(
        || "none".to_string(),
        |modifier| format!("0x{modifier:016x}"),
    )
}

#[cfg(test)]
mod tests;
