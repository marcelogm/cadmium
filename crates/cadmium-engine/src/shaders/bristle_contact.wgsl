struct Span {
    start: vec2<f32>,
    end: vec2<f32>,
    arcs: vec2<f32>,
    touching: bool,
};

struct Sweep {
    before: Span,
    now: Span,
    radius_px: f32,
    first_touching_arc: f32,
    draw_log: f32,
};

struct SweptContact {
    coverage: f32,
    place: vec2<f32>,
};

const MIN_COVERAGE: f32 = 1e-6;
const DRAW_DISCARDED_BITS: u32 = 8u;
const COVERAGE_FEATHER_PX: f32 = 0.5;
const MIN_SPAN_PX2: f32 = 1e-8;
const MIN_TOUCHING_ARC: f32 = 1e-3;
const TOUCHING: f32 = 1.0;
const MAX_SWEEP_SAMPLES: u32 = 8u;
const MAX_SWEEP_SAMPLE_SPACING_PX: f32 = 1.0;
const NO_EXTENT_Q: i32 = 2147483647;

@group(0) @binding(0) var<uniform> dab: Dab;
@group(0) @binding(1) var<storage, read> rests: array<BristleRest>;
@group(0) @binding(2) var<storage, read> states: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> contact_mask: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read_write> texel_load: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read_write> swept: array<vec4<f32>>;
@group(0) @binding(6) var<storage, read> cell_grids: array<u32>;
@group(0) @binding(7) var<storage, read_write> contact_extent: array<atomic<i32>, CONTACT_EXTENT_WORDS>;
@group(0) @binding(8) var<storage, read_write> summed_coverage: array<atomic<u32>>;

fn contact_mask_word(index: u32) -> u32 {
    return atomicLoad(&contact_mask[index]);
}

fn clearance_px(vertex: vec4<f32>) -> f32 {
    return vertex.z - vertex.w;
}

fn touching_span(bristle: u32, segment: u32, band_px: f32) -> Span {
    let upper = states[position_slot(bristle, segment)];
    let lower = states[position_slot(bristle, segment + 1u)];
    let upper_clearance = clearance_px(upper);
    let lower_clearance = clearance_px(lower);
    if (upper_clearance > band_px && lower_clearance > band_px) {
        return Span(upper.xy, lower.xy, vec2<f32>(0.0), false);
    }
    var cut = vec2<f32>(0.0, 1.0);
    if (upper_clearance > band_px) {
        cut.x = (upper_clearance - band_px) / (upper_clearance - lower_clearance);
    }
    if (lower_clearance > band_px) {
        cut.y = (band_px - upper_clearance) / (lower_clearance - upper_clearance);
    }
    let arcs = (vec2<f32>(f32(segment)) + cut) / f32(BRISTLE_SEGMENTS);
    return Span(mix(upper.xy, lower.xy, cut.x), mix(upper.xy, lower.xy, cut.y), arcs, true);
}

fn swept_slot(bristle: u32, segment: u32) -> u32 {
    return (bristle * BRISTLE_SEGMENTS + segment) * SWEPT_VECTORS;
}

fn remembered_span(slot: u32) -> Span {
    let ends = swept[slot];
    let rest = swept[slot + 1u];
    return Span(ends.xy, ends.zw, rest.xy, rest.z == TOUCHING);
}

fn remember_span(slot: u32, span: Span) {
    swept[slot] = vec4<f32>(span.start, span.end);
    swept[slot + 1u] = vec4<f32>(span.arcs, select(0.0, TOUCHING, span.touching), 0.0);
}

fn continues_previous_dab() -> bool {
    let moved = dab.sweep_x != 0.0 || dab.sweep_y != 0.0;
    return moved && dab.bristle_reset == 0u;
}

fn edge_side(point: vec2<f32>, start: vec2<f32>, end: vec2<f32>) -> f32 {
    let along = end - start;
    let offset = point - start;
    return along.x * offset.y - along.y * offset.x;
}

fn sweep_samples(sweep: Sweep) -> u32 {
    let travel = max(distance(sweep.before.start, sweep.now.start), distance(sweep.before.end, sweep.now.end));
    return clamp(u32(ceil(travel / MAX_SWEEP_SAMPLE_SPACING_PX)), 1u, MAX_SWEEP_SAMPLES);
}

