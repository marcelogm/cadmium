use super::*;

const VULKAN_RENDERER: &str = "GskVulkanRenderer";
const GL_RENDERER: &str = "GskGLRenderer";

#[test]
fn present_mode_names_follow_the_environment_contract() {
    for name in ["readback", "dmabuf", "dmabuf-nodcc", "linear"] {
        assert_eq!(
            PresentMode::from_name(name).map(PresentMode::name),
            Some(name)
        );
    }
    assert_eq!(
        PresentMode::from_name("linear"),
        Some(PresentMode::Dmabuf(ModifierPolicy::LinearOnly))
    );
    assert_eq!(PresentMode::from_name("fast"), None);
}

#[test]
fn vulkan_gtk_on_an_exporting_gpu_presents_by_dmabuf() {
    assert_eq!(
        choose_mode(None, true, VULKAN_RENDERER),
        PresentMode::Dmabuf(ModifierPolicy::Any)
    );
}

#[test]
fn gl_gtk_or_a_gpu_without_export_presents_by_readback() {
    assert_eq!(choose_mode(None, true, GL_RENDERER), PresentMode::Readback);
    assert_eq!(
        choose_mode(None, false, VULKAN_RENDERER),
        PresentMode::Readback
    );
}

#[test]
fn the_environment_overrides_the_automatic_choice() {
    assert_eq!(
        choose_mode(Some(PresentMode::Readback), true, VULKAN_RENDERER),
        PresentMode::Readback
    );
    assert_eq!(
        choose_mode(
            Some(PresentMode::Dmabuf(ModifierPolicy::LinearOnly)),
            true,
            GL_RENDERER
        ),
        PresentMode::Dmabuf(ModifierPolicy::LinearOnly)
    );
}

#[test]
fn fallback_reason_names_the_gl_renderer() {
    assert!(mode_reason(false, true, GL_RENDERER).contains(GL_RENDERER));
    assert!(mode_reason(true, true, VULKAN_RENDERER).contains(PRESENT_MODE_VARIABLE));
}

#[test]
fn fallback_log_names_the_last_modifier_or_none() {
    assert_eq!(describe_modifier(Some(0x20)), "0x0000000000000020");
    assert_eq!(describe_modifier(None), "none");
}
