// The floor a hair rests on, from a relief `surface_px(at)` the including shader defines.
// A hair is a cylinder: it rests on the highest relief within its radius of its axis, and
// along the path its vertex took since the floor was last read, so it cannot pass through
// a wall narrower than its step. Phase 4 of #62 replaces the strand radius with the
// tapered radius r(s) per vertex.

/// Points on the ring of the disc the floor is the highest relief of.
const FLOOR_RING_POINTS: u32 = 6u;
/// sin 60°: the hexagon's ring points sit at (±1, 0) and (±½, ±√3/2) radii.
const HEXAGON_SINE: f32 = 0.866025403784439;
/// Most points read along a vertex's path besides its end.
const MAX_SWEEP_SAMPLES: u32 = 3u;
/// Half the span of the central differences that measure the floor's slope, px.
const SLOPE_PROBE_PX: f32 = 1.0;

/// The highest relief within `radius_px` of `at`: the centre and a ring at the radius.
fn disc_floor_px(at: vec2<f32>, radius_px: f32) -> f32 {
    let ring = array<vec2<f32>, FLOOR_RING_POINTS>(
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.5, HEXAGON_SINE),
        vec2<f32>(-0.5, HEXAGON_SINE),
        vec2<f32>(-1.0, 0.0),
        vec2<f32>(-0.5, -HEXAGON_SINE),
        vec2<f32>(0.5, -HEXAGON_SINE),
    );
    var highest = surface_px(at);
    for (var point = 0u; point < FLOOR_RING_POINTS; point++) {
        highest = max(highest, surface_px(at + radius_px * ring[point]));
    }
    return highest;
}

/// The highest disc floor along the straight path from `start` to `end`, read at most a
/// radius apart.
fn swept_floor_px(start: vec2<f32>, end: vec2<f32>, radius_px: f32) -> f32 {
    return swept_from_px(start, end, radius_px, disc_floor_px(end, radius_px));
}

/// `swept_floor_px` given the disc floor at `end`.
fn swept_from_px(start: vec2<f32>, end: vec2<f32>, radius_px: f32, end_floor_px: f32) -> f32 {
    var highest = end_floor_px;
    let spans = ceil(distance(start, end) / max(radius_px, MIN_SEGMENT_SPAN_PX));
    let samples = u32(min(spans, f32(MAX_SWEEP_SAMPLES + 1u)));
    for (var sample = 1u; sample < samples; sample++) {
        let along = mix(start, end, f32(sample) / f32(samples));
        highest = max(highest, disc_floor_px(along, radius_px));
    }
    return highest;
}

/// The disc floor's slope (∂f/∂x, ∂f/∂y) at `at`, by central differences.
fn floor_slope(at: vec2<f32>, radius_px: f32) -> vec2<f32> {
    let dx = vec2<f32>(SLOPE_PROBE_PX, 0.0);
    let dy = vec2<f32>(0.0, SLOPE_PROBE_PX);
    return vec2<f32>(
        disc_floor_px(at + dx, radius_px) - disc_floor_px(at - dx, radius_px),
        disc_floor_px(at + dy, radius_px) - disc_floor_px(at - dy, radius_px),
    ) / (2.0 * SLOPE_PROBE_PX);
}

/// The floor under each vertex (1..) of `chain` that moved there from `previous`; the root
/// is held by the ferrule and needs none.
fn floors_under(previous: Chain, chain: Chain, radius_px: f32) -> Floors {
    var floors: Floors;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let at = chain[vertex].xy;
        floors.heights_px[vertex] = swept_floor_px(previous[vertex].xy, at, radius_px);
        floors.slopes[vertex] = floor_slope(at, radius_px);
    }
    return floors;
}
