use ash::vk;
use gtk::gdk;
use gtk::prelude::*;

use super::{DmabufError, IMAGE_USAGE, ModifierPolicy, PRESENT_FOURCC, PRESENT_VK_FORMAT};

pub(super) const DRM_FORMAT_MOD_LINEAR: u64 = 0;
const DRM_FORMAT_MOD_VENDOR_SHIFT: u64 = 56;
const DRM_FORMAT_MOD_VENDOR_AMD: u64 = 0x02;
const AMD_FMT_MOD_DCC_BIT: u64 = 1 << 13;

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

pub(super) fn shared_modifiers(
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

pub(super) fn candidate_modifiers(
    vulkan_modifiers: &[u64],
    display: &gdk::Display,
    policy: ModifierPolicy,
) -> Result<Vec<u64>, DmabufError> {
    let gtk_formats = display.dmabuf_formats();
    let candidates = shared_modifiers(
        vulkan_modifiers,
        |modifier| gtk_formats.contains(PRESENT_FOURCC, modifier),
        policy,
    );
    if candidates.is_empty() {
        return Err(DmabufError::Unsupported(format!(
            "no AB24 modifier shared by Vulkan {} and GTK under {policy:?}",
            format_modifiers(vulkan_modifiers)
        )));
    }
    Ok(candidates)
}

pub(super) fn vulkan_exportable_storage_modifiers(
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

pub(super) fn single_plane_storage_modifiers(
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
