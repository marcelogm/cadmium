use cadmium_engine::gpu::{GpuContext, GpuLost, TexelRows, read_mapped, staging_buffer};

const STAGING_BYTES: u64 = 16;

#[test]
fn creates_vulkan_device_with_storage_read_write_on_rgba16float() {
    let gpu = GpuContext::new().expect("gpu");
    assert_eq!(gpu.adapter.get_info().backend, wgpu::Backend::Vulkan);
    assert!(
        gpu.device
            .features()
            .contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES)
    );
    let flags = gpu
        .adapter
        .get_texture_format_features(wgpu::TextureFormat::Rgba16Float)
        .flags;
    assert!(flags.contains(wgpu::TextureFormatFeatureFlags::STORAGE_READ_WRITE));
}

#[test]
fn requests_adapter_limits_instead_of_defaults() {
    let gpu = GpuContext::new().expect("gpu");
    assert!(gpu.device.limits().max_storage_textures_per_shader_stage >= 8);
    assert!(gpu.device.limits().max_storage_buffer_binding_size >= 256 << 20);
}

#[test]
fn reports_dmabuf_export_on_radv() {
    let gpu = GpuContext::new().expect("gpu");
    assert!(gpu.supports_dmabuf_export);
}

#[test]
fn a_mapped_read_returns_what_the_gpu_copied() {
    let gpu = GpuContext::new().expect("gpu");
    let staging = staging_buffer(&gpu, "test_staging", STAGING_BYTES);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.clear_buffer(&staging, 0, None);
    gpu.queue.submit([encoder.finish()]);
    let read = read_mapped(&gpu, &staging, <[u8]>::to_vec).unwrap();
    assert_eq!(read, vec![0; STAGING_BYTES as usize]);
    assert!(!gpu.is_lost());
}

#[test]
fn a_lost_device_fails_the_read_instead_of_panicking() {
    let gpu = GpuContext::new().expect("gpu");
    let staging = staging_buffer(&gpu, "test_staging", STAGING_BYTES);
    gpu.device.destroy();
    assert_eq!(read_mapped(&gpu, &staging, <[u8]>::len), Err(GpuLost));
    assert!(gpu.is_lost());
}

#[test]
fn unpadding_keeps_only_the_texel_bytes_of_each_row() {
    let rows = TexelRows::new(8, 2);
    assert_eq!(rows.padded_row_bytes, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let mut padded = vec![0u8; rows.padded_size() as usize];
    padded[..8].copy_from_slice(&[1; 8]);
    let second = rows.padded_row_bytes as usize;
    padded[second..second + 8].copy_from_slice(&[2; 8]);
    assert_eq!(rows.unpad(&padded), [[1u8; 8], [2u8; 8]].concat());
}
