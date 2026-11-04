use cadmium_engine::gpu::GpuContext;

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
