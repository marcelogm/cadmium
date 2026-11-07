alias Wetness = array<f32, BRISTLES_PER_THREAD>;
const MOVING_VERTICES: u32 = BRISTLE_VERTICES - 1u;
const CLUMP_ITEMS_PER_THREAD: u32 = BRISTLES_PER_THREAD * MOVING_VERTICES;
alias MovedVertices = array<vec4<f32>, CLUMP_ITEMS_PER_THREAD>;

const OVERLAP_SHARE_PER_SIDE: f32 = 0.5;

var<workgroup> centroids: array<vec3<f32>, CLUSTER_CENTROIDS>;
var<workgroup> pulls: array<f32, MAX_BRISTLES>;

fn cluster_start(cluster: u32) -> u32 {
    return clusters[CLUSTER_SPAN_WORDS * cluster];
}

fn cluster_size(cluster: u32) -> u32 {
    return clusters[CLUSTER_SPAN_WORDS * cluster + 1u];
}

fn cluster_member(index: u32) -> u32 {
    return clusters[CLUSTER_SPAN_WORDS * MAX_CLUSTERS + index];
}

fn lane_volume_um(rest: BristleRest, sample: u32) -> f32 {
    let share = f32(sample) / f32(LANE_SAMPLES - 1u);
    let last = vec2<f32>(f32(TIP_SIZE - 1u));
    let texel = clamp(mix(rest.lane_start, rest.lane_end, share), vec2<f32>(0.0), last);
    return tip[tip_index(vec2<u32>(texel)) + VOLUME_CHANNEL];
}

fn wetness_of(rest: BristleRest) -> f32 {
    var volume_um = 0.0;
    for (var sample = 0u; sample < LANE_SAMPLES; sample++) {
        volume_um += lane_volume_um(rest, sample);
    }
    if (dab.loaded_thickness_um <= 0.0) {
        return 0.0;
    }
    return clamp(volume_um / (f32(LANE_SAMPLES) * dab.loaded_thickness_um), 0.0, 1.0);
}

fn owned_wetness(thread: u32) -> Wetness {
    var wetness: Wetness;
    for (var share = 0u; share < BRISTLES_PER_THREAD; share++) {
        let bristle = owned_bristle(thread, share);
        let rest = rests[min(bristle, MAX_BRISTLES - 1u)];
        wetness[share] = select(0.0, wetness_of(rest), bristle < dab.bristle_total);
    }
    return wetness;
}

fn gather_centroid(slot: u32) {
    let cluster = slot / BRISTLE_VERTICES;
    let vertex = slot % BRISTLE_VERTICES;
    let start = cluster_start(cluster);
    let size = cluster_size(cluster);
    var sum = vec3<f32>(0.0);
    for (var index = 0u; index < size; index++) {
        sum += states[position_slot(cluster_member(start + index), vertex)].xyz;
    }
    centroids[slot] = sum / f32(max(size, 1u));
}

fn gather_centroids(thread: u32) {
    for (var slot = thread; slot < CLUSTER_CENTROIDS; slot += BRISTLE_WORKGROUP_SIZE) {
        gather_centroid(slot);
    }
}

fn apart(offset: vec3<f32>, roots_apart: vec3<f32>) -> vec3<f32> {
    let gap_px = length(offset);
    let fallback = heading_of(roots_apart, vec3<f32>(0.0));
    let direction = offset / max(gap_px, MIN_SEGMENT_SPAN_PX);
    return select(fallback, direction, gap_px > MIN_SEGMENT_SPAN_PX);
}

fn spaced(at: vec3<f32>, bristle: u32, vertex: u32, pull: f32) -> vec3<f32> {
    let rest = rests[bristle];
    let root = states[position_slot(bristle, 0u)].xyz;
    let centroid = centroids[rest.cluster * BRISTLE_VERTICES + vertex];
    let start = cluster_start(rest.cluster);
    var pushed = at;
    for (var index = 0u; index < cluster_size(rest.cluster); index++) {
        let other = cluster_member(start + index);
        let spacing_px = min(2.0 * rest.radius_px, distance(rest.root_px, rests[other].root_px));
        let offset = at - mix(states[position_slot(other, vertex)].xyz, centroid, pull);
        let gap_px = length(offset);
        let crowded = other != bristle && gap_px < spacing_px;
        let overlap_px = (spacing_px - gap_px) * OVERLAP_SHARE_PER_SIDE;
        let push = apart(offset, root - states[position_slot(other, 0u)].xyz) * overlap_px;
        pushed += select(vec3<f32>(0.0), push, crowded);
    }
    return pushed;
}

fn floor_after_move(held: vec4<f32>, moved_to: vec2<f32>) -> f32 {
    if distance(held.xy, moved_to) <= MIN_SEGMENT_SPAN_PX {
        return held.w;
    }
    return surface_px(moved_to);
}

fn clumped_vertex(bristle: u32, vertex: u32) -> vec4<f32> {
    let rest = rests[bristle];
    let pull = pulls[bristle];
    let held = states[position_slot(bristle, vertex)];
    let centroid = centroids[rest.cluster * BRISTLE_VERTICES + vertex];
    let pushed = spaced(mix(held.xyz, centroid, pull), bristle, vertex, pull);
    let floor_px = floor_after_move(held, pushed.xy);
    return vec4<f32>(pushed.xy, max(pushed.z, floor_px), floor_px);
}

fn share_pulls(thread: u32, wetness: Wetness) {
    for (var share = 0u; share < BRISTLES_PER_THREAD; share++) {
        let bristle = owned_bristle(thread, share);
        if bristle < dab.bristle_total {
            pulls[bristle] = tuning.clumping * wetness[share];
        }
    }
    workgroupBarrier();
}

fn clumped_bristle(item: u32) -> u32 {
    return item / MOVING_VERTICES;
}

fn clumped_vertex_index(item: u32) -> u32 {
    return 1u + item % MOVING_VERTICES;
}

fn clump_owned(thread: u32) {
    gather_centroids(thread);
    workgroupBarrier();
    var moved: MovedVertices;
    for (var share = 0u; share < CLUMP_ITEMS_PER_THREAD; share++) {
        let item = owned_bristle(thread, share);
        if clumped_bristle(item) >= dab.bristle_total {
            break;
        }
        moved[share] = clumped_vertex(clumped_bristle(item), clumped_vertex_index(item));
    }
    storageBarrier();
    for (var share = 0u; share < CLUMP_ITEMS_PER_THREAD; share++) {
        let item = owned_bristle(thread, share);
        if clumped_bristle(item) >= dab.bristle_total {
            break;
        }
        states[position_slot(clumped_bristle(item), clumped_vertex_index(item))] = moved[share];
    }
    storageBarrier();
}
