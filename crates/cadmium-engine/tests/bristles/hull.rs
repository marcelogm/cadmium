use cadmium_engine::engine::BristleSnapshot;

pub fn touching_segment_ends(snapshot: &BristleSnapshot) -> Vec<[f32; 2]> {
    snapshot
        .chains_px
        .iter()
        .zip(&snapshot.radii_px)
        .flat_map(|(chain, &radius)| {
            chain
                .windows(2)
                .filter(move |pair| pair.iter().any(|vertex| vertex[2] <= radius))
                .flat_map(|pair| pair.iter().map(|vertex| [vertex[0], vertex[1]]))
        })
        .collect()
}

fn cross(o: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}

fn add_to_half_hull(hull: &mut Vec<[f32; 2]>, start: usize, point: [f32; 2]) {
    while hull.len() >= start + 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], point) <= 0.0
    {
        hull.pop();
    }
    hull.push(point);
}

pub fn convex_hull(mut points: Vec<[f32; 2]>) -> Vec<[f32; 2]> {
    points.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    let mut hull: Vec<[f32; 2]> = Vec::new();
    for ordered in [points.clone(), points.iter().rev().copied().collect()] {
        let start = hull.len();
        for point in ordered {
            add_to_half_hull(&mut hull, start, point);
        }
        hull.pop();
    }
    hull
}

fn distance_to_segment(point: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let along = [b[0] - a[0], b[1] - a[1]];
    let offset = [point[0] - a[0], point[1] - a[1]];
    let span = along[0] * along[0] + along[1] * along[1];
    let t = if span > 0.0 {
        ((offset[0] * along[0] + offset[1] * along[1]) / span).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (offset[0] - t * along[0]).hypot(offset[1] - t * along[1])
}

pub fn distance_to_hull(point: [f32; 2], hull: &[[f32; 2]]) -> f32 {
    let inside = hull.len() >= 3
        && (0..hull.len()).all(|i| cross(hull[i], hull[(i + 1) % hull.len()], point) >= 0.0);
    if inside {
        return 0.0;
    }
    (0..hull.len())
        .map(|i| distance_to_segment(point, hull[i], hull[(i + 1) % hull.len()]))
        .fold(f32::INFINITY, f32::min)
}
