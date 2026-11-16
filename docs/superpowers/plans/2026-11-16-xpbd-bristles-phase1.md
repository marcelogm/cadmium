# Quasi-static Bristles — Phase 1 (Core) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the bristle step's positional relaxations with the quasi-static per-strand
solve of the spec (#62, phase 1). The new step has:
- hinge coordinates, bending compliance from E·I, and an exact clamp and inextensibility;
- frictionless canvas contact through an active set;
- pressure mapped to the handle height above the rest shape's touch;
- state that persists across strokes.

The old step goes, and with it friction, bend memory, the wet point and clumping, until
phases 2 and 3 bring them back.

**Architecture:**
- The solve is a pure WGSL snippet, `bristle_solve.wgsl` plus `bristle_contacts.wgsl`. It
  takes one strand's chain, rest shape, compliances and floors, runs one Gauss-Newton
  iteration and returns the normal force.
- The dab step (`bristle_step.wgsl`) calls the solve once per substep:
  - it measures the touch height,
  - interpolates the ferrule pose,
  - carries each chain rigidly with it,
  - and calls the solve.
- A test-only lab pipeline (`engine/bristles/lab.rs`) runs the same snippet on single
  strands, so the mechanics are checked against `mechanics` on the GPU without a brush in
  the way.
- A Rust bridge (`engine/bristles/material.rs`) turns today's `HairSpec` into per-hinge
  compliances until phase 4 brings `Hair`.

**Tech Stack:** Rust 2024, wgpu compute (WGSL), `bytemuck`.

Spec: `docs/superpowers/specs/2026-11-16-xpbd-bristles-design.md`, sections 1–3 and 5.
The prototype that chose the solve is in the spec's section 2 ("Why quasi-static").

---

## Ground rules for every task

- **Branch:** `feat/xpbd-phase1`, created from `feat/xpbd-phase0` (already done). The PR
  targets `feat/xpbd-phase0` until #63 merges.
- **Running cargo:** every cargo command runs in the `cadmium-dev` Docker image, never on
  the host. From the repo root:

  ```bash
  DOCKER="docker run --rm --device /dev/dri --group-add $(stat -c %g /dev/dri/renderD128) --group-add $(stat -c %g /dev/dri/card* | head -1) --security-opt label=disable -u $(id -u):$(id -g) -v $PWD:$PWD -w $PWD -v $HOME/.cargo:/cargo -e CARGO_HOME=/cargo -e HOME=/tmp cadmium-dev"
  $DOCKER cargo test --release -p cadmium-engine --lib bristles
  ```

  The Bash tool needs `dangerouslyDisableSandbox: true` for Docker.
- **Lints:** `cargo clippy --workspace --all-targets --release -- -D warnings`, with the
  `clippy.toml` limits:
  - functions ≤ 40 lines;
  - cognitive complexity ≤ 10;
  - nesting ≤ 3;
  - ≤ 5 arguments.

  `.github/check-file-sizes.sh` rejects `.rs`/`.wgsl` files over 400 lines.
- **Code style:**
  - Unit tests live in a sibling `foo/tests.rs`, and `foo.rs` ends with
    `#[cfg(test)] mod tests;`.
  - Name constants; no magic numbers.
  - Doc comments say what a thing is.
- **Commits:** short conventional subject. **No `Co-Authored-By`, no "Generated with",
  no AI attribution of any kind.**
- **Ignored tests:** a test that needs a mechanism a later phase brings back gets
  `#[ignore = "phase N of #62: <what it needs>"]`, the convention phase 0 set.

## File structure

| File | Status | Responsibility |
|---|---|---|
| `crates/cadmium-engine/src/engine/bristles/material.rs` (+ `material/tests.rs`) | create | `HairSpec` → modulus, diameter, taper; per-hinge compliance per metre of segment |
| `crates/cadmium-engine/src/shaders/bristle_solve.wgsl` | create | Pure: hinge turns, rest curl transport, one Gauss-Newton iteration, quaternion apply |
| `crates/cadmium-engine/src/shaders/bristle_contacts.wgsl` | create | Pure: contact compliance matrix, active set, Cholesky |
| `crates/cadmium-engine/src/shaders/bristle_lab.wgsl` | create | Test entry point `solve_lab` on single strands |
| `crates/cadmium-engine/src/engine/bristles/lab.rs` (+ `lab/tests.rs`) | create (`#[cfg(test)]`) | Runs the lab pipeline; GPU mechanics tests against `mechanics` |
| `crates/cadmium-engine/src/shaders/bristle_shape.wgsl` | create | Rest chain in the ferrule frame, strand shape at a pose |
| `crates/cadmium-engine/src/shaders/bristle_step.wgsl` | rewrite | Touch height, poses, rigid carry, substep loop, outputs |
| `crates/cadmium-engine/src/shaders/bristle_frame.wgsl` | modify | New `HandleState`, positions-only slots, ferrule-frame transforms |
| `crates/cadmium-engine/src/shaders/bristle.wgsl` | modify | Keep `segment_length`, `base_lean`, `turned`, `pose_between`; drop the hang |
| `crates/cadmium-engine/src/shaders/bristle_relief.wgsl` | modify | Keep the floor lookup and `workgroup_range`; drop the base bisection and `hair_load` |
| `crates/cadmium-engine/src/shaders/bristle_tuning.wgsl` | rewrite | Wet support, tooth, hinge compliances |
| `crates/cadmium-engine/src/shaders/bristle_rest.wgsl` | modify | `curl` field |
| `crates/cadmium-engine/src/shaders/bristle_{friction,memory,point,clump}.wgsl` | delete | Old step |
| `crates/cadmium-engine/src/shaders/brush_tip.wgsl` | modify | `Dab` field renames |
| `crates/cadmium-engine/src/engine/dab_record.rs` | modify | Same renames |
| `crates/cadmium-engine/src/engine/bristles/{layout,state,rig,handle,tuning}.rs` (+ tests) | modify | 8 vertices, curl, positions-only state, normal forces, persistent strokes, offsets |
| `crates/cadmium-engine/src/engine/bristles.rs`, `engine/shader.rs`, `engine/pass/bristle_step.rs`, `engine.rs` | modify | Wiring |
| `crates/cadmium-engine/src/brush/head.rs` (+ tests) | modify | Remove `point_axes` (dead without the wet point) |
| `crates/cadmium-engine/tests/bristles/*.rs` | modify | New invariants; ignores; removed-mechanism tests deleted |
| `docs/superpowers/specs/2026-11-16-xpbd-bristles-design.md` | modify | Head stiffness moves to phase 2 |

---

### Task 1: Material bridge

**Files:**
- Create: `crates/cadmium-engine/src/engine/bristles/material.rs`
- Create: `crates/cadmium-engine/src/engine/bristles/material/tests.rs`
- Modify: `crates/cadmium-engine/src/engine/bristles.rs` (add `mod material;`)

- [ ] **Step 1: Write the failing tests** in `material/tests.rs`:

```rust
use super::*;
use crate::brush::HairSpec;

const FERRULE_RADIUS_PX: f32 = 20.0;
const STRANDS: usize = 64;

fn compliances(hair: &HairSpec, strands: usize) -> [f32; HINGES] {
    hinge_compliances_per_m(&HairMaterial::of(hair), FERRULE_RADIUS_PX, strands)
}

#[test]
fn a_stiffer_hair_bends_less() {
    let soft = compliances(&HairSpec::SQUIRREL, STRANDS);
    let stiff = compliances(&HairSpec::HOG, STRANDS);
    assert!(soft.iter().zip(&stiff).all(|(s, h)| s > h), "{soft:?} vs {stiff:?}");
}

#[test]
fn the_modulus_spans_one_to_seven_gigapascals() {
    let at = |stiffness| HairMaterial::of(&HairSpec { stiffness, ..HairSpec::default() });
    assert_eq!(at(0.0).modulus_pa, 1.0e9);
    assert_eq!(at(1.0).modulus_pa, 7.0e9);
}

#[test]
fn the_clamp_hinge_spans_half_a_segment() {
    let material = HairMaterial { tip_taper: 1.0, ..HairMaterial::of(&HairSpec::default()) };
    let c = hinge_compliances_per_m(&material, FERRULE_RADIUS_PX, STRANDS);
    assert!((c[1] / c[0] - 2.0).abs() < 1e-5, "{c:?}");
}

#[test]
fn the_tapered_tip_is_softer_by_the_fourth_power_of_its_radius() {
    let material = HairMaterial::of(&HairSpec::default());
    let c = hinge_compliances_per_m(&material, FERRULE_RADIUS_PX, STRANDS);
    let share = |hinge: usize| 1.0 - (1.0 - material.tip_taper) * hinge as f64 / HINGES as f64;
    let want = (share(1) / share(HINGES - 1)).powi(4);
    let got = f64::from(c[HINGES - 1] / c[1]);
    assert!((got / want - 1.0).abs() < 1e-4, "{got} vs {want}");
}

#[test]
fn simulating_twice_the_strands_halves_each_strands_hair_count() {
    let one = compliances(&HairSpec::default(), STRANDS);
    let two = compliances(&HairSpec::default(), 2 * STRANDS);
    assert!(one.iter().zip(&two).all(|(a, b)| (b / a - 2.0).abs() < 1e-4));
}
```

- [ ] **Step 2: Run, expect a compile failure** (`hinge_compliances_per_m` undefined):

`$DOCKER cargo test --release -p cadmium-engine --lib material`

- [ ] **Step 3: Write `material.rs`:**

```rust
//! The physical hair the quasi-static solve bends, built from today's `HairSpec` until
//! phase 4 of #62 brings `Hair` in physical units. Every number here is an estimate [U]
//! (docs/superpowers/specs/2026-11-16-xpbd-bristles-literature.md).

use super::layout::BRISTLE_VERTICES;
use crate::brush::HairSpec;
use crate::mechanics::{bending_stiffness, metres_from_px, real_hair_count};

/// Hinges per strand: one per edge, the first being the clamp to the ferrule.
pub(crate) const HINGES: usize = BRISTLE_VERTICES - 1;
/// Young's modulus at `HairSpec::stiffness` 0 and 1, so squirrel ≈ 2, sable ≈ 3.4,
/// synthetic ≈ 4 and hog ≈ 6 GPa [U].
const MODULUS_RANGE_PA: [f64; 2] = [1.0e9, 7.0e9];
/// Root diameter of one hair [U].
const HAIR_DIAMETER_M: f64 = 75.0e-6;
/// Tip radius over root radius [U].
const TIP_TAPER: f64 = 0.6;
/// Share of the ferrule's section the hairs fill [U].
const PACKING: f64 = 0.7;
/// The clamp closes the strand, so its hinge bends over half a segment.
const CLAMP_SPAN_SHARE: f64 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HairMaterial {
    pub(crate) modulus_pa: f64,
    pub(crate) diameter_m: f64,
    pub(crate) tip_taper: f64,
}

impl HairMaterial {
    pub(crate) fn of(hair: &HairSpec) -> HairMaterial {
        let [soft, stiff] = MODULUS_RANGE_PA;
        HairMaterial {
            modulus_pa: soft + f64::from(hair.stiffness) * (stiff - soft),
            diameter_m: HAIR_DIAMETER_M,
            tip_taper: TIP_TAPER,
        }
    }

    fn radius_at(&self, share: f64) -> f64 {
        0.5 * self.diameter_m * (1.0 - (1.0 - self.tip_taper) * share)
    }
}

/// Each hinge's bending compliance per metre of segment, in 1/(N·m²): the shader
/// multiplies by the strand's segment length to get rad per N·m. A strand stands for a
/// tuft of N_real/N_sim hairs whose EI add, so the head's stiffness does not depend on
/// how many strands are simulated.
pub(crate) fn hinge_compliances_per_m(
    material: &HairMaterial,
    ferrule_radius_px: f32,
    strands: usize,
) -> [f32; HINGES] {
    let real = real_hair_count(PACKING, metres_from_px(f64::from(ferrule_radius_px)), material.diameter_m);
    let hairs = real / strands.max(1) as f64;
    std::array::from_fn(|hinge| {
        let share = hinge as f64 / HINGES as f64;
        let span = if hinge == 0 { CLAMP_SPAN_SHARE } else { 1.0 };
        let stiffness = hairs * bending_stiffness(material.modulus_pa, material.radius_at(share));
        (span / stiffness) as f32
    })
}

#[cfg(test)]
mod tests;
```

- [ ] **Step 4: Run the tests and expect them to pass.** Run the same command. All 5 should pass.
- [ ] **Step 5: Commit** with `git commit -m "feat(engine): hair material bridge for the quasi-static bristles"`.

---

### Task 2: The pure strand solve and the GPU lab

**Files:**
- Create: `crates/cadmium-engine/src/shaders/bristle_solve.wgsl`
- Create: `crates/cadmium-engine/src/shaders/bristle_contacts.wgsl`
- Create: `crates/cadmium-engine/src/shaders/bristle_lab.wgsl`
- Create: `crates/cadmium-engine/src/engine/bristles/lab.rs`
- Create: `crates/cadmium-engine/src/engine/bristles/lab/tests.rs`
- Modify: `crates/cadmium-engine/src/engine/shader.rs` (`LAB_SNIPPETS`)
- Modify: `crates/cadmium-engine/src/engine/bristles.rs` (`#[cfg(test)] mod lab;`)

This task does not depend on `BRISTLE_VERTICES` being 8, but the tests size the strand
with it, so they keep working after Task 3.

- [ ] **Step 1: Write the failing lab tests** in `lab/tests.rs`:

```rust
use super::*;
use crate::mechanics::{cantilever_tip_deflection, metres_from_px};

const LENGTH_PX: f32 = 30.0;
/// EI of the test strand, N·m²: about three 60 µm hairs at 4 GPa.
const STIFFNESS_N_M2: f64 = 7.6e-9;
const BEAM_TOLERANCE: f64 = 0.05;
const FLOOR_FAR_BELOW_PX: f32 = -1.0e4;
const SEGMENTS: usize = BRISTLE_VERTICES - 1;

fn segment_px() -> f32 {
    LENGTH_PX / SEGMENTS as f32
}

/// Uniform compliances, rad per N·m, with the clamp spanning half a segment.
fn uniform_compliances() -> [f32; 8] {
    let segment_m = metres_from_px(f64::from(segment_px()));
    std::array::from_fn(|hinge| match hinge {
        0 => (0.5 * segment_m / STIFFNESS_N_M2) as f32,
        h if h < SEGMENTS => (segment_m / STIFFNESS_N_M2) as f32,
        _ => 0.0,
    })
}

fn strand(clamp: [f32; 3], root: [f32; 3]) -> LabStrand {
    LabStrand::new(root, clamp, segment_px(), uniform_compliances())
}

fn tilted(degrees_below: f32) -> [f32; 3] {
    let angle = degrees_below.to_radians();
    [angle.cos(), 0.0, -angle.sin()]
}

#[test]
fn a_tip_load_bends_the_strand_as_a_cantilever() {
    let gpu = GpuContext::new().unwrap();
    let want_px = 0.5;
    let length_m = metres_from_px(f64::from(LENGTH_PX));
    let force_n = 3.0 * STIFFNESS_N_M2 * metres_from_px(want_px) / length_m.powi(3);
    let loaded = strand([1.0, 0.0, 0.0], [0.0, 0.0, 0.0])
        .with_floor(FLOOR_FAR_BELOW_PX)
        .with_tip_force([0.0, 0.0, -(force_n as f32)])
        .with_press(0.0, 4);
    let run = &run(&gpu, &[loaded])[0];
    let got_px = -f64::from(run.chain_px[SEGMENTS][2]);
    let exact_px = cantilever_tip_deflection(force_n, length_m, STIFFNESS_N_M2) / metres_from_px(1.0);
    assert!((got_px / exact_px - 1.0).abs() <= BEAM_TOLERANCE, "{got_px} px vs {exact_px} px");
}

/// The rest strand at `degrees` below horizontal with its tip on the floor at z = 0,
/// pressed `press_px` down in `steps`.
fn pressed_tilted(degrees: f32, press_px: f32, steps: u32) -> LabStrand {
    let clamp = tilted(degrees);
    let rise = -clamp[2] * LENGTH_PX;
    strand(clamp, [0.0, 0.0, rise]).with_floor(0.0).with_press(press_px, steps)
}

#[test]
fn a_pressed_tilted_strand_pushes_back_as_a_beam() {
    let gpu = GpuContext::new().unwrap();
    let degrees = 20.0_f32;
    let press_px = 0.3;
    let run = &run(&gpu, &[pressed_tilted(degrees, press_px, 1)])[0];
    let length_m = metres_from_px(f64::from(LENGTH_PX));
    let cosine = f64::from(degrees.to_radians().cos());
    // Tip load N on a beam at angle θ: the tip drops Δz = N·L³·cos²θ / (3EI).
    let exact_n = 3.0 * STIFFNESS_N_M2 * metres_from_px(f64::from(press_px))
        / (length_m.powi(3) * cosine * cosine);
    let got_n = f64::from(run.normal_force_n);
    assert!((got_n / exact_n - 1.0).abs() <= BEAM_TOLERANCE, "{got_n} N vs {exact_n} N");
}

fn tip_reach_px(run: &LabRun) -> f32 {
    let tip = run.chain_px[SEGMENTS];
    tip[0].hypot(tip[1])
}

#[test]
fn an_upright_strand_pressed_down_buckles_wider_and_never_sinks() {
    let gpu = GpuContext::new().unwrap();
    let presses = [5.0, 10.0, 15.0];
    let strands: Vec<LabStrand> = presses
        .iter()
        .map(|&press| {
            strand([0.0, 0.0, -1.0], [0.0, 0.0, LENGTH_PX])
                .with_curl([0.0, 0.02, 0.0])
                .with_floor(0.0)
                .with_press(press, 60)
        })
        .collect();
    let runs = run(&gpu, &strands);
    for run in &runs {
        assert!(run.chain_px.iter().all(|at| at[2] >= -1e-3), "{:?}", run.chain_px);
        for pair in run.chain_px.windows(2) {
            let span = (0..3).map(|a| (pair[1][a] - pair[0][a]).powi(2)).sum::<f32>().sqrt();
            assert!((span - segment_px()).abs() < 1e-3, "segment {span}");
        }
    }
    let reaches: Vec<f32> = runs.iter().map(tip_reach_px).collect();
    assert!(reaches.windows(2).all(|pair| pair[1] > pair[0]), "{reaches:?}");
}

#[test]
fn pressing_in_twice_the_steps_ends_in_the_same_shape() {
    let gpu = GpuContext::new().unwrap();
    let runs = run(&gpu, &[pressed_tilted(45.0, 3.0, 4), pressed_tilted(45.0, 3.0, 8)]);
    for (a, b) in runs[0].chain_px.iter().zip(&runs[1].chain_px) {
        let gap = (0..3).map(|axis| (a[axis] - b[axis]).abs()).fold(0.0, f32::max);
        assert!(gap < 0.05, "{a:?} vs {b:?}");
    }
}
```

- [ ] **Step 2: Run them and expect a compile failure.**

  Run `$DOCKER cargo test --release -p cadmium-engine --lib lab`. It fails because the `lab` module does not exist.

- [ ] **Step 3: Write `bristle_solve.wgsl`:**

```wgsl
alias Chain = array<vec3<f32>, BRISTLE_VERTICES>;
alias Floors = array<f32, BRISTLE_VERTICES>;
alias Edges = array<vec3<f32>, BRISTLE_SEGMENTS>;
alias Turns = array<vec3<f32>, BRISTLE_SEGMENTS>;
alias HingeCompliances = array<f32, BRISTLE_SEGMENTS>;

const METRES_PER_PX: f32 = RELIEF_UM_PER_PX * 1.0e-6;
const UP: vec3<f32> = vec3<f32>(0.0, 0.0, 1.0);
/// Largest turn a hinge takes in one iteration: a guard for steps that leave the linear
/// range, not physics.
const MAX_HINGE_TURN_RAD: f32 = 0.3;
const MIN_TURN_RAD: f32 = 1.0e-6;
const MIN_EDGE_PX: f32 = 1.0e-4;
const MIN_TRANSPORT_COSINE: f32 = 1.0e-3;

/// What the solve needs to know about one strand at the current ferrule pose.
struct StrandShape {
    /// Rest direction of the first edge, world.
    clamp: vec3<f32>,
    /// Rest turn of every hinge past the clamp, world, at the clamp.
    curl: vec3<f32>,
    segment_px: f32,
    /// Bending compliance per hinge, rad per N·m.
    compliances: HingeCompliances,
    /// A load on the tip, N. Zero in the app; the lab uses it.
    tip_force_n: vec3<f32>,
};

/// The rotation vector of the shortest turn from unit `from` to unit `to`.
fn rotation_between(from: vec3<f32>, to: vec3<f32>) -> vec3<f32> {
    let axis = cross(from, to);
    let sine = length(axis);
    if sine <= MIN_TURN_RAD {
        return vec3<f32>(0.0);
    }
    return axis / sine * atan2(sine, dot(from, to));
}

/// `v` turned by the rotation vector `turn` (Rodrigues).
fn turned_by(v: vec3<f32>, turn: vec3<f32>) -> vec3<f32> {
    let angle = length(turn);
    if angle <= MIN_TURN_RAD {
        return v + cross(turn, v);
    }
    let k = turn / angle;
    let c = cos(angle);
    return v * c + cross(k, v) * sin(angle) + k * dot(k, v) * (1.0 - c);
}

/// `v` carried by the shortest turn from unit `from` to unit `to`: parallel transport.
fn transported(v: vec3<f32>, from: vec3<f32>, to: vec3<f32>) -> vec3<f32> {
    let c = dot(from, to);
    let w = cross(from, to);
    return v * c + cross(w, v) + w * (dot(w, v) / max(1.0 + c, MIN_TRANSPORT_COSINE));
}

fn edges_of(chain: Chain, clamp: vec3<f32>) -> Edges {
    var edges: Edges;
    var previous = clamp;
    for (var edge = 0u; edge < BRISTLE_SEGMENTS; edge++) {
        let span = chain[edge + 1u] - chain[edge];
        let size = length(span);
        edges[edge] = select(previous, span / max(size, MIN_EDGE_PX), size > MIN_EDGE_PX);
        previous = edges[edge];
    }
    return edges;
}

/// The step of the bending energy alone: each hinge turns back to its rest turn, plus
/// what the tip load bends it, before the canvas pushes back.
fn free_turns(chain: Chain, edges: Edges, shape: StrandShape) -> Turns {
    var turns: Turns;
    var before = shape.clamp;
    let tip = chain[BRISTLE_VERTICES - 1u];
    for (var hinge = 0u; hinge < BRISTLE_SEGMENTS; hinge++) {
        let curl = transported(shape.curl, shape.clamp, before);
        let rest = select(vec3<f32>(0.0), curl, hinge > 0u);
        let lever_m = (tip - chain[hinge]) * METRES_PER_PX;
        let loaded = shape.compliances[hinge] * cross(lever_m, shape.tip_force_n);
        turns[hinge] = rest - rotation_between(before, edges[hinge]) + loaded;
        before = edges[hinge];
    }
    return turns;
}

fn capped(turn: vec3<f32>) -> vec3<f32> {
    return turn * min(1.0, MAX_HINGE_TURN_RAD / max(length(turn), MIN_TURN_RAD));
}

fn quaternion_of(turn: vec3<f32>) -> vec4<f32> {
    let angle = length(turn);
    if angle <= MIN_TURN_RAD {
        return normalize(vec4<f32>(0.5 * turn, 1.0));
    }
    return vec4<f32>(turn / angle * sin(0.5 * angle), cos(0.5 * angle));
}

fn composed(outer: vec4<f32>, inner: vec4<f32>) -> vec4<f32> {
    let axis = outer.w * inner.xyz + inner.w * outer.xyz + cross(outer.xyz, inner.xyz);
    return vec4<f32>(axis, outer.w * inner.w - dot(outer.xyz, inner.xyz));
}

fn rotated(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    let t = 2.0 * cross(q.xyz, v);
    return v + q.w * t + cross(q.xyz, t);
}

/// Turns every edge by its hinge and all hinges above it, and rebuilds the chain from the
/// root with exact segment lengths.
fn turn_chain(chain: ptr<function, Chain>, edges: Edges, turns: Turns, segment_px: f32) {
    var total = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    for (var edge = 0u; edge < BRISTLE_SEGMENTS; edge++) {
        total = normalize(composed(quaternion_of(capped(turns[edge])), total));
        let direction = normalize(rotated(total, edges[edge]));
        (*chain)[edge + 1u] = (*chain)[edge] + direction * segment_px;
    }
}

/// One Gauss-Newton step of the strand's bending energy under its canvas contacts, from
/// `chain` (vertex 0 already on the ferrule). Returns the canvas's normal force, N.
fn solve_iteration(chain: ptr<function, Chain>, shape: StrandShape, floors: Floors) -> f32 {
    let edges = edges_of(*chain, shape.clamp);
    var turns = free_turns(*chain, edges, shape);
    let forces = contact_forces(*chain, turns, shape.compliances, floors);
    var total = 0.0;
    for (var pushed = 1u; pushed < BRISTLE_VERTICES; pushed++) {
        let force = forces[pushed - 1u];
        total += force;
        for (var hinge = 0u; hinge < pushed; hinge++) {
            let arm_m = ((*chain)[pushed] - (*chain)[hinge]) * METRES_PER_PX;
            turns[hinge] += shape.compliances[hinge] * force * cross(arm_m, UP);
        }
    }
    turn_chain(chain, edges, turns, shape.segment_px);
    return total;
}
```

- [ ] **Step 4: Write `bristle_contacts.wgsl`:**

```wgsl
alias Gaps = array<f32, BRISTLE_SEGMENTS>;
alias Active = array<bool, BRISTLE_SEGMENTS>;
alias ContactMatrix = array<array<f32, BRISTLE_SEGMENTS>, BRISTLE_SEGMENTS>;

const CONTACT_ROUNDS: u32 = 8u;
/// How far under its floor a vertex left out of the active set may end, px.
const CONTACT_TOLERANCE_PX: f32 = 1.0e-3;
/// Share of the largest diagonal added to every active pivot: a straight strand pressed
/// along its length has no bending answer, and this keeps it from dividing by zero.
const CONTACT_RIDGE_SHARE: f32 = 1.0e-5;
const MIN_PIVOT: f32 = 1.0e-20;

/// How far vertex `vertex` rises, px, under 1 N pushing vertex `pushed` up (indices are
/// vertices, 1-based): Σ over the hinges above both of α·(lever · arm) in the canvas plane.
fn contact_compliance(chain: Chain, compliances: HingeCompliances, vertex: u32, pushed: u32) -> f32 {
    var rise = 0.0;
    for (var hinge = 0u; hinge < min(vertex, pushed); hinge++) {
        let lever = (chain[vertex] - chain[hinge]).xy;
        let arm_m = (chain[pushed] - chain[hinge]).xy * METRES_PER_PX;
        rise += compliances[hinge] * dot(lever, arm_m);
    }
    return rise;
}

fn contact_matrix(chain: Chain, compliances: HingeCompliances) -> ContactMatrix {
    var matrix: ContactMatrix;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        for (var column = 0u; column < BRISTLE_SEGMENTS; column++) {
            matrix[row][column] = contact_compliance(chain, compliances, row + 1u, column + 1u);
        }
    }
    return matrix;
}

/// How far under its floor each vertex (1..) would end after `turns`, px.
fn gaps_after(chain: Chain, turns: Turns, floors: Floors) -> Gaps {
    var gaps: Gaps;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        var z = chain[vertex].z;
        for (var hinge = 0u; hinge < vertex; hinge++) {
            z += cross(turns[hinge], chain[vertex] - chain[hinge]).z;
        }
        gaps[vertex - 1u] = floors[vertex] - z;
    }
    return gaps;
}

fn factorised(system: ContactMatrix) -> ContactMatrix {
    var lower = system;
    for (var column = 0u; column < BRISTLE_SEGMENTS; column++) {
        var pivot = lower[column][column];
        for (var k = 0u; k < column; k++) {
            pivot -= lower[column][k] * lower[column][k];
        }
        let root = sqrt(max(pivot, MIN_PIVOT));
        lower[column][column] = root;
        for (var row = column + 1u; row < BRISTLE_SEGMENTS; row++) {
            var entry = lower[row][column];
            for (var k = 0u; k < column; k++) {
                entry -= lower[row][k] * lower[column][k];
            }
            lower[row][column] = entry / root;
        }
    }
    return lower;
}

fn cholesky_solved(system: ContactMatrix, rhs: Gaps) -> Gaps {
    let lower = factorised(system);
    var x = rhs;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        for (var k = 0u; k < row; k++) {
            x[row] -= lower[row][k] * x[k];
        }
        x[row] /= lower[row][row];
    }
    for (var step = 0u; step < BRISTLE_SEGMENTS; step++) {
        let row = BRISTLE_SEGMENTS - 1u - step;
        for (var k = row + 1u; k < BRISTLE_SEGMENTS; k++) {
            x[row] -= lower[k][row] * x[k];
        }
        x[row] /= lower[row][row];
    }
    return x;
}

/// The forces that put every active vertex on its floor; inactive rows solve to 0.
fn active_forces(matrix: ContactMatrix, gaps: Gaps, active: Active) -> Gaps {
    var largest = 0.0;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        largest = max(largest, matrix[row][row]);
    }
    var system: ContactMatrix;
    var rhs: Gaps;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        for (var column = 0u; column < BRISTLE_SEGMENTS; column++) {
            system[row][column] = select(0.0, matrix[row][column], active[row] && active[column]);
        }
        let ridged = matrix[row][row] + CONTACT_RIDGE_SHARE * largest;
        system[row][row] = select(1.0, ridged, active[row]);
        rhs[row] = select(0.0, gaps[row], active[row]);
    }
    return cholesky_solved(system, rhs);
}

/// Drops the contacts that would pull the strand down. True if any was dropped.
fn released(active: ptr<function, Active>, forces: Gaps) -> bool {
    var dropped = false;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        if (*active)[row] && forces[row] < 0.0 {
            (*active)[row] = false;
            dropped = true;
        }
    }
    return dropped;
}

/// Adds the vertices the forces leave under their floor. True if any was added.
fn landed(active: ptr<function, Active>, matrix: ContactMatrix, gaps: Gaps, forces: Gaps) -> bool {
    var added = false;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        var rise = 0.0;
        for (var column = 0u; column < BRISTLE_SEGMENTS; column++) {
            rise += select(0.0, matrix[row][column] * forces[column], (*active)[column]);
        }
        if !(*active)[row] && gaps[row] - rise > CONTACT_TOLERANCE_PX {
            (*active)[row] = true;
            added = true;
        }
    }
    return added;
}

/// The canvas's normal force on each vertex (1..), N: the active set of a frictionless,
/// one-sided contact (Baxter & Lin's and Chu & Tai's quasi-static brushes).
fn contact_forces(chain: Chain, turns: Turns, compliances: HingeCompliances, floors: Floors) -> Gaps {
    let matrix = contact_matrix(chain, compliances);
    let gaps = gaps_after(chain, turns, floors);
    var active: Active;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        active[row] = gaps[row] > 0.0;
    }
    var forces: Gaps;
    for (var round = 0u; round < CONTACT_ROUNDS; round++) {
        forces = active_forces(matrix, gaps, active);
        let dropped = released(&active, forces);
        if !dropped && !landed(&active, matrix, gaps, forces) {
            break;
        }
    }
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        forces[row] = select(0.0, max(forces[row], 0.0), active[row]);
    }
    return forces;
}
```

- [ ] **Step 5: Write `bristle_lab.wgsl`:**

```wgsl
/// One strand for the lab, all vec4 so the layout is the same on both sides:
/// root.xyz and w = segment px; clamp.xyz and w = floor height px; curl.xyz and
/// w = press px; tip_force.xyz N and w = press steps; compliances rad per N·m.
struct LabStrand {
    root: vec4<f32>,
    clamp: vec4<f32>,
    curl: vec4<f32>,
    tip_force: vec4<f32>,
    compliances: array<vec4<f32>, 2>,
};

@group(0) @binding(0) var<storage, read> lab_strands: array<LabStrand>;
@group(0) @binding(1) var<storage, read_write> lab_out: array<vec4<f32>>;

const LAB_ITERATIONS_PER_STEP: u32 = 2u;
const LAB_OUT_VECTORS: u32 = BRISTLE_VERTICES + 1u;

fn lab_shape(strand: LabStrand) -> StrandShape {
    var compliances: HingeCompliances;
    for (var hinge = 0u; hinge < BRISTLE_SEGMENTS; hinge++) {
        compliances[hinge] = strand.compliances[hinge / 4u][hinge % 4u];
    }
    return StrandShape(strand.clamp.xyz, strand.curl.xyz, strand.root.w, compliances, strand.tip_force.xyz);
}

fn lab_rest_chain(shape: StrandShape, root: vec3<f32>) -> Chain {
    var chain: Chain;
    var edge = shape.clamp;
    chain[0] = root;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        if vertex > 1u {
            edge = turned_by(edge, shape.curl);
        }
        chain[vertex] = chain[vertex - 1u] + edge * shape.segment_px;
    }
    return chain;
}

fn flat_floors(height_px: f32) -> Floors {
    var floors: Floors;
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        floors[vertex] = height_px;
    }
    return floors;
}

@compute @workgroup_size(1)
fn solve_lab(@builtin(workgroup_id) id: vec3<u32>) {
    let strand = lab_strands[id.x];
    let shape = lab_shape(strand);
    var chain = lab_rest_chain(shape, strand.root.xyz);
    let steps = max(u32(strand.tip_force.w), 1u);
    let drop = vec3<f32>(0.0, 0.0, strand.curl.w / f32(steps));
    var force = 0.0;
    for (var step = 0u; step < steps; step++) {
        for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
            chain[vertex] -= drop;
        }
        for (var iteration = 0u; iteration < LAB_ITERATIONS_PER_STEP; iteration++) {
            force = solve_iteration(&chain, shape, flat_floors(strand.clamp.w));
        }
    }
    let first = id.x * LAB_OUT_VECTORS;
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        lab_out[first + vertex] = vec4<f32>(chain[vertex], 0.0);
    }
    lab_out[first + BRISTLE_VERTICES] = vec4<f32>(force, 0.0, 0.0, 0.0);
}
```

  For a curl, `curl.xyz` is the world rest turn. The lab tests pass one at a right angle to the clamp.

- [ ] **Step 6: Add `LAB_SNIPPETS` to `engine/shader.rs`**, after `BRISTLE_SNIPPETS`:

```rust
#[cfg(test)]
pub(super) const LAB_SNIPPETS: [&str; 2] = [
    include_str!("../shaders/bristle_solve.wgsl"),
    include_str!("../shaders/bristle_contacts.wgsl"),
];
```

  `shader_source` already emits `BRISTLE_VERTICES`, `BRISTLE_SEGMENTS` and `RELIEF_UM_PER_PX`.

- [ ] **Step 7: Write `lab.rs`:**

```rust
//! Runs the strand solve of `bristle_solve.wgsl` on single strands with flat floors, so
//! its mechanics can be checked against the closed forms in `mechanics`.

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use super::layout::BRISTLE_VERTICES;
use crate::compute::{ComputeStage, buffer_binding, read_buffer, storage_buffer, storage_entry};
use crate::engine::shader::{LAB_SNIPPETS, shader_source};
use crate::gpu::GpuContext;

const OUT_VECTORS: usize = BRISTLE_VERTICES + 1;
const VECTOR_FLOATS: usize = 4;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(super) struct LabStrand {
    root: [f32; 4],
    clamp: [f32; 4],
    curl: [f32; 4],
    tip_force: [f32; 4],
    compliances: [f32; 8],
}

impl LabStrand {
    pub(super) fn new(root: [f32; 3], clamp: [f32; 3], segment_px: f32, compliances: [f32; 8]) -> LabStrand {
        let [x, y, z] = root;
        let [cx, cy, cz] = clamp;
        LabStrand {
            root: [x, y, z, segment_px],
            clamp: [cx, cy, cz, 0.0],
            curl: [0.0; 4],
            tip_force: [0.0, 0.0, 0.0, 1.0],
            compliances,
        }
    }

    pub(super) fn with_floor(mut self, height_px: f32) -> LabStrand {
        self.clamp[3] = height_px;
        self
    }

    pub(super) fn with_curl(mut self, curl: [f32; 3]) -> LabStrand {
        self.curl[..3].copy_from_slice(&curl);
        self
    }

    pub(super) fn with_tip_force(mut self, force_n: [f32; 3]) -> LabStrand {
        self.tip_force[..3].copy_from_slice(&force_n);
        self
    }

    /// Lowers the whole strand by `press_px` in `steps`, solving after each.
    pub(super) fn with_press(mut self, press_px: f32, steps: u32) -> LabStrand {
        self.curl[3] = press_px;
        self.tip_force[3] = steps as f32;
        self
    }
}

pub(super) struct LabRun {
    pub(super) chain_px: Vec<[f32; 3]>,
    pub(super) normal_force_n: f32,
}

fn lab_stage(gpu: &GpuContext) -> (ComputeStage, wgpu::ComputePipeline) {
    let source = shader_source(&LAB_SNIPPETS, include_str!("../../shaders/bristle_lab.wgsl"));
    let stage = ComputeStage::new(gpu, "bristle_lab", &source, &[storage_entry(0, true), storage_entry(1, false)]);
    let pipeline = stage.pipeline(gpu, "bristle_lab", "solve_lab");
    (stage, pipeline)
}

pub(super) fn run(gpu: &GpuContext, strands: &[LabStrand]) -> Vec<LabRun> {
    let (stage, pipeline) = lab_stage(gpu);
    let input = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("bristle_lab_strands"),
        contents: bytemuck::cast_slice(strands),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let out_bytes = (strands.len() * OUT_VECTORS * VECTOR_FLOATS * std::mem::size_of::<f32>()) as u64;
    let output = storage_buffer(gpu, "bristle_lab_out", out_bytes);
    let bind_group = stage.bind_group(gpu, &[buffer_binding(0, &input), buffer_binding(1, &output)]);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(strands.len() as u32, 1, 1);
    }
    gpu.queue.submit([encoder.finish()]);
    let floats = read_buffer(gpu, &output).unwrap();
    floats.chunks(OUT_VECTORS * VECTOR_FLOATS).map(lab_run).collect()
}

fn lab_run(floats: &[f32]) -> LabRun {
    let vectors: Vec<&[f32]> = floats.chunks(VECTOR_FLOATS).collect();
    LabRun {
        chain_px: vectors[..BRISTLE_VERTICES].iter().map(|v| [v[0], v[1], v[2]]).collect(),
        normal_force_n: vectors[BRISTLE_VERTICES][0],
    }
}

mod tests;
```

  `bristles.rs` declares the module as `#[cfg(test)] mod lab;`, so the whole lab, its tests included, exists only in test builds.

- [ ] **Step 8: Run the lab tests.**

  Run `$DOCKER cargo test --release -p cadmium-engine --lib lab`. All 4 should pass. If the
  tilted beam is just outside 5%, check the beam-column effect before touching the tolerance. The prototype gave 1.7% at 20° with Δz = 0.3 px.

- [ ] **Step 9: Commit** with `git commit -m "feat(engine): quasi-static strand solve with a GPU lab against the beam formulas"`.

---

### Task 3: Layout — 8 vertices and a rest curl

**Files:**
- Modify: `crates/cadmium-engine/src/engine/bristles/layout.rs`
- Modify: `crates/cadmium-engine/src/shaders/bristle_rest.wgsl`
- Modify: `crates/cadmium-engine/src/engine/bristles/state.rs:42` (size assert)
- Test: `crates/cadmium-engine/src/engine/bristles/tests.rs`

- [ ] **Step 1: Add the failing test** to `bristles/tests.rs`:

```rust
#[test]
fn every_bristle_curls_a_little_and_the_curls_keep_their_seed() {
    let laid = layout(&request(HeadSpec::ROUND, 20.0));
    for bristle in &laid.bristles {
        let size = bristle.curl[0].hypot(bristle.curl[1]);
        assert!((size - layout::REST_CURL_RAD).abs() < 1e-6, "{:?}", bristle.curl);
    }
    let again = layout(&request(HeadSpec::ROUND, 20.0));
    assert_eq!(laid.bristles, again.bristles);
}
```

- [ ] **Step 2: Run it and expect a compile failure** (there is no field `curl`).
- [ ] **Step 3: Implement the change.**
  - In `layout.rs`:
    - set `BRISTLE_VERTICES = 8`;
    - add `pub(super) const REST_CURL_RAD: f32 = 0.02;` with the doc "Rest turn of every hinge past the clamp, so a pressed strand buckles the same way every time [estimate]";
    - add `const CURL_SEED_SALT: u64 = 0xC0_11;`.
  - Add `pub(crate) curl: [f32; 2]` and `pub(crate) _pad: [f32; 2]` to `BristleRest`, after `lane_end`.
  - In `layout()`, draw the curls from their own generator so the other jitters keep their sequence:

```rust
    let mut curls = SplitMix::new(request.seed ^ CURL_SEED_SALT);
    // inside the map closure, after lane_end:
            curl: rest_curl(&mut curls),
            _pad: [0.0; 2],
```

```rust
fn rest_curl(random: &mut SplitMix) -> [f32; 2] {
    let angle = std::f32::consts::TAU * random.unit();
    [REST_CURL_RAD * angle.cos(), REST_CURL_RAD * angle.sin()]
}
```

  - In `state.rs`, change the assert to `size_of::<BristleRest>() == 64`.
  - In `bristle_rest.wgsl`, add `curl: vec2<f32>, _pad: vec2<f32>,` after `lane_end`.
- [ ] **Step 4: Run the layout tests.**

  Run `$DOCKER cargo test --release -p cadmium-engine --lib bristles`. The layout tests pass. The old step still compiles: it reads `BRISTLE_VERTICES` from constants, so it now runs on 8 vertices.

- [ ] **Step 5: Commit** with `git commit -m "feat(engine): eight vertices and a rest curl per bristle"`.

---

### Task 4: Dab record, handle and rig — offsets and persistent strokes

**Files:**
- Modify: `crates/cadmium-engine/src/engine/dab_record.rs:54-63`
- Modify: `crates/cadmium-engine/src/shaders/brush_tip.wgsl:40-50`
- Modify: `crates/cadmium-engine/src/engine/bristles/handle.rs` (+ `handle/tests.rs`)
- Modify: `crates/cadmium-engine/src/engine/bristles/rig.rs` (+ `rig/tests.rs`)

Field changes, made the same way in both `DabRecord` and the WGSL `Dab`:

| Old | New | Meaning |
|---|---|---|
| `handle_height_px` | `handle_offset_px` | Ferrule height over the touch height: −p·press_depth·L pressed, +L lifted |
| `handle_rest_height_px` | (kept) | Where the hanging tips reach the canvas: the pose that places the rest shape when the GPU measures the touch height |
| `bristle_stiffness` | `spare_0` | Unused until phase 4 |
| `bristle_damping` | `spare_1` | Unused until phase 4 |

- [ ] **Step 1: Write the failing tests.**
  - Replace `handle/tests.rs::the_first_dab_of_a_stroke_takes_no_time` with:

```rust
#[test]
fn the_clock_keeps_running_between_strokes() {
    let mut clock = HandleClock::default();
    assert_eq!(clock.elapsed_seconds(120.0), 0.0);
    assert_eq!(clock.elapsed_seconds(130.0), 0.010);
    assert_eq!(
        clock.elapsed_seconds(900.0),
        (MAX_STEP_MS / MILLISECONDS_PER_SECOND) as f32
    );
}
```

  - Replace `a_leaning_brush_presses_from_where_its_hanging_tips_reach` with:

```rust
#[test]
fn pressing_offsets_the_ferrule_by_the_press_depth_of_the_whole_length() {
    assert_eq!(gauge().offset_px(0.0), 0.0);
    assert_eq!(gauge().offset_px(1.0), -0.5 * 40.0);
    assert_eq!(gauge().offset_px(2.0), -0.5 * 40.0);
    let altitude = std::f32::consts::FRAC_PI_3;
    assert!((gauge().hanging_px(altitude) - 40.0 * altitude.sin()).abs() < 1e-4);
}
```

  - In `rig/tests.rs`:
    - rename `handle_height_px` to `handle_offset_px`;
    - change `once_lifted_a_settle_raises_the_handle_by_one_bristle_length` to assert `settled.handle_offset_px == REACH_PX`;
    - add:

```rust
#[test]
fn a_new_stroke_keeps_the_bristles_the_last_one_left() {
    let (mut rig, first) = rig_after_one_pressed_dab();
    assert_eq!(first.bristle_reset, 1, "the first dab after taking the brush up places it at rest");
    rig.lift();
    rig.begin_stroke();
    let mut next: DabRecord = bytemuck::Zeroable::zeroed();
    rig.advance(&mut next, &pressed_dab(), &SimParams::default());
    assert_eq!(next.bristle_reset, 0);
}
```

- [ ] **Step 2: Run them and expect failures.**

  Run `$DOCKER cargo test --release -p cadmium-engine --lib handle rig`. It does not compile yet: `offset_px` and `handle_offset_px` are undefined.

- [ ] **Step 3: Implement the change.**
  - In `handle.rs`:
    - Remove `HandleClock::begin_stroke`.
    - Add to `HandleGauge`:

```rust
    /// How far the ferrule stands from the height at which the brush's rest shape just
    /// touches: p·press_depth·L below it (DiVerdi, US 8,760,438).
    pub(super) fn offset_px(&self, pressure: f32) -> f32 {
        -pressure.clamp(0.0, 1.0) * self.press_depth * self.reach_px
    }

    /// Height of the ferrule when the hanging tips reach a flat canvas.
    pub(super) fn hanging_px(&self, altitude: f32) -> f32 {
        self.reach_px * altitude.sin().clamp(0.0, 1.0)
    }
```

    - Rewrite `height_px` as `(self.hanging_px(altitude) + self.offset_px(pressure)).max(ferrule_clearance_px)`. It now only estimates the travel the substeps follow.
    - Keep the rest of `handle.rs` as it is.
  - In `rig.rs`:
    - `begin_stroke` keeps only `self.pressed = true; self.lift_pending = false;`. The doc comment becomes "A stroke starts from the bristles as the last one left them."
    - `advance`:
      - sets `record.handle_offset_px = gauge.offset_px(dab.pressure)`;
      - sets `record.handle_rest_height_px = gauge.hanging_px(dab.altitude)`;
      - drops the `bristle_stiffness` and `bristle_damping` lines.
    - `settle` replaces `record.handle_height_px += self.reach_px` with `record.handle_offset_px = self.reach_px`, and computes the travel from `ferrule_over_nib(record.center, record.handle_rest_height_px + record.handle_offset_px, record.handle_axis)`.
  - In `dab_record.rs` and `brush_tip.wgsl`, apply the renames in the table.
  - In the old `bristle_step.wgsl`/`bristle_relief.wgsl`/`bristle_point.wgsl`, rename the reads in the same way, so the tree keeps compiling until Task 6 replaces them:
    - `dab.handle_height_px` becomes `(dab.handle_rest_height_px + dab.handle_offset_px)`;
    - `dab.bristle_stiffness` becomes `0.5`.
- [ ] **Step 4: Run the tests and expect them to pass.**

  Run `$DOCKER cargo test --release -p cadmium-engine --lib`. The `dab_record` mirror test, the handle tests and the rig tests should pass.

- [ ] **Step 5: Commit** with `git commit -m "feat(engine): press depth as a handle offset and strokes that keep the bristles"`.

---

### Task 5: Tuning — compliances replace the old knobs

**Files:**
- Rewrite: `crates/cadmium-engine/src/engine/bristles/tuning.rs`, `tuning/tests.rs`
- Rewrite: `crates/cadmium-engine/src/shaders/bristle_tuning.wgsl`
- Modify: `crates/cadmium-engine/src/brush/head.rs` (remove `point_axes`, `ROUND_POINT_ONSET`, `FAN_POINT_FADE_DEG`), `brush/head/tests.rs` (drop the two `point_axes` tests)

- [ ] **Step 1: Rewrite `tuning/tests.rs`:**

```rust
use super::{BRISTLE_TUNING_BYTES, BristleTuning};
use crate::brush::HairSpec;
use crate::engine::bristles::material::{HairMaterial, hinge_compliances_per_m};
use crate::params::SimParams;

const SCALAR_BYTES: u64 = 4;
const VEC4_BYTES: u64 = 16;

fn wgsl_tuning_struct_bytes() -> u64 {
    let source = include_str!("../../../shaders/bristle_tuning.wgsl");
    let body = source
        .split("struct BristleTuning {")
        .nth(1)
        .and_then(|rest| rest.split("};").next())
        .expect("bristle_tuning.wgsl declares struct BristleTuning");
    let scalars = body.lines().filter(|line| line.contains(": f32")).count() as u64;
    let vectors = body.lines().filter(|line| line.contains("array<vec4<f32>, 2>")).count() as u64;
    scalars * SCALAR_BYTES + vectors * 2 * VEC4_BYTES
}

#[test]
fn the_shader_tuning_struct_mirrors_the_bristle_tuning() {
    assert_eq!(wgsl_tuning_struct_bytes(), BRISTLE_TUNING_BYTES);
}

#[test]
fn the_tuning_carries_the_hairs_compliances() {
    let tuning = BristleTuning::of(&SimParams::default(), &HairSpec::SABLE, 3.0, 20.0, 64);
    let want = hinge_compliances_per_m(&HairMaterial::of(&HairSpec::SABLE), 20.0, 64);
    assert_eq!(&tuning.hinge_compliance_per_m[..want.len()], &want);
    assert_eq!(tuning.hinge_compliance_per_m[want.len()..], [0.0]);
    assert_eq!(tuning.canvas_tooth_um, 3.0);
    assert_eq!(tuning.wet_support, 0.0);
}
```

- [ ] **Step 2: Run them and expect a compile failure.**
- [ ] **Step 3: Implement the change.**
  - Rewrite `tuning.rs`:

```rust
use super::material::{HINGES, HairMaterial, hinge_compliances_per_m};
use crate::brush::HairSpec;
use crate::params::SimParams;

pub(super) const BRISTLE_TUNING_BYTES: u64 = std::mem::size_of::<BristleTuning>() as u64;
const UNIFORM_ALIGNMENT_BYTES: u64 = 16;
/// Two vec4 in the shader hold the hinge compliances.
const COMPLIANCE_SLOTS: usize = 8;
const _: () = assert!(BRISTLE_TUNING_BYTES.is_multiple_of(UNIFORM_ALIGNMENT_BYTES));
const _: () = assert!(HINGES <= COMPLIANCE_SLOTS);

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BristleTuning {
    pub(crate) wet_support: f32,
    pub(crate) canvas_tooth_um: f32,
    pub(crate) spare: [f32; 2],
    /// Bending compliance per metre of segment, per hinge, 1/(N·m²).
    pub(crate) hinge_compliance_per_m: [f32; COMPLIANCE_SLOTS],
}

impl BristleTuning {
    pub(crate) fn of(
        params: &SimParams,
        hair: &HairSpec,
        canvas_tooth_um: f32,
        ferrule_radius_px: f32,
        strands: usize,
    ) -> BristleTuning {
        let hinges = hinge_compliances_per_m(&HairMaterial::of(hair), ferrule_radius_px, strands);
        let mut hinge_compliance_per_m = [0.0; COMPLIANCE_SLOTS];
        hinge_compliance_per_m[..HINGES].copy_from_slice(&hinges);
        BristleTuning {
            wet_support: params.get("bristle_wet_support"),
            canvas_tooth_um,
            spare: [0.0; 2],
            hinge_compliance_per_m,
        }
    }
}

#[cfg(test)]
mod tests;
```

  - Rewrite `bristle_tuning.wgsl`:

```wgsl
struct BristleTuning {
    wet_support: f32,
    canvas_tooth_um: f32,
    spare_0: f32,
    spare_1: f32,
    hinge_compliance_per_m: array<vec4<f32>, 2>,
};

fn load_bearing_um(dry_um: f32, wet_um: f32) -> f32 {
    return max(dry_um + tuning.wet_support * wet_um - tuning.canvas_tooth_um, 0.0);
}

fn hinge_compliance_per_m(hinge: u32) -> f32 {
    return tuning.hinge_compliance_per_m[hinge / 4u][hinge % 4u];
}
```

  - In `rig.rs`, add `pub(crate) fn tuning(&self, params: &SimParams, canvas_tooth_um: f32) -> BristleTuning` returning `BristleTuning::of(params, &self.hair, canvas_tooth_um, self.ferrule_radius_px, self.count as usize)`.
  - In `engine.rs`:
    - `fit_bristles` calls `self.bristles.tuning(&self.params, canvas.tooth_height_um())` after `fit`;
    - `settle_bristles` does the same, without `head()`/`hair()`;
    - remove `BristleRig::head` and `BristleRig::hair` if nothing else uses them.
  - Remove `HeadSpec::point_axes`, its two constants and its two tests in `head/tests.rs`.
  - The old step's shaders still read friction and point fields that no longer exist. Task 6 deletes them in the same commit range, so do Tasks 5 and 6 back to back: run the build only after Task 6, Step 3.
- [ ] **Step 4: Commit together with Task 6** (see Task 6, Step 7).

---

### Task 6: The new bristle step

**Files:**
- Create: `crates/cadmium-engine/src/shaders/bristle_shape.wgsl`
- Rewrite: `crates/cadmium-engine/src/shaders/bristle_step.wgsl`
- Modify: `crates/cadmium-engine/src/shaders/bristle_frame.wgsl`, `bristle.wgsl`, `bristle_relief.wgsl`
- Delete: `crates/cadmium-engine/src/shaders/bristle_friction.wgsl`, `bristle_memory.wgsl`, `bristle_point.wgsl`, `bristle_clump.wgsl`
- Modify: `crates/cadmium-engine/src/engine/shader.rs`, `engine/pass/bristle_step.rs`, `engine/bristles/state.rs`, `engine/bristles.rs`

- [ ] **Step 1: Rewrite `bristle_frame.wgsl`.** Keep `Frame`, `Pose`, `frame_of`, `tilted`, `untilted`, `spun` and `unspun` unchanged. Replace the rest with:

```wgsl
struct HandleState {
    ferrule: vec3<f32>,
    spare_0: f32,
    axis: vec3<f32>,
    spare_1: f32,
    orientation: vec2<f32>,
    spare_2: vec2<f32>,
};

fn pose_of(state: HandleState) -> Pose {
    return Pose(state.ferrule, state.axis, state.orientation);
}

fn state_slot(bristle: u32) -> u32 {
    return bristle * BRISTLE_STATE_VECTORS;
}

/// A vertex's position in `xyz` and the floor under it in `w`.
fn position_slot(bristle: u32, vertex: u32) -> u32 {
    return state_slot(bristle) + vertex;
}

fn to_ferrule(world: vec3<f32>, pose: Pose) -> vec3<f32> {
    let local = untilted(world - pose.ferrule, pose.axis);
    return vec3<f32>(unspun(local.xy, pose.orientation), local.z);
}

fn from_ferrule(local: vec3<f32>, pose: Pose) -> vec3<f32> {
    return pose.ferrule + direction_at(local, pose);
}

fn direction_at(local: vec3<f32>, pose: Pose) -> vec3<f32> {
    return tilted(vec3<f32>(spun(local.xy, pose.orientation), local.z), pose.axis);
}

/// `world` moved rigidly with the ferrule from pose `from` to pose `to`.
fn carried(world: vec3<f32>, from: Pose, to: Pose) -> vec3<f32> {
    return from_ferrule(to_ferrule(world, from), to);
}
```

- [ ] **Step 2: Trim `bristle.wgsl`** down to `segment_length`, `base_lean`, `turned` and `pose_between`. Delete `root_position` and `hanging_along`, which only the old step used. Contact uses none of these; it needs only `position_slot` and `BristleRest`.

- [ ] **Step 3: Write `bristle_shape.wgsl`** and the new `bristle_step.wgsl`, and trim the relief.

  `bristle_shape.wgsl`:

```wgsl
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
    );
}
```

  `bristle_relief.wgsl`:
  - keep `PIXEL_CENTRE_PX`, `surface_um_at`, `surface_px`, `floors_under`, `NO_FLOOR_PX`, `partials`, `upper_partials` and `workgroup_range`;
  - delete the `Floors` alias (it now lives in `bristle_solve.wgsl`), `BASE_BISECTIONS`, `BISECTIONS_PER_ROUND`, `FLAT_HAIR_LOAD`, `SupportFloors`, `level_partials`, `LevelRound`, `workgroup_sums`, `round_levels`, `bisected`, `owned_support_floors`, `floor_range`, `penetration_px`, `resting_base_px`, `handle_press_px` and `hair_load`.

  `bristle_step.wgsl`:

```wgsl
@group(0) @binding(0) var<uniform> dab: Dab;
@group(0) @binding(1) var<storage, read> rests: array<BristleRest>;
@group(0) @binding(2) var<storage, read_write> states: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> handle: HandleState;
@group(0) @binding(4) var<uniform> tuning: BristleTuning;
@group(0) @binding(5) var dry_height: texture_storage_2d<r32float, read>;
@group(0) @binding(6) var wet_vol: texture_storage_2d<r32float, read>;
@group(0) @binding(7) var<storage, read_write> hair_loads: array<f32>;
@group(0) @binding(8) var<storage, read_write> normal_forces: array<f32>;

// Mirrors MIN_AXIS_RISE in engine/bristles/handle.rs.
const MIN_AXIS_RISE: f32 = 0.0175;
/// Fewest solve iterations a dab runs, so a dab with no travel still settles its strands.
const MIN_DAB_ITERATIONS: u32 = 2u;
/// Every hair carries the typical load until phase 4 of #62 hands Exchange the normal force.
const FLAT_HAIR_LOAD: f32 = 1.0;

fn handle_axis() -> vec3<f32> {
    return vec3<f32>(dab.handle_axis_x, dab.handle_axis_y, dab.handle_axis_z);
}

fn pose_at_height(height_px: f32) -> Pose {
    let axis = handle_axis();
    let over_nib = dab.center + axis.xy * (height_px / max(axis.z, MIN_AXIS_RISE));
    return Pose(vec3<f32>(over_nib, height_px), axis, dab.bristle_orientation);
}

fn owned_bristle(thread: u32, share: u32) -> u32 {
    return thread + share * BRISTLE_WORKGROUP_SIZE;
}

/// How high the ferrule must stand for this strand's rest shape to just touch: the floor
/// under each rest vertex, less that vertex's depth below the ferrule.
fn strand_touch_px(bristle: u32, placed: Pose) -> f32 {
    let local = rest_chain_local(rests[bristle]);
    var touch = -NO_FLOOR_PX;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let at = from_ferrule(local[vertex], placed);
        touch = max(touch, surface_px(at.xy) - (at.z - placed.ferrule.z));
    }
    return touch;
}

/// The height at which the brush's rest shape, in the dab's pose, first touches
/// (spec §3): p = 0 lands only the lowest tip, whatever the hairs are doing now.
fn touch_height_px(thread: u32) -> f32 {
    let placed = pose_at_height(dab.handle_rest_height_px);
    var touch = -NO_FLOOR_PX;
    for (var share = 0u; share < BRISTLES_PER_THREAD; share++) {
        let bristle = owned_bristle(thread, share);
        if bristle < dab.bristle_total {
            touch = max(touch, strand_touch_px(bristle, placed));
        }
    }
    let bounds = workgroup_range(thread, vec2<f32>(NO_FLOOR_PX, touch));
    return select(bounds.y, 0.0, bounds.y <= -NO_FLOOR_PX);
}

fn ferrule_clearance_px() -> f32 {
    return dab.ferrule_radius_px * length(handle_axis().xy);
}

fn held_pose(offset_px: f32, touch_px: f32) -> Pose {
    return pose_at_height(max(touch_px + offset_px, ferrule_clearance_px()));
}

/// Where the dab starts: the last dab's pose, or, after a reset, just touching.
fn start_pose(touch_px: f32) -> Pose {
    if dab.bristle_reset != 0u {
        return held_pose(max(dab.handle_offset_px, 0.0), touch_px);
    }
    return pose_of(handle);
}

fn place_at_rest(bristle: u32, pose: Pose) {
    let local = rest_chain_local(rests[bristle]);
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        let at = from_ferrule(local[vertex], pose);
        states[position_slot(bristle, vertex)] = vec4<f32>(at, surface_px(at.xy));
    }
}

fn held_chain(bristle: u32) -> Chain {
    var chain: Chain;
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        chain[vertex] = states[position_slot(bristle, vertex)].xyz;
    }
    return chain;
}

fn store_chain(bristle: u32, chain: Chain) {
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        let at = chain[vertex];
        states[position_slot(bristle, vertex)] = vec4<f32>(at, surface_px(at.xy));
    }
}

fn carry(chain: ptr<function, Chain>, from: Pose, to: Pose) {
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        (*chain)[vertex] = carried((*chain)[vertex], from, to);
    }
}

/// Moves the strand along the dab's ferrule path, one solve iteration per substep, and
/// returns the canvas's normal force at the end, N.
fn solved_dab(bristle: u32, start: Pose, end: Pose) -> f32 {
    var chain = held_chain(bristle);
    let substeps = max(dab.substeps, 1u);
    let iterations = max(substeps, MIN_DAB_ITERATIONS);
    var previous = start;
    var force = 0.0;
    for (var iteration = 0u; iteration < iterations; iteration++) {
        let pose = pose_between(start, end, min(f32(iteration + 1u) / f32(substeps), 1.0));
        carry(&chain, previous, pose);
        previous = pose;
        force = solve_iteration(&chain, strand_shape(rests[bristle], pose), floors_under(chain));
    }
    store_chain(bristle, chain);
    return force;
}

@compute @workgroup_size(BRISTLE_WORKGROUP_SIZE)
fn step(@builtin(local_invocation_index) thread: u32) {
    let touch_px = touch_height_px(thread);
    let start = start_pose(touch_px);
    let end = held_pose(dab.handle_offset_px, touch_px);
    for (var share = 0u; share < BRISTLES_PER_THREAD; share++) {
        let bristle = owned_bristle(thread, share);
        if bristle < dab.bristle_total {
            if dab.bristle_reset != 0u {
                place_at_rest(bristle, start);
            }
            normal_forces[bristle] = solved_dab(bristle, start, end);
            hair_loads[bristle] = FLAT_HAIR_LOAD;
        }
    }
    if thread == 0u {
        handle = HandleState(end.ferrule, 0.0, end.axis, 0.0, end.orientation, vec2<f32>(0.0));
    }
}
```

- [ ] **Step 4: Wire the snippets, the pass and the buffers.**
  - `engine/shader.rs`:
    - `BRISTLE_SNIPPETS` becomes `[brush_tip, bristle_rest, bristle_frame, bristle, bristle_tuning, bristle_relief, bristle_solve, bristle_contacts, bristle_shape]` (length 9);
    - remove these constants from `bristle_constants()` and the `use` list: `BRISTLE_SOLVER_ITERATIONS`, `BRISTLE_TIP_STIFFNESS_SHARE`, `PLASTIC_BEND_THRESHOLD_RAD`, `MAX_PLASTIC_LEAN`, `MIN_LEAN_DROP`, `LANE_SAMPLES`, `CLUSTER_CENTROIDS`, `MAX_CLUSTERS`, `CLUSTER_SPAN_WORDS`.

    Keep `MIN_SEGMENT_SPAN_PX` and `TOUCH_TOLERANCE_PX` if Contact or the snapshot still uses them. Grep for each before removing it.
  - `engine/pass/bristle_step.rs`: the layout entries become `[dab_record_entry(), storage_entry(1, true), storage_entry(2, false), storage_entry(3, false), uniform_entry(4, None), texture_entry(5, …), texture_entry(6, …), storage_entry(7, false), storage_entry(8, false)]`. The bindings match: rests, states, handle, tuning, dry height, wet vol, hair_loads, normal_forces. Drop the `tip` and `clusters` bindings.
  - `engine/bristles/state.rs`:
    - set `BRISTLE_STATE_VECTORS = BRISTLE_VERTICES`;
    - delete `BEND_VECTOR` and every constant the shader list above dropped;
    - add `normal_forces: storage_buffer(gpu, "bristle_normal_forces", HAIR_LOAD_BYTES)` to `BristleBuffers`.
    - In `BristleSnapshot`:
      - keep `rest_bends`, documented as "each bristle's plastic lean; [0, 0] until phase 2 brings plasticity back", and fill it with zeros;
      - add `pub normal_forces_n: Vec<f32>` and `pub clearances_px: Vec<Vec<f32>>`, the latter holding, per bristle and vertex, `z − floor`;
      - `bristle_snapshot` reads `normal_forces` too.
  - `engine/bristles.rs`: update the `pub(crate) use` list to match, and add `mod material;` and `#[cfg(test)] mod lab;`.
  - Delete the four old shaders: `git rm crates/cadmium-engine/src/shaders/bristle_{friction,memory,point,clump}.wgsl`.
- [ ] **Step 5: Build and lint.**

  ```bash
  $DOCKER cargo clippy --workspace --all-targets --release -- -D warnings
  .github/check-file-sizes.sh
  ```

  Both should pass. Fix the dead code clippy reports in the same files: a constant or a field that only the old step used is removed, not allowed.
- [ ] **Step 6: Run the unit tests and the lab tests.**

  Run `$DOCKER cargo test --release -p cadmium-engine --lib`. They should pass.
- [ ] **Step 7: Commit** with `git commit -m "feat(engine): quasi-static bristle step replaces the positional relaxations"`. This commit includes Task 5.

---

### Task 7: Invariants and the existing bristle tests

**Files:**
- Modify: `crates/cadmium-engine/tests/bristles/invariants.rs`
- Modify: the files of the bristle tests that need a later phase (see Step 4)

- [ ] **Step 1: Add the phase 1 invariants** to `invariants.rs`.
  - Remove the `#[ignore]` from `a_light_first_touch_lands_the_wet_point_instead_of_splaying_the_hairs`.
  - Add:

```rust
/// One dab at p = 0 on a fresh brush: the rest shape just touches.
fn zero_pressure_touch(gpu: &GpuContext) -> BristleSnapshot {
    let mut s = session(gpu);
    s.limit_dabs_per_frame(Some(1));
    s.begin_stroke(StrokeSample { pressure: 0.0, ..light_touch_sample(0) });
    frame(gpu, &mut s);
    s.engine.bristle_snapshot(gpu).unwrap()
}

#[test]
fn at_zero_pressure_only_tips_touch() {
    let gpu = GpuContext::new().unwrap();
    let snapshot = zero_pressure_touch(&gpu);
    assert!(snapshot.touching.iter().any(|&t| t), "nothing touches at p = 0");
    for (bristle, clearances) in snapshot.clearances_px.iter().enumerate() {
        let (_tip, above) = clearances.split_last().unwrap();
        assert!(
            above.iter().all(|&c| c > TOUCH_TOLERANCE_PX),
            "bristle {bristle} touches above its tip: {clearances:?}"
        );
    }
}

#[test]
fn the_splay_does_not_depend_on_how_many_hairs_are_simulated() {
    let gpu = GpuContext::new().unwrap();
    let splay = |scale: f32| {
        let mut s = session(&gpu);
        s.brush.head.density_per_px *= scale;
        s.reload_brush(&gpu);
        press(&gpu, &mut s, &line_at_rate(STEADY_PRESSURE, COARSE_RATE_HZ));
        mean_splay_px(&s.engine.bristle_snapshot(&gpu).unwrap())
    };
    let (base, denser) = (splay(1.0), splay(DENSITY_SCALE));
    assert!(
        relative_gap(f64::from(base), f64::from(denser)) <= MAX_RELATIVE_GAP,
        "splay {base} px, {denser} px with {DENSITY_SCALE}x hair density"
    );
}
```

  `TOUCH_TOLERANCE_PX` is a local `const TOUCH_TOLERANCE_PX: f32 = 0.01;` and mirrors the engine's.
- [ ] **Step 2: Run the whole bristle suite.**

  Run `$DOCKER cargo test --release -p cadmium-engine --test bristles 2>&1 | tee target/phase1-bristles.txt`. Keep the list of failures.
- [ ] **Step 3: Fix every failure that is a defect of the new step**, before you ignore anything. These count as defects:
  - a vertex below the canvas;
  - non-finite values;
  - non-determinism;
  - no mark where the brush is pressed;
  - a reset that does not happen after a shape change, a clean or a tool change;
  - paint that is not conserved.

  For each one, use superpowers:systematic-debugging. If the root cause is a phase 1 mechanism, fix the step and add a lab or unit test that pins it.
- [ ] **Step 4: Triage the rest.**
  - **Needs friction or plasticity → `#[ignore = "phase 2 of #62: …"]`.** The expected ones:
    - in `friction.rs`: the trail, the turn lag, staying bent after a hard stroke, the pulled-tip anisotropy, and holding a pressed brush still;
    - in `look.rs`: the S-curve inside/outside and the wavering edges, if they fail;
    - in `relief.rs`: a light touch carried by ridge tops, if it fails.
  - **Needs clumping or the wet point → `#[ignore = "phase 3 of #62: …"]`.** The expected ones:
    - `clumping.rs`, both tests;
    - `threads.rs`, both tests;
    - `dynamics.rs::a_wet_round_brush_hangs_to_a_point_and_a_dry_one_hangs_straight`;
    - `relief.rs::hairs_pulled_into_clumps_across_a_dried_edge_stay_on_top_of_it`;
    - `look.rs::a_stroke_running_dry_ends_in_threads_of_different_lengths`;
    - `sizes.rs::the_largest_brush_pressed_without_clumping_leaves_no_holes`, if its holes come from the missing spacing.
  - **Pins a removed mechanism → delete it**, as the spec's "Existing tests" says. Name each one in the commit body. The expected ones:
    - `settle.rs::each_stroke_starts_from_a_brush_at_rest`, which pins the per-stroke reset;
    - `friction.rs::a_wet_brush_never_sets_its_point_into_its_bend_memory`, which pins the wet point in the bend memory.

  Every ignore names the mechanism. Example:
  `#[ignore = "phase 2 of #62: tips trail only once static friction holds them"]`.
- [ ] **Step 5: Run the whole engine suite.**

  Run `$DOCKER cargo test --release -p cadmium-engine`. Everything passes except the tests ignored above.
- [ ] **Step 6: Commit** with `git commit -m "test(engine): phase 1 bristle invariants; park the tests of later phases"`, and list the deleted tests in the body.

---

### Task 8: Spec, cost gate and sheets

**Files:**
- Modify: `docs/superpowers/specs/2026-11-16-xpbd-bristles-design.md` (section 5 table)

- [ ] **Step 1: Move head stiffness to phase 2.** The table row becomes:

  `| Head stiffness | Baxter & Lin | 2 | The whole head bends about 90° under 1–2 N, with the tip held by static friction. |`

  Add to the phase 1 bullets: "Head stiffness waits for friction: without it the pressed tip slides, so Baxter & Lin's 90° under 1–2 N does not apply (the prototype reached 90° only with the root at the floor, at 0.2–0.9 N depending on the [U] diameter)." Commit with `docs: head stiffness needs friction, so it moves to phase 2`.
- [ ] **Step 2: Run the full cost gate** (no filter; it takes a few minutes):

  ```bash
  $DOCKER env CADMIUM_BENCH_BASELINE=docs/superpowers/baselines/2026-11-16-bristle-cost.tsv \
    cargo run --release -p cadmium-engine --example bristle_cost
  ```

  It must pass: exit 0, every scenario within 10% or 0.05 ms of the baseline.
- [ ] **Step 3: If the gate fails, measure before changing anything.**
  - Rerun the failing scenario with `CADMIUM_BENCH_REPEATS=1` and read the timestamps.
  - The expected suspect is Contact's rasterise dispatch, which grows from 5 to 7 segments per strand.
  - The first lever is to stop enqueueing workgroups for segments that cannot touch. Only after that comes trimming the step (for example, building `contact_matrix` for the active rows only).
  - Commit each optimisation on its own, with the before and after numbers in the body.
- [ ] **Step 4: Render the sheets** with the commands in `docs/superpowers/baselines/README.md`, from the `xpbd-baseline` worktree and from this branch. Then open `target/brush_sheets/page/index.html`. The sheets are not committed. Note which brushes changed most, for the PR comment.

---

### Task 9: Pull request

- [ ] **Step 1:** `$DOCKER cargo fmt --all -- --check`, clippy (Task 6, Step 5), `.github/check-file-sizes.sh`, `$DOCKER cargo test --release --workspace`. All of them should pass.
- [ ] **Step 2:** Push, and open the PR against `feat/xpbd-phase0`. The description is short:

  ```
  Replaces the bristle step with the quasi-static per-strand solve: hinge coordinates, bending from E·I, frictionless contact through an active set, pressure as handle height over the rest shape's touch, and bristles that persist across strokes.

  Related to #62
  ```

- [ ] **Step 3:** Ask the user before posting a comment on #62. If they agree, the comment holds:
  - why XPBD was replaced (the numbers of the spec's section 2);
  - the gate result;
  - the sheet observations;
  - the list of tests parked for phases 2 and 3.