fn span_at(sweep: Sweep, time: f32) -> Span {
    let start = mix(sweep.before.start, sweep.now.start, time);
    let end = mix(sweep.before.end, sweep.now.end, time);
    return Span(start, end, mix(sweep.before.arcs, sweep.now.arcs, time), true);
}

fn span_share(point: vec2<f32>, span: Span) -> f32 {
    let along = span.end - span.start;
    return clamp(dot(point - span.start, along) / max(dot(along, along), MIN_SPAN_PX2), 0.0, 1.0);
}

fn across_under(point: vec2<f32>, span: Span, reach_px: f32) -> f32 {
    let along = span.end - span.start;
    let side = edge_side(point, span.start, span.end) / sqrt(max(dot(along, along), MIN_SPAN_PX2));
    return 0.5 + 0.5 * clamp(side / reach_px, -1.0, 1.0);
}

fn swept_contact(point: vec2<f32>, sweep: Sweep, reach_px: f32) -> SweptContact {
    let samples = sweep_samples(sweep);
    var exposed = 0.0;
    var exposed_time = 0.0;
    var exposed_arc = 0.0;
    for (var sample = 0u; sample < samples; sample++) {
        let time = (f32(sample) + 0.5) / f32(samples);
        let span = span_at(sweep, time);
        let share = span_share(point, span);
        let closest = mix(span.start, span.end, share);
        let cover = clamp(reach_px - distance(point, closest), 0.0, 1.0);
        exposed += cover;
        exposed_time += cover * time;
        exposed_arc += cover * mix(span.arcs.x, span.arcs.y, share);
    }
    let weight = max(exposed, MIN_COVERAGE);
    let arc_share = share_of_touching_arc(exposed_arc / weight, sweep.first_touching_arc);
    let across = across_under(point, span_at(sweep, exposed_time / weight), reach_px);
    return SweptContact(exposed / f32(samples), vec2<f32>(arc_share, across));
}

fn first_touching_arc(bristle: u32, band_px: f32) -> f32 {
    for (var segment = 0u; segment < BRISTLE_SEGMENTS; segment++) {
        let span = touching_span(bristle, segment, band_px);
        if (span.touching) {
            return span.arcs.x;
        }
    }
    return 1.0;
}

fn share_of_touching_arc(arc: f32, first: f32) -> f32 {
    return (arc - first) / max(1.0 - first, MIN_TOUCHING_ARC);
}

fn mixed_bits(seed: u32) -> u32 {
    var bits = seed;
    bits ^= bits >> 16u;
    bits *= 0x7feb352du;
    bits ^= bits >> 15u;
    bits *= 0x846ca68bu;
    bits ^= bits >> 16u;
    return bits;
}

fn hair_draw_log(bristle: u32) -> f32 {
    let dab_seed = mixed_bits(bitcast<u32>(dab.center.x) ^ mixed_bits(bitcast<u32>(dab.center.y)));
    let draw_bits = mixed_bits(bristle ^ dab_seed) >> DRAW_DISCARDED_BITS;
    return log((f32(draw_bits) + 0.5) / f32(1u << (32u - DRAW_DISCARDED_BITS)));
}

fn draw_priority(draw_log: f32, coverage: f32) -> f32 {
    let share = exp(draw_log / max(coverage, MIN_COVERAGE));
    return max(share, 1.0 / f32(CONTACT_LEVELS));
}

fn mark_pixel(pixel: vec2<i32>, sweep: Sweep, bristle: u32) {
    let point = vec2<f32>(pixel) + vec2<f32>(0.5);
    let reach_px = sweep.radius_px + COVERAGE_FEATHER_PX;
    let contact = swept_contact(point, sweep, reach_px);
    let level = contact_level(contact.coverage);
    let priority = draw_priority(sweep.draw_log, contact.coverage);
    let word = packed_contact(priority, contact.place, bristle);
    if (level > 0u) {
        atomicMax(&contact_mask[mask_slot(pixel)], word);
        atomicAdd(&summed_coverage[mask_slot(pixel)], level);
    }
}

fn swept_bounds(sweep: Sweep) -> vec4<i32> {
    let low = min(min(sweep.now.start, sweep.now.end), min(sweep.before.start, sweep.before.end));
    let high = max(max(sweep.now.start, sweep.now.end), max(sweep.before.start, sweep.before.end));
    let reach = vec2<f32>(sweep.radius_px + COVERAGE_FEATHER_PX + 1.0);
    let first = max(vec2<i32>(floor(low - reach)), dab.canvas_origin);
    let last = min(vec2<i32>(floor(high + reach)), dab.canvas_origin + vec2<i32>(dab.canvas_size) - vec2<i32>(1));
    return vec4<i32>(first, last);
}

