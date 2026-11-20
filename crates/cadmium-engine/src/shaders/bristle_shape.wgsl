fn rest_clamp_local(rest: BristleRest) -> vec3<f32> {
    return normalize(vec3<f32>(base_lean(rest), -1.0));
}

/// The rest turn of every hinge past the clamp, in the ferrule frame: a small curl at
/// right angles to the clamp, so a pressed strand buckles the same way every time.
fn rest_curl_local(rest: BristleRest) -> vec3<f32> {
    return cross(rest_clamp_local(rest), vec3<f32>(rest.curl, 0.0));
}

/// The strand at rest in the ferrule frame: a planar arc of the curl from the clamp.
fn rest_chain_local(rest: BristleRest) -> Chain {
    var chain: Chain;
    let curl = rest_curl_local(rest);
    var edge = rest_clamp_local(rest);
    chain[0] = vec3<f32>(rest.root_px, 0.0);
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        if vertex > 1u {
            edge = turned_by(edge, curl);
        }
        chain[vertex] = chain[vertex - 1u] + edge * segment_length(rest);
    }
    return chain;
}

/// What the solve needs of strand `rest` at ferrule pose `pose`.
fn strand_shape(rest: BristleRest, pose: Pose) -> StrandShape {
    var compliances: HingeCompliances;
    let segment_m = segment_length(rest) * METRES_PER_PX;
    for (var hinge = 0u; hinge < BRISTLE_SEGMENTS; hinge++) {
        compliances[hinge] = hinge_compliance_per_m(hinge) * segment_m / rest.stiffness;
    }
    return StrandShape(
        direction_at(rest_clamp_local(rest), pose),
        direction_at(rest_curl_local(rest), pose),
        segment_length(rest),
        compliances,
        vec3<f32>(0.0),
        Friction(tuning.friction.x, tuning.friction.y, tuning.friction.z),
        Turns(),
    );
}
