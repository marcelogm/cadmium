pub(crate) fn region_rect(origin: [i32; 2], size: [u32; 2]) -> [f32; 4] {
    [
        origin[0] as f32,
        origin[1] as f32,
        (origin[0] + size[0] as i32 - 1) as f32,
        (origin[1] + size[1] as i32 - 1) as f32,
    ]
}

pub(crate) fn clamped_region(
    first: [i32; 2],
    last: [i32; 2],
    apron: i32,
    limits: [i32; 2],
) -> Option<([i32; 2], [u32; 2])> {
    let low = [0, 1].map(|axis| first[axis].saturating_sub(apron).max(0));
    let high = [0, 1].map(|axis| last[axis].saturating_add(apron).min(limits[axis] - 1));
    if (0..2).any(|axis| high[axis] < low[axis]) {
        return None;
    }
    Some((low, [0, 1].map(|axis| (high[axis] - low[axis] + 1) as u32)))
}