fn across_reach(span: Span) -> vec2<f32> {
    let start = across_stroke_px(span.start);
    let end = across_stroke_px(span.end);
    return vec2<f32>(min(start, end), max(start, end));
}

fn along_reach(span: Span) -> vec2<f32> {
    let start = along_stroke_px(span.start);
    let end = along_stroke_px(span.end);
    return vec2<f32>(min(start, end), max(start, end));
}

fn widen_words(low_word: u32, high_word: u32, before: vec2<f32>, now: vec2<f32>, reach_px: f32) {
    let low = min(before.x, now.x) - reach_px;
    let high = max(before.y, now.y) + reach_px;
    atomicMin(&contact_extent[low_word], i32(floor(low * CONTACT_EXTENT_SCALE)));
    atomicMax(&contact_extent[high_word], i32(ceil(high * CONTACT_EXTENT_SCALE)));
}

fn widen_contact_extent(sweep: Sweep) {
    let reach_px = sweep.radius_px + COVERAGE_FEATHER_PX;
    let across_before = across_reach(sweep.before);
    let across_now = across_reach(sweep.now);
    widen_words(CONTACT_EXTENT_LOW, CONTACT_EXTENT_HIGH, across_before, across_now, reach_px);
    let along_before = along_reach(sweep.before);
    let along_now = along_reach(sweep.now);
    widen_words(CONTACT_ALONG_LOW, CONTACT_ALONG_HIGH, along_before, along_now, reach_px);
}

fn widen_contact_box(bounds: vec4<i32>) {
    atomicMin(&contact_extent[CONTACT_BOX_FIRST], bounds.x);
    atomicMin(&contact_extent[CONTACT_BOX_FIRST + 1u], bounds.y);
    atomicMax(&contact_extent[CONTACT_BOX_LAST], bounds.z);
    atomicMax(&contact_extent[CONTACT_BOX_LAST + 1u], bounds.w);
}

const ROW_SLACK_PX: f32 = 1.0;

struct RowSpan {
    first: f32,
    last: f32,
};

fn hull_corner(sweep: Sweep, corner: u32) -> vec2<f32> {
    switch corner {
        case 0u: { return sweep.before.start; }
        case 1u: { return sweep.before.end; }
        case 2u: { return sweep.now.start; }
        default: { return sweep.now.end; }
    }
}

fn widen_to(span: RowSpan, x: f32) -> RowSpan {
    return RowSpan(min(span.first, x), max(span.last, x));
}

fn crossing(span: RowSpan, start: vec2<f32>, end: vec2<f32>, line_y: f32) -> RowSpan {
    let low = min(start.y, end.y);
    let high = max(start.y, end.y);
    if (line_y < low || line_y > high || high <= low) {
        return span;
    }
    return widen_to(span, mix(start.x, end.x, (line_y - start.y) / (end.y - start.y)));
}

fn hull_row_span(sweep: Sweep, centre_y: f32, reach_px: f32) -> RowSpan {
    let slab = vec2<f32>(centre_y - reach_px, centre_y + reach_px);
    var span = RowSpan(3.0e38, -3.0e38);
    for (var corner = 0u; corner < 4u; corner++) {
        let point = hull_corner(sweep, corner);
        if (point.y >= slab.x && point.y <= slab.y) {
            span = widen_to(span, point.x);
        }
        for (var other = corner + 1u; other < 4u; other++) {
            let end = hull_corner(sweep, other);
            span = crossing(span, point, end, slab.x);
            span = crossing(span, point, end, slab.y);
        }
    }
    return RowSpan(span.first - reach_px - ROW_SLACK_PX, span.last + reach_px + ROW_SLACK_PX);
}

fn rasterize_row(sweep: Sweep, bristle: u32, bounds: vec4<i32>, row: i32) {
    let reach_px = sweep.radius_px + COVERAGE_FEATHER_PX;
    let span = hull_row_span(sweep, f32(row) + 0.5, reach_px);
    if (span.first > span.last) {
        return;
    }
    let first = max(bounds.x, i32(floor(span.first)));
    let last = min(bounds.z, i32(ceil(span.last)));
    for (var x = first; x <= last; x++) {
        mark_pixel(vec2<i32>(x, row), sweep, bristle);
    }
}

