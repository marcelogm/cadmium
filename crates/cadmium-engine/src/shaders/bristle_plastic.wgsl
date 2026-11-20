/// Bend past which a hinge starts to set, rad [U].
const PLASTIC_YIELD_RAD: f32 = 0.02;
/// Largest plastic shift of one hinge's rest turn, rad: Chu & Tai's zero-shift cap.
const MAX_PLASTIC_SHIFT_RAD: f32 = 0.1;

/// The share of the excess over the yield a hinge sets in `dt_s` at `per_second`.
fn creep_share(per_second: f32, dt_s: f32) -> f32 {
    return clamp(per_second * dt_s, 0.0, 1.0);
}

/// How far hinge `hinge`, from edge `before` to `edge`, stands from its rest turn plus its
/// plastic shift, world.
fn hinge_excess(before: vec3<f32>, edge: vec3<f32>, shape: StrandShape, hinge: u32) -> vec3<f32> {
    let curl = select(vec3<f32>(0.0), transported(shape.curl, shape.clamp, before), hinge > 0u);
    let shift = transported(shape.plastic[hinge], shape.clamp, before);
    return rotation_between(before, edge) - curl - shift;
}

fn excess_sizes(chain: Chain, shape: StrandShape) -> PerContact {
    let edges = edges_of(chain, shape.clamp);
    var sizes: PerContact;
    var before = shape.clamp;
    for (var hinge = 0u; hinge < BRISTLE_SEGMENTS; hinge++) {
        sizes[hinge] = length(hinge_excess(before, edges[hinge], shape, hinge));
        before = edges[hinge];
    }
    return sizes;
}

/// The plastic shifts after the hinges set `share` of their excess over the yield
/// (Baxter's joint friction, Chu & Tai's zero-shift), each capped; world, at the clamp.
fn crept(chain: Chain, shape: StrandShape, share: f32) -> Turns {
    if share <= 0.0 {
        return shape.plastic;
    }
    let edges = edges_of(chain, shape.clamp);
    var plastic: Turns;
    var before = shape.clamp;
    for (var hinge = 0u; hinge < BRISTLE_SEGMENTS; hinge++) {
        let excess = hinge_excess(before, edges[hinge], shape, hinge);
        let size = length(excess);
        var shift = transported(shape.plastic[hinge], shape.clamp, before);
        if size > PLASTIC_YIELD_RAD {
            shift += share * (size - PLASTIC_YIELD_RAD) / size * excess;
        }
        shift *= MAX_PLASTIC_SHIFT_RAD / max(length(shift), MAX_PLASTIC_SHIFT_RAD);
        plastic[hinge] = transported(shift, before, shape.clamp);
        before = edges[hinge];
    }
    return plastic;
}
