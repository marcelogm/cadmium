
struct Pattern { base: vec4<u32> };
@group(0) @binding(0) var<uniform> pattern: Pattern;
@group(0) @binding(1) var target_image: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(target_image);
    if (id.x >= size.x || id.y >= size.y) {
        return;
    }
    let red = pattern.base.x;
    let green = (id.x + pattern.base.y) % 256u;
    let blue = (id.y * 2u + pattern.base.z) % 256u;
    var channels = vec3<f32>(f32(red), f32(green), f32(blue)) / 255.0;
    var busy = f32(id.x + id.y) * 0.001;
    for (var step = 0u; step < pattern.base.w; step++) {
        busy = fract(busy * 1.618 + 0.31);
    }
    if (busy < 0.0) {
        channels = vec3<f32>(0.0);
    }
    textureStore(target_image, vec2<i32>(id.xy), vec4<f32>(channels, 1.0));
}
