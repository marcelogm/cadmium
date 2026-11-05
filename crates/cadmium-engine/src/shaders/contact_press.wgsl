const PRESS_SLOPE_SCALE: f32 = 4.0;

fn contact_extent_px() -> vec2<f32> {
    return vec2<f32>(f32(contact_extent[CONTACT_EXTENT_LOW]), f32(contact_extent[CONTACT_EXTENT_HIGH])) / CONTACT_EXTENT_SCALE;
}

fn contact_along_extent_px() -> vec2<f32> {
    return vec2<f32>(f32(contact_extent[CONTACT_ALONG_LOW]), f32(contact_extent[CONTACT_ALONG_HIGH])) / CONTACT_EXTENT_SCALE;
}

fn contact_press_gradient(pixel: vec2<i32>) -> vec2<f32> {
    let extent = contact_extent_px();
    let half_width = 0.5 * (extent.y - extent.x);
    if (half_width <= 0.0) {
        return vec2<f32>(0.0);
    }
    let middle = 0.5 * (extent.x + extent.y);
    let across = (across_stroke_px(vec2<f32>(pixel) + vec2<f32>(0.5)) - middle) / half_width;
    let inside = 1.0 - across * across;
    if (inside <= 0.0) {
        return vec2<f32>(0.0);
    }
    let pressed = contact_at(pixel).coverage * dab.ferrule_press_share * dab.pressure;
    return -PRESS_SLOPE_SCALE * pressed * inside * across * across_stroke_direction();
}