fn rasterize_sweep(sweep: Sweep, bristle: u32, thread: u32) {
    if (thread == 0u) {
        widen_contact_extent(sweep);
    }
    let bounds = swept_bounds(sweep);
    if (bounds.z < bounds.x || bounds.w < bounds.y) {
        return;
    }
    if (thread == 0u) {
        widen_contact_box(bounds);
    }
    if (bounds.w - bounds.y + 1 < i32(CONTACT_WORKGROUP_SIZE)) {
        rasterize_box(sweep, bristle, bounds, thread);
        return;
    }
    for (var row = bounds.y + i32(thread); row <= bounds.w; row += i32(CONTACT_WORKGROUP_SIZE)) {
        rasterize_row(sweep, bristle, bounds, row);
    }
}

fn rasterize_box(sweep: Sweep, bristle: u32, bounds: vec4<i32>, thread: u32) {
    let width = u32(bounds.z - bounds.x + 1);
    let pixels = width * u32(bounds.w - bounds.y + 1);
    for (var index = thread; index < pixels; index += CONTACT_WORKGROUP_SIZE) {
        let offset = vec2<i32>(vec2<u32>(index % width, index / width));
        mark_pixel(bounds.xy + offset, sweep, bristle);
    }
}

@compute @workgroup_size(8, 8)
fn clear_mask(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= dab.canvas_size.x || id.y >= dab.canvas_size.y) {
        return;
    }
    atomicStore(&contact_mask[id.y * dab.canvas_size.x + id.x], 0u);
    atomicStore(&summed_coverage[id.y * dab.canvas_size.x + id.x], 0u);
    if (all(id.xy == vec2<u32>(0u))) {
        atomicStore(&contact_extent[CONTACT_EXTENT_LOW], NO_EXTENT_Q);
        atomicStore(&contact_extent[CONTACT_EXTENT_HIGH], -NO_EXTENT_Q);
        atomicStore(&contact_extent[CONTACT_ALONG_LOW], NO_EXTENT_Q);
        atomicStore(&contact_extent[CONTACT_ALONG_HIGH], -NO_EXTENT_Q);
        atomicStore(&contact_extent[CONTACT_BOX_FIRST], NO_EXTENT_Q);
        atomicStore(&contact_extent[CONTACT_BOX_FIRST + 1u], NO_EXTENT_Q);
        atomicStore(&contact_extent[CONTACT_BOX_LAST], -NO_EXTENT_Q);
        atomicStore(&contact_extent[CONTACT_BOX_LAST + 1u], -NO_EXTENT_Q);
    }
}

@compute @workgroup_size(8, 8)
fn clear_loads(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= dab.tip_size.x || id.y >= dab.tip_size.y) {
        return;
    }
    atomicStore(&texel_load[texel_slot(vec2<u32>(dab.tip_origin) + id.xy)], 0u);
}

@compute @workgroup_size(CONTACT_WORKGROUP_SIZE)
fn rasterize(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(local_invocation_index) thread: u32,
) {
    let bristle = group.x / BRISTLE_SEGMENTS;
    let segment = group.x % BRISTLE_SEGMENTS;
    if (bristle >= dab.bristle_total) {
        return;
    }
    let rest = rests[bristle];
    let slot = swept_slot(bristle, segment);
    let now = touching_span(bristle, segment, rest.radius_px);
    var before = now;
    let remembered = remembered_span(slot);
    if (continues_previous_dab() && remembered.touching) {
        before = remembered;
    }
    storageBarrier();
    if (thread == 0u) {
        remember_span(slot, now);
    }
    if (now.touching) {
        let first = first_touching_arc(bristle, rest.radius_px);
        let sweep = Sweep(before, now, rest.radius_px, first, hair_draw_log(bristle));
        rasterize_sweep(sweep, bristle, thread);
    }
}

@compute @workgroup_size(8, 8)
fn census(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= dab.canvas_size.x || id.y >= dab.canvas_size.y) {
        return;
    }
    let slot = id.y * dab.canvas_size.x + id.x;
    let word = atomicLoad(&contact_mask[slot]);
    if (coverage_level(word) == 0u) {
        return;
    }
    let level = min(atomicLoad(&summed_coverage[slot]), CONTACT_LEVELS);
    atomicStore(&contact_mask[slot], with_coverage_level(word, level));
    atomicAdd(&texel_load[cell_slot(unpacked_contact(word))], level);
}
