alias Chain = array<vec3<f32>, BRISTLE_VERTICES>;
alias Anchors = array<vec4<f32>, BRISTLE_VERTICES>;

const HELD: f32 = 1.0;

fn direction_or_zero(offset: vec2<f32>) -> vec2<f32> {
    let span = length(offset);
    let direction = offset / max(span, MIN_SEGMENT_SPAN_PX);
    return select(vec2<f32>(0.0), direction, span > MIN_SEGMENT_SPAN_PX);
}

fn drag_share(slide: vec2<f32>, outward: vec2<f32>) -> f32 {
    let pulling = max(-dot(direction_or_zero(slide), direction_or_zero(outward)), 0.0);
    return 1.0 - tuning.drag_anisotropy * pulling;
}

fn grip(chain: ptr<function, Chain>, anchors: ptr<function, Anchors>, vertex: u32, pressed: f32) {
    let at = (*chain)[vertex];
    let anchor = (*anchors)[vertex];
    if anchor.w != HELD {
        (*anchors)[vertex] = vec4<f32>(at, HELD);
        return;
    }
    let slide = at.xy - anchor.xy;
    let distance = length(slide);
    let share = drag_share(slide, at.xy - (*chain)[vertex - 1u].xy) * pressed;
    if distance <= tuning.static_friction * share {
        (*chain)[vertex] = vec3<f32>(anchor.xy, at.z);
        return;
    }
    let held = tuning.kinetic_friction * share;
    let slid = vec3<f32>(at.xy - slide / distance * held, at.z);
    (*chain)[vertex] = slid;
    (*anchors)[vertex] = vec4<f32>(slid, HELD);
}

fn touch_canvas(chain: ptr<function, Chain>, anchors: ptr<function, Anchors>, floors: Floors) {
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let clearance = (*chain)[vertex].z - floors[vertex];
        (*chain)[vertex].z = max((*chain)[vertex].z, floors[vertex]);
        if clearance < 0.0 {
            grip(chain, anchors, vertex, -clearance);
        } else if clearance > TOUCH_TOLERANCE_PX {
            (*anchors)[vertex] = vec4<f32>(0.0);
        }
    }
}
