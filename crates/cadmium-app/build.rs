fn main() {
    glib_build_tools::compile_resources(
        &["data/resources", "data"],
        "data/resources/resources.gresource.xml",
        "cadmium.gresource",
    );
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_windows_icon();
    }
}

#[cfg(windows)]
fn embed_windows_icon() {
    embed_resource::compile("data/windows/cadmium.rc", embed_resource::NONE)
        .manifest_required()
        .unwrap();
}

#[cfg(not(windows))]
fn embed_windows_icon() {}
