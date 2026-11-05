struct Contact {
    coverage: f32,
    along: u32,
    across: u32,
    bristle: u32,
};

const ACROSS_SHIFT: u32 = CONTACT_BRISTLE_BITS;
const ALONG_SHIFT: u32 = ACROSS_SHIFT + CONTACT_BIN_BITS;
const COVERAGE_SHIFT: u32 = ALONG_SHIFT + CONTACT_BIN_BITS;
const BIN_MASK: u32 = (1u << CONTACT_BIN_BITS) - 1u;
const BRISTLE_MASK: u32 = (1u << CONTACT_BRISTLE_BITS) - 1u;

fn contact_level(share: f32) -> u32 {
    return u32(round(clamp(share, 0.0, 1.0) * f32(CONTACT_LEVELS)));
}

fn grid_bin(share: f32) -> u32 {
    return min(u32(clamp(share, 0.0, 1.0) * f32(CELL_GRID_SIDE)), CELL_GRID_SIDE - 1u);
}

fn packed_contact(coverage: f32, place: vec2<f32>, bristle: u32) -> u32 {
    let coverage_bits = contact_level(coverage) << COVERAGE_SHIFT;
    let place_bits = (grid_bin(place.x) << ALONG_SHIFT) | (grid_bin(place.y) << ACROSS_SHIFT);
    return coverage_bits | place_bits | (bristle & BRISTLE_MASK);
}

fn coverage_level(word: u32) -> u32 {
    return word >> COVERAGE_SHIFT;
}

fn with_coverage_level(word: u32, level: u32) -> u32 {
    let below_coverage = (1u << COVERAGE_SHIFT) - 1u;
    return (word & below_coverage) | (level << COVERAGE_SHIFT);
}

fn unpacked_contact(word: u32) -> Contact {
    let coverage = f32(coverage_level(word)) / f32(CONTACT_LEVELS);
    let along = (word >> ALONG_SHIFT) & BIN_MASK;
    let across = (word >> ACROSS_SHIFT) & BIN_MASK;
    return Contact(coverage, along, across, word & BRISTLE_MASK);
}

fn inside_dab_region(pixel: vec2<i32>) -> bool {
    let local = pixel - dab.canvas_origin;
    return all(local >= vec2<i32>(0)) && all(local < vec2<i32>(dab.canvas_size));
}

fn mask_slot(pixel: vec2<i32>) -> u32 {
    let local = vec2<u32>(pixel - dab.canvas_origin);
    return local.y * dab.canvas_size.x + local.x;
}

fn contact_at(pixel: vec2<i32>) -> Contact {
    if (!inside_dab_region(pixel)) {
        return Contact(0.0, 0u, 0u, 0u);
    }
    return unpacked_contact(contact_mask_word(mask_slot(pixel)));
}

fn across_stroke_direction() -> vec2<f32> {
    return vec2<f32>(-sin(dab.direction), cos(dab.direction));
}

fn across_stroke_px(point: vec2<f32>) -> f32 {
    return dot(point - dab.center, across_stroke_direction());
}

fn along_stroke_px(point: vec2<f32>) -> f32 {
    return dot(point - dab.center, vec2<f32>(cos(dab.direction), sin(dab.direction)));
}
