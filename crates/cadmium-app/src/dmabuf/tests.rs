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
