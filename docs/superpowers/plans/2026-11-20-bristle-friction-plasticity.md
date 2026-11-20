# Bristle friction and plasticity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Coulomb canvas friction (static, kinetic, anisotropic) and time-creeping plastic rest curvature to the quasi-static bristle solve, in both the serial and the grouped solve.

**Architecture:** The contact active set grows from one normal row per vertex to three rows per vertex (normal, two tangents). Stuck contacts keep their tangent rows, which hold the vertex on its anchor; sliding contacts drop them and take a kinetic force, lagged one iteration, against the slip. A plastic shift per hinge is added to the rest turn and creeps once per dab. Anchors and shifts persist in `states`. The serial solve (`solve_reference`) is written first and tested in the GPU lab; the grouped solve then mirrors it term for term, held to it by the equivalence test.

**Tech Stack:** Rust, wgpu, WGSL. Spec: `docs/superpowers/specs/2026-11-20-bristle-friction-plasticity-design.md`.

---

## Conventions for every task

**Worktree:** `/home/marcelo/Desenvolvimento/cadmium-friction`, branch `feat/bristle-friction`. All paths below are relative to `crates/cadmium-engine/` unless they start with `docs/` or `.github/`.

**Cargo runs only in Docker** (the host lacks the GTK libraries). Run the commands with the sandbox disabled, from the worktree root, through this zsh function. Define it in the same command each time:

```bash
dk() { local ga=(); for g in $(stat -c '%g' /dev/dri/* | sort -u); do ga+=(--group-add $g); done; docker run --rm --device /dev/dri "${ga[@]}" --security-opt label=disable -u $(id -u):$(id -g) -e HOME=/tmp -e CARGO_HOME=$HOME/.cargo -v $HOME/.cargo:$HOME/.cargo -v $PWD:$PWD -w $PWD cadmium-dev sh -c "$*"; }
cd /home/marcelo/Desenvolvimento/cadmium-friction && dk 'cargo test --release -p cadmium-engine --lib bristles::lab 2>&1 | tail -30'
```

**Before each commit:** `dk 'cargo fmt --all && cargo clippy --workspace --all-targets --release -- -D warnings'` and `.github/check-file-sizes.sh` (every `.rs` and `.wgsl` file at most 400 lines). Split a file rather than exceed the limit.

**Comments:** only what the code does not say. No doc comment that restates the name; no usage contracts. Physics sources and units are worth a comment.

**Commits:** conventional-commit subject, no body unless it adds what the diff does not say, **no attribution trailers of any kind**.

**Shaders:** WGSL files are concatenated by `shader_source` (`src/engine/shader.rs`); declaration order does not matter. `BRISTLE_SNIPPETS` feeds the step pass; `LAB_SNIPPETS` feeds the lab. A new file must be added to both lists when both use it.

**Floating-point order:** the grouped solve must accumulate every sum in the same order as the serial one; the equivalence test (`tests/bristles/parallel_solve.rs`) allows 1e-3 px and 1e-3 of the force. With friction off (μ = 0) the serial solve must give the same results as before this plan: the phase 1 lab tests are the check.

---

## File structure

| file | responsibility |
|---|---|
| `src/shaders/bristle_contacts.wgsl` (rewrite) | rows (normal + 2 tangents per vertex), gaps, contact matrix, Cholesky, normal active set |
| `src/shaders/bristle_friction.wgsl` (new) | `Friction`, `Contacts`, kinetic forces, the static cone, anisotropy, anchors, `contact_forces` |
| `src/shaders/bristle_plastic.wgsl` (new) | yield, creep, cap: `crept`, `hinge_excess`, `creep_share` |
| `src/shaders/bristle_solve.wgsl` | `StrandShape` gains `friction` and `plastic`; `free_turns` adds the shift; `pushed_step` applies all rows and updates anchors |
| `src/shaders/bristle_frame.wgsl` | `anchor_slot`, `plastic_slot`, `direction_in_ferrule` |
| `src/shaders/bristle_step.wgsl` | serial dab: carry anchors and shifts, creep at the end, reset clears them |
| `src/shaders/bristle_tuning.wgsl`, `src/engine/bristles/tuning.rs` | `friction: vec4` = μₛ, μₖ, anisotropy, creep per second |
| `src/shaders/bristle_shape.wgsl` | `strand_shape` fills `friction` from tuning |
| `src/engine/bristles/state.rs` | state layout, snapshot `anchors`, `plastic`, `rest_bends` |
| `src/shaders/bristle_lab.wgsl`, `src/engine/bristles/lab.rs` | lab strand: friction, drag phase, timing; output friction forces, anchors, shifts |
| `src/engine/bristles/lab/tests/friction.rs`, `.../tests/plasticity.rs` (new) | lab tests against closed forms |
| `src/shaders/bristle_strand_groups.wgsl`, `src/shaders/bristle_group_contacts.wgsl`, `src/shaders/bristle_group_friction.wgsl` (new) | grouped mirror |
| `tests/bristles/parallel_solve.rs` | equivalence of anchors and shifts |
| `tests/bristles/*.rs`, `tests/engine_shapes.rs` | the 8 phase 2 ignores removed |

---

### Task 1: State layout for anchors and plastic shifts

**Files:**
- Modify: `src/engine/bristles/state.rs`
- Modify: `src/shaders/bristle_frame.wgsl`
- Modify: `src/shaders/bristle_step.wgsl` (`place_at_rest`)
- Modify: `src/shaders/bristle_strand_groups.wgsl` (`held_position` reset)
- Test: `src/engine/bristles/tests.rs` (or a new `src/engine/bristles/state/tests.rs` if `tests.rs` would pass 400 lines)

- [ ] **Step 1: Check how `BRISTLE_STATE_VECTORS` reaches WGSL**

Run: `grep -n 'STATE_VECTORS' src/engine/shader.rs src/shaders/*.wgsl`
Expected: `shader.rs` injects `const BRISTLE_STATE_VECTORS: u32 = …u;`. If it does not, add it to the injected constants block next to `BRISTLE_VERTICES`.

- [ ] **Step 2: Write the failing snapshot test**

In the bristle unit tests (`src/engine/bristles/tests.rs` or a new `state/tests.rs` declared with `#[cfg(test)] mod tests;` at the end of `state.rs`):

```rust
#[test]
fn a_snapshot_reads_each_strands_anchors_and_plastic_shifts() {
    let mut floats = vec![0.0; 2 * STATE_FLOATS];
    let second = STATE_FLOATS;
    let anchor = second + (ANCHOR_VECTORS_FIRST + 3) * VECTOR_FLOATS;
    floats[anchor..anchor + 4].copy_from_slice(&[1.0, 2.0, 3.0, -0.5]);
    let shift = second + (PLASTIC_VECTORS_FIRST + 2) * VECTOR_FLOATS;
    floats[shift..shift + 3].copy_from_slice(&[0.01, 0.02, 0.0]);
    let snapshot = BristleSnapshot::of(&floats, &[0.0, 0.0], &[1.0, 1.0], &[0, 0]);
    assert_eq!(snapshot.anchors[1][3], [1.0, 2.0, 3.0, -0.5]);
    assert_eq!(snapshot.anchors[0][3], [0.0; 4]);
    assert_eq!(snapshot.plastic[1][2], [0.01, 0.02, 0.0]);
    assert_eq!(snapshot.rest_bends[1], [-0.02, 0.01]);
    assert_eq!(snapshot.rest_bends[0], [0.0, 0.0]);
}
```

- [ ] **Step 3: Run it to see it fail**

Run: `dk 'cargo test --release -p cadmium-engine --lib a_snapshot_reads_each_strands_anchors 2>&1 | tail -20'`
Expected: compile error, `ANCHOR_VECTORS_FIRST` / `anchors` not found.

- [ ] **Step 4: Implement the layout**

In `state.rs`, replace the `BRISTLE_STATE_VECTORS` definition:

```rust
/// Per strand: each vertex's position and the floor under it; then each vertex's friction
/// anchor, w the normal force holding it (0 free, > 0 stuck, < 0 sliding); then each
/// hinge's plastic rest shift, a rotation vector in the ferrule frame.
pub(crate) const BRISTLE_STATE_VECTORS: usize = 3 * BRISTLE_VERTICES - 1;
pub(crate) const ANCHOR_VECTORS_FIRST: usize = BRISTLE_VERTICES;
pub(crate) const PLASTIC_VECTORS_FIRST: usize = 2 * BRISTLE_VERTICES;
const HINGES: usize = BRISTLE_VERTICES - 1;
```

Add to `BristleSnapshot`, replacing the `rest_bends` doc comment:

```rust
    /// Each strand's plastic lean: Σ over hinges of (−p.y, p.x), rad, the way the shifts
    /// lean a downward strand's tip.
    pub rest_bends: Vec<[f32; 2]>,
    /// Per strand and vertex, the friction anchor: xyz px, w the normal force holding it.
    pub anchors: Vec<Vec<[f32; 4]>>,
    /// Per strand and hinge, the plastic shift of the rest turn, ferrule frame, rad.
    pub plastic: Vec<Vec<[f32; 3]>>,
```

In `BristleSnapshot::of`, replace `rest_bends: vec![[0.0; 2]; states.len()],` with:

```rust
            rest_bends: states.iter().copied().map(rest_bend).collect(),
            anchors: states.iter().copied().map(anchors).collect(),
            plastic: states.iter().copied().map(plastic).collect(),
```

And add the readers next to `chain`:

```rust
fn anchors(state: &[f32]) -> Vec<[f32; 4]> {
    (0..BRISTLE_VERTICES)
        .map(|index| {
            let start = (ANCHOR_VECTORS_FIRST + index) * VECTOR_FLOATS;
            [state[start], state[start + 1], state[start + 2], state[start + 3]]
        })
        .collect()
}

fn plastic(state: &[f32]) -> Vec<[f32; 3]> {
    (0..HINGES)
        .map(|index| vertex(state, PLASTIC_VECTORS_FIRST + index))
        .collect()
}

fn rest_bend(state: &[f32]) -> [f32; 2] {
    plastic(state)
        .iter()
        .fold([0.0, 0.0], |[x, y], p| [x - p[1], y + p[0]])
}
```

In `bristle_frame.wgsl`, after `position_slot`:

```wgsl
/// A vertex's friction anchor in `xyz`, and in `w` the normal force holding it: 0 free,
/// > 0 stuck, < 0 sliding.
fn anchor_slot(bristle: u32, vertex: u32) -> u32 {
    return state_slot(bristle) + BRISTLE_VERTICES + vertex;
}

/// A hinge's plastic rest shift in `xyz`: a rotation vector in the ferrule frame.
fn plastic_slot(bristle: u32, hinge: u32) -> u32 {
    return state_slot(bristle) + 2u * BRISTLE_VERTICES + hinge;
}

fn direction_in_ferrule(world: vec3<f32>, pose: Pose) -> vec3<f32> {
    let local = untilted(world, pose.axis);
    return vec3<f32>(unspun(local.xy, pose.orientation), local.z);
}
```

In `bristle_step.wgsl`, `place_at_rest` also clears the new blocks:

```wgsl
fn place_at_rest(bristle: u32, pose: Pose) {
    let rest = rests[bristle];
    let local = rest_chain_local(rest);
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        let at = from_ferrule(local[vertex], pose);
        states[position_slot(bristle, vertex)] = vec4<f32>(at, disc_floor_px(at.xy, rest.radius_px));
        states[anchor_slot(bristle, vertex)] = vec4<f32>(0.0);
    }
    for (var hinge = 0u; hinge < BRISTLE_SEGMENTS; hinge++) {
        states[plastic_slot(bristle, hinge)] = vec4<f32>(0.0);
    }
}
```

In `bristle_strand_groups.wgsl`, `held_position` is the grouped reset. Each lane clears its own vertex's anchor and, if it owns a hinge, that hinge's shift. Add to `solve` right after `at = held_position(lane, start);`:

```wgsl
        if dab.bristle_reset != 0u {
            states[anchor_slot(lane.bristle, lane.vertex)] = vec4<f32>(0.0);
            if lane.vertex < BRISTLE_SEGMENTS {
                states[plastic_slot(lane.bristle, lane.vertex)] = vec4<f32>(0.0);
            }
        }
```

- [ ] **Step 5: Run the unit test and the bristle suite**

Run: `dk 'cargo test --release -p cadmium-engine --lib a_snapshot_reads_each_strands_anchors 2>&1 | tail -5'`
Expected: PASS.
Run: `dk 'cargo test --release -p cadmium-engine 2>&1 | grep -E "^test result|FAILED|panicked"'`
Expected: every result line `0 failed`, so the bigger stride broke nothing.

- [ ] **Step 6: Commit**

```bash
git add -A crates/cadmium-engine
git commit -m "feat(engine): bristle state holds friction anchors and plastic shifts"
```

---

### Task 2: Friction and creep parameters in the bristle tuning

**Files:**
- Modify: `src/engine/bristles/tuning.rs`, `src/engine/bristles/tuning/tests.rs`
- Modify: `src/shaders/bristle_tuning.wgsl`

- [ ] **Step 1: Write the failing test** (in `tuning/tests.rs`, following the existing tests there for how `BristleTuning::of` is called)

```rust
#[test]
fn the_tuning_carries_the_canvas_friction_and_the_hairs_creep() {
    let hair = HairSpec {
        drag_anisotropy: 0.4,
        bend_memory: 3.0,
        ..HairSpec::default()
    };
    let tuning = BristleTuning::of(&SimParams::default(), &hair, 0.0, 20.0, 64);
    assert_eq!(
        tuning.friction,
        [STATIC_FRICTION, KINETIC_FRICTION, 0.4, 3.0]
    );
}
```

Adapt the `SimParams` construction to what the existing tests in that file use.

- [ ] **Step 2: Run it to see it fail**

Run: `dk 'cargo test --release -p cadmium-engine --lib the_tuning_carries_the_canvas_friction 2>&1 | tail -10'`
Expected: compile error, no field `friction`.

- [ ] **Step 3: Implement**

`tuning.rs`:

```rust
/// Coulomb coefficients of hair on canvas until `Hair` arrives in phase 4 of #62: Baxter &
/// Lin give 0.3–0.5. Not measured for hair on primed canvas [U].
pub(crate) const STATIC_FRICTION: f32 = 0.5;
pub(crate) const KINETIC_FRICTION: f32 = 0.3;
```

Add a field after `hinge_compliance_per_m`:

```rust
    /// μₛ, μₖ, Baxter & Lin's anisotropy C_η, and the plastic creep rate, 1/s.
    pub(crate) friction: [f32; 4],
```

and in `of`: `friction: [STATIC_FRICTION, KINETIC_FRICTION, hair.drag_anisotropy, hair.bend_memory],`.

`bristle_tuning.wgsl`, add as the last struct member:

```wgsl
    /// μₛ, μₖ, anisotropy C_η, creep per second.
    friction: vec4<f32>,
```

The struct stays a multiple of 16 bytes (64); the existing `const _` assertion checks it.

- [ ] **Step 4: Run the tuning tests**

Run: `dk 'cargo test --release -p cadmium-engine --lib tuning 2>&1 | tail -10'`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A crates/cadmium-engine
git commit -m "feat(engine): bristle tuning carries canvas friction and creep"
```

---

### Task 3: Lab strands with friction, a drag phase and timing

The lab today presses one strand onto a plane or a step and reports its chain and normal forces. It gains: friction coefficients and creep; a second phase that drags the whole strand by a total xy in N steps (optionally dropping the tip load); a dt per step for creep; outputs of the friction force and anchor mode per vertex and the plastic shift and excess per hinge. With μ = 0 and no drag, every phase 1 lab test must give the same numbers.

**Files:**
- Modify: `src/engine/bristles/lab.rs`
- Modify: `src/shaders/bristle_lab.wgsl`
- Modify: `src/engine/shader.rs` (`LAB_SNIPPETS`: add `bristle_friction.wgsl`, `bristle_plastic.wgsl`; the arrays grow from 3 to 5)

This task only adds the plumbing: the shader keeps calling the frictionless solve until Task 4 replaces it. To keep the lab compiling before Task 4 and Task 5 exist, create `bristle_friction.wgsl` and `bristle_plastic.wgsl` now as empty files (one comment line each) and add them to `LAB_SNIPPETS` and `BRISTLE_SNIPPETS`.

- [ ] **Step 1: Extend `LabStrand` (Rust) and its builders**

Add three fields after `step`, before `compliances`, with the same order in WGSL:

```rust
    /// μₛ, μₖ, anisotropy C_η, creep per second.
    friction: [f32; 4],
    /// xy: how far the drag phase moves the strand, px; z: its steps; w: 1 drops the tip
    /// load for the drag phase.
    drag: [f32; 4],
    /// x: seconds each press step lasts, y: each drag step, for the creep.
    timing: [f32; 4],
```

`new` sets all three to `[0.0; 4]`. Builders:

```rust
    pub(super) fn with_friction(mut self, static_mu: f32, kinetic_mu: f32, anisotropy: f32) -> LabStrand {
        self.friction[..3].copy_from_slice(&[static_mu, kinetic_mu, anisotropy]);
        self
    }

    /// Creeps the plastic shifts at `per_second`, each step lasting `step_seconds`.
    pub(super) fn with_creep(mut self, per_second: f32, step_seconds: f32) -> LabStrand {
        self.friction[3] = per_second;
        self.timing[0] = step_seconds;
        self.timing[1] = step_seconds;
        self
    }

    /// How long each drag step lasts, for the creep.
    pub(super) fn with_drag_seconds(mut self, step_seconds: f32) -> LabStrand {
        self.timing[1] = step_seconds;
        self
    }

    /// After the press, moves the whole strand by `drag_px` in `steps`, solving after each.
    pub(super) fn with_drag(mut self, drag_px: [f32; 2], steps: u32) -> LabStrand {
        self.drag[..2].copy_from_slice(&drag_px);
        self.drag[2] = steps as f32;
        self
    }

    /// Drops the tip load for the drag phase.
    pub(super) fn released_for_the_drag(mut self) -> LabStrand {
        self.drag[3] = 1.0;
        self
    }
```

- [ ] **Step 2: Extend `LabRun` and the output decoding**

```rust
const HINGES: usize = BRISTLE_VERTICES - 1;
/// Per strand: the vertices (xyz, normal force), the total, the friction on each vertex
/// (xyz, anchor w), and each hinge's plastic shift (xyz, excess w).
const OUT_VECTORS: usize = 2 * BRISTLE_VERTICES + 1 + HINGES;
```

```rust
pub(super) struct LabRun {
    pub(super) chain_px: Vec<[f32; 3]>,
    /// The canvas's normal force on each vertex, N; the root's is 0.
    pub(super) vertex_forces_n: Vec<f32>,
    pub(super) normal_force_n: f32,
    /// The canvas's friction on each vertex, N, world; the root's is 0.
    pub(super) friction_n: Vec<[f32; 3]>,
    /// Each vertex's anchor mode: 0 free, > 0 stuck, < 0 sliding (the normal force).
    pub(super) anchor_modes: Vec<f32>,
    /// Each hinge's plastic shift, world, rad.
    pub(super) plastic: Vec<[f32; 3]>,
    /// How far each hinge's turn stands from its rest turn plus shift, rad.
    pub(super) excess_rad: Vec<f32>,
}
```

`lab_run` reads them: friction block from vector `BRISTLE_VERTICES + 1`, `BRISTLE_VERTICES` vectors; plastic block from `2 * BRISTLE_VERTICES + 1`, `HINGES` vectors.

- [ ] **Step 3: Extend the WGSL lab**

`LabStrand` in `bristle_lab.wgsl` gains `friction: vec4<f32>, drag: vec4<f32>, timing: vec4<f32>,` after `step`. Update the header comment to describe them. `LAB_OUT_VECTORS` becomes `2u * BRISTLE_VERTICES + 1u + BRISTLE_SEGMENTS`. After the existing writes, write zeros for the new blocks (Task 4 and Task 5 fill them):

```wgsl
    let friction_first = first + BRISTLE_VERTICES + 1u;
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        lab_out[friction_first + vertex] = vec4<f32>(0.0);
    }
    let plastic_first = friction_first + BRISTLE_VERTICES;
    for (var hinge = 0u; hinge < BRISTLE_SEGMENTS; hinge++) {
        lab_out[plastic_first + hinge] = vec4<f32>(0.0);
    }
```

- [ ] **Step 4: Run the lab tests**

Run: `dk 'cargo test --release -p cadmium-engine --lib bristles::lab 2>&1 | tail -15'`
Expected: every phase 1 lab test passes unchanged.

- [ ] **Step 5: Commit**

```bash
git add -A crates/cadmium-engine
git commit -m "test(engine): lab strands carry friction, a drag phase and creep timing"
```

---

### Task 4: Friction in the serial solve

**Files:**
- Rewrite: `src/shaders/bristle_contacts.wgsl`
- Fill: `src/shaders/bristle_friction.wgsl`
- Modify: `src/shaders/bristle_solve.wgsl`, `src/shaders/bristle_shape.wgsl`, `src/shaders/bristle_step.wgsl`, `src/shaders/bristle_lab.wgsl`
- Create: `src/engine/bristles/lab/tests/friction.rs` (declare `mod friction;` at the end of `lab/tests.rs`; it can use the parent's private helpers through `use super::*;`)

The grouped solve does not change in this task; the app keeps running it, frictionless. `tests/bristles/parallel_solve.rs` will fail from this task until Task 6: run the lab and the rest of the suite, not that file.

- [ ] **Step 1: Write the failing lab tests**

`src/engine/bristles/lab/tests/friction.rs`:

```rust
use super::*;

const STATIC_MU: f32 = 0.5;
const KINETIC_MU: f32 = 0.3;
const SLANT_DEGREES: f32 = 45.0;
const PRESS_PX: f32 = 1.0;
const PRESS_STEPS: u32 = 10;
const SMALL_DRAG_PX: f32 = 0.05;
const LONG_DRAG_PX: f32 = 3.0;
const DRAG_STEPS: u32 = 30;
const HELD_PX: f32 = 1e-3;
const COEFFICIENT_TOLERANCE: f32 = 0.02;
const THRESHOLD_RUNS: usize = 40;
const THRESHOLD_STEP_PX: f32 = 0.01;
const ANISOTROPY: f32 = 0.5;
const DEEP_PRESS_PX: f32 = 15.0;
const DEEP_PRESS_STEPS: u32 = 30;
const SAME_PATH_PX: f32 = 0.05;
const TIP: usize = BRISTLE_VERTICES - 1;

/// A straight strand slanted toward +x, its tip resting on the floor, then pressed.
fn slanted(press_px: f32, steps: u32) -> LabStrand {
    let clamp = tilted(SLANT_DEGREES);
    let tip_z = LENGTH_PX * clamp[2];
    strand(clamp, AT_ORIGIN)
        .with_floor(tip_z)
        .with_press(press_px, steps)
        .with_friction(STATIC_MU, KINETIC_MU, 0.0)
}

fn pressed() -> LabStrand {
    slanted(PRESS_PX, PRESS_STEPS)
}

fn length3(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn tip_ratio(run: &LabRun) -> f32 {
    length3(run.friction_n[TIP]) / run.vertex_forces_n[TIP]
}

fn xy_span(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

#[test]
fn a_tip_dragged_less_than_its_cone_allows_stays_on_its_anchor() {
    let gpu = GpuContext::new().unwrap();
    let runs = run(&gpu, &[pressed(), pressed().with_drag([SMALL_DRAG_PX, 0.0], DRAG_STEPS)]);
    let (held, dragged) = (&runs[0], &runs[1]);
    assert!(held.vertex_forces_n[TIP] > 0.0, "the press put no load on the tip");
    let moved = xy_span(held.chain_px[TIP], dragged.chain_px[TIP]);
    assert!(moved < HELD_PX, "the stuck tip moved {moved} px");
    assert!(dragged.anchor_modes[TIP] > 0.0, "the tip is not stuck");
    assert!(tip_ratio(dragged) <= STATIC_MU, "ratio {}", tip_ratio(dragged));
}

#[test]
fn a_tip_dragged_far_slides_at_the_kinetic_coefficient() {
    let gpu = GpuContext::new().unwrap();
    let runs = run(&gpu, &[pressed(), pressed().with_drag([LONG_DRAG_PX, 0.0], DRAG_STEPS)]);
    let (held, dragged) = (&runs[0], &runs[1]);
    assert!(dragged.anchor_modes[TIP] < 0.0, "the tip is not sliding");
    assert!(xy_span(held.chain_px[TIP], dragged.chain_px[TIP]) > HELD_PX);
    let ratio = tip_ratio(dragged);
    assert!(
        (ratio - KINETIC_MU).abs() <= COEFFICIENT_TOLERANCE * KINETIC_MU,
        "the sliding tip's friction is {ratio} of its load"
    );
}

#[test]
fn a_tip_lets_go_at_the_static_coefficient() {
    let gpu = GpuContext::new().unwrap();
    let strands: Vec<LabStrand> = (1..=THRESHOLD_RUNS)
        .map(|run| pressed().with_drag([run as f32 * THRESHOLD_STEP_PX, 0.0], 1))
        .collect();
    let runs = run(&gpu, &strands);
    let stuck: Vec<f32> = runs.iter().filter(|r| r.anchor_modes[TIP] > 0.0).map(tip_ratio).collect();
    assert!(!stuck.is_empty() && stuck.len() < runs.len(), "no run crossed the cone");
    let highest = stuck.iter().copied().fold(0.0, f32::max);
    assert!(highest <= STATIC_MU * (1.0 + 1e-3), "a stuck tip held {highest} of its load");
    assert!(
        highest >= STATIC_MU * (1.0 - 10.0 * COEFFICIENT_TOLERANCE),
        "the tip let go at {highest} of its load"
    );
}

/// The tip's pull direction points to the root, along −x; dragging toward −x pulls it.
#[test]
fn a_pulled_tip_slides_more_easily_than_a_pushed_one() {
    let gpu = GpuContext::new().unwrap();
    let drag = |sign: f32| {
        pressed()
            .with_friction(STATIC_MU, KINETIC_MU, ANISOTROPY)
            .with_drag([sign * LONG_DRAG_PX, 0.0], DRAG_STEPS)
    };
    let runs = run(&gpu, &[drag(-1.0), drag(1.0)]);
    let (pulled, pushed) = (tip_ratio(&runs[0]), tip_ratio(&runs[1]));
    let expected_pulled = KINETIC_MU * (1.0 - ANISOTROPY);
    assert!(
        (pulled - expected_pulled).abs() <= COEFFICIENT_TOLERANCE * expected_pulled,
        "pulled: {pulled} of the load, expected {expected_pulled}"
    );
    assert!(
        (pushed - KINETIC_MU).abs() <= COEFFICIENT_TOLERANCE * KINETIC_MU,
        "pushed: {pushed} of the load"
    );
}

#[test]
fn two_stuck_contacts_both_hold_their_anchors() {
    let gpu = GpuContext::new().unwrap();
    let deep = || slanted(DEEP_PRESS_PX, DEEP_PRESS_STEPS);
    let runs = run(&gpu, &[deep(), deep().with_drag([SMALL_DRAG_PX, 0.0], DRAG_STEPS)]);
    let (held, dragged) = (&runs[0], &runs[1]);
    let stuck: Vec<usize> = (1..BRISTLE_VERTICES)
        .filter(|&v| dragged.anchor_modes[v] > 0.0)
        .collect();
    assert!(stuck.len() >= 2, "only {stuck:?} stuck");
    for v in stuck {
        let moved = xy_span(held.chain_px[v], dragged.chain_px[v]);
        assert!(moved < HELD_PX, "vertex {v} moved {moved} px");
        let ratio = length3(dragged.friction_n[v]) / dragged.vertex_forces_n[v];
        assert!(ratio <= STATIC_MU * (1.0 + 1e-3), "vertex {v} holds {ratio}");
    }
}

#[test]
fn without_friction_a_dragged_tip_slides_along() {
    let gpu = GpuContext::new().unwrap();
    let frictionless = || pressed().with_friction(0.0, 0.0, 0.0);
    let runs = run(&gpu, &[frictionless(), frictionless().with_drag([SMALL_DRAG_PX, 0.0], DRAG_STEPS)]);
    assert!(xy_span(runs[0].chain_px[TIP], runs[1].chain_px[TIP]) > 0.5 * SMALL_DRAG_PX);
    assert_eq!(runs[1].anchor_modes[TIP], 0.0);
    assert_eq!(runs[1].friction_n[TIP], [0.0; 3]);
}

#[test]
fn a_drag_in_steps_half_as_long_ends_in_the_same_place() {
    let gpu = GpuContext::new().unwrap();
    let runs = run(
        &gpu,
        &[
            pressed().with_drag([LONG_DRAG_PX, 0.0], DRAG_STEPS),
            pressed().with_drag([LONG_DRAG_PX, 0.0], 2 * DRAG_STEPS),
        ],
    );
    for (a, b) in runs[0].chain_px.iter().zip(&runs[1].chain_px) {
        assert!(span_px(*a, *b) < SAME_PATH_PX, "{a:?} against {b:?}");
    }
}
```

The `strand`, `tilted`, `span_px`, `AT_ORIGIN`, `LENGTH_PX` helpers come from the parent `lab/tests.rs`. If `with_floor(tip_z)` leaves the tip a hair above the floor because of the floating rest chain, lower it by `SINK_TOLERANCE_PX`.

- [ ] **Step 2: Wire the lab to the drag phase, friction output and anchors** (`bristle_lab.wgsl`)

`lab_shape` fills the new `StrandShape` fields (Step 4 defines them):

```wgsl
fn lab_shape(strand: LabStrand) -> StrandShape {
    var compliances: HingeCompliances;
    for (var hinge = 0u; hinge < BRISTLE_SEGMENTS; hinge++) {
        compliances[hinge] = strand.compliances[hinge / 4u][hinge % 4u];
    }
    let friction = Friction(strand.friction.x, strand.friction.y, strand.friction.z);
    return StrandShape(strand.clamp.xyz, strand.curl.xyz, strand.root.w, compliances, strand.tip_force.xyz, friction, Turns());
}
```

`solve_lab` keeps `var anchors: Anchors;` and `var contacts: Contacts;` across both phases. Factor the per-step body into a function so both phases share it:

```wgsl
struct LabState {
    chain: Chain,
    sampled: Chain,
    anchors: Anchors,
    contacts: Contacts,
};

fn lab_step(state: ptr<function, LabState>, strand: LabStrand, shape: StrandShape, shift: vec3<f32>) {
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        (*state).chain[vertex] += shift;
    }
    for (var iteration = 0u; iteration < LAB_ITERATIONS_PER_STEP; iteration++) {
        let floors = lab_floors(strand, (*state).sampled, (*state).chain);
        (*state).sampled = (*state).chain;
        (*state).contacts = solve_iteration(&(*state).chain, shape, floors, &(*state).anchors);
    }
    if strand.step.w != 0.0 {
        let lifted = rested(&(*state).chain, (*state).sampled, shape, strand.step.z);
        for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
            (*state).contacts.forces[normal_row(vertex)] += lifted[vertex - 1u];
        }
    }
}
```

`solve_lab`: build `shape`, run the press steps with `shift = -drop`, then `drag.z` steps with `shift = vec3(strand.drag.xy / steps, 0.0)`; before the drag phase, if `strand.drag.w != 0.0`, set `shape.tip_force_n = vec3(0.0)`. Outputs:

```wgsl
    let normals = floor_normals(lab_floors(strand, state.sampled, state.chain));
    let normal = normal_forces(state.contacts);
    let friction = friction_forces(state.contacts, normals);
    lab_out[first] = vec4<f32>(state.chain[0], 0.0);
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        lab_out[first + vertex] = vec4<f32>(state.chain[vertex], normal[vertex - 1u]);
    }
    lab_out[first + BRISTLE_VERTICES] = vec4<f32>(summed(normal), 0.0, 0.0, 0.0);
    let friction_first = first + BRISTLE_VERTICES + 1u;
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        lab_out[friction_first + vertex] = vec4<f32>(friction[vertex], state.anchors[vertex].w);
    }
```

(The plastic block stays zero until Task 5.) Note `normals` here reads the floors at the final chain. They are the normals the last contact solve used to within a step. On the flat floors these tests use, they are exact.

- [ ] **Step 3: Run the new tests to see them fail**

Run: `dk 'cargo test --release -p cadmium-engine --lib bristles::lab::tests::friction 2>&1 | tail -20'`
Expected: compile errors (`Friction`, `Anchors`, `Contacts` undefined).

- [ ] **Step 4: Rewrite `bristle_contacts.wgsl`** (complete file)

```wgsl
alias PerContact = array<f32, BRISTLE_SEGMENTS>;
alias Anchors = array<vec4<f32>, BRISTLE_VERTICES>;
alias VertexForces = array<vec3<f32>, BRISTLE_VERTICES>;
alias VertexFlags = array<bool, BRISTLE_VERTICES>;
/// Rows per vertex: its floor normal, then the two tangents friction holds it along.
const ROW_KINDS: u32 = 3u;
const MAX_CONTACT_ROWS: u32 = ROW_KINDS * BRISTLE_SEGMENTS;
alias RowValues = array<f32, MAX_CONTACT_ROWS>;
alias RowFlags = array<bool, MAX_CONTACT_ROWS>;
alias ContactMatrix = array<array<f32, MAX_CONTACT_ROWS>, MAX_CONTACT_ROWS>;
alias ActiveRows = array<u32, MAX_CONTACT_ROWS>;

const CONTACT_ROUNDS: u32 = 8u;
/// How far under its floor a vertex left out of the active set may end, px.
const CONTACT_TOLERANCE_PX: f32 = 1.0e-3;
/// Share of the contact compliance added to every active pivot, so the Cholesky never
/// divides by zero. A vertex whose own compliance is no larger than the ridge has no
/// bending lever (a straight strand pressed along its length): no force can lift it, so
/// it takes none and sinks instead. In the app every strand has a rest curl, which gives
/// it the lever to buckle.
const CONTACT_RIDGE_SHARE: f32 = 1.0e-5;
const MIN_PIVOT: f32 = 1.0e-20;

fn row_vertex(row: u32) -> u32 {
    return row / ROW_KINDS + 1u;
}

fn normal_row(vertex: u32) -> u32 {
    return (vertex - 1u) * ROW_KINDS;
}

/// The direction a row's force pushes its vertex and its gap is measured along.
fn row_direction(normals: Normals, row: u32) -> vec3<f32> {
    let normal = normals[row_vertex(row)];
    let first = perpendicular(normal);
    let kind = row % ROW_KINDS;
    if kind == 0u {
        return normal;
    }
    if kind == 1u {
        return first;
    }
    return cross(normal, first);
}

/// How far row `row`'s vertex moves along its direction, px, under 1 N on row `column`'s
/// vertex along its own: Σ over the hinges above both of α·(arm × d_column)·(lever × d_row).
fn row_compliance(chain: Chain, compliances: HingeCompliances, normals: Normals, row: u32, column: u32) -> f32 {
    let vertex = row_vertex(row);
    let pushed = row_vertex(column);
    let along = row_direction(normals, row);
    let push = row_direction(normals, column);
    var rise = 0.0;
    for (var hinge = 0u; hinge < min(vertex, pushed); hinge++) {
        let lever = chain[vertex] - chain[hinge];
        let arm_m = (chain[pushed] - chain[hinge]) * METRES_PER_PX;
        rise += compliances[hinge] * dot(cross(arm_m, push), cross(lever, along));
    }
    return rise;
}

fn moved_by(chain: Chain, turns: Turns, vertex: u32) -> vec3<f32> {
    var moved = vec3<f32>(0.0);
    for (var hinge = 0u; hinge < vertex; hinge++) {
        moved += cross(turns[hinge], chain[vertex] - chain[hinge]);
    }
    return moved;
}

/// Each row's gap after `turns`, px: for a normal row how far inside the relief the vertex
/// ends (the linearised z − f(x, y) ≥ 0 over the gradient's length); for a tangent row how
/// far it ends from its anchor along that tangent, negated.
fn gaps_after(chain: Chain, turns: Turns, floors: Floors, normals: Normals, anchors: Anchors) -> RowValues {
    var gaps: RowValues;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let moved = moved_by(chain, turns, vertex);
        let normal = normals[vertex];
        let row = normal_row(vertex);
        gaps[row] = (floors.heights_px[vertex] - chain[vertex].z) * normal.z - dot(normal, moved);
        let off_anchor = chain[vertex] + moved - anchors[vertex].xyz;
        gaps[row + 1u] = -dot(row_direction(normals, row + 1u), off_anchor);
        gaps[row + 2u] = -dot(row_direction(normals, row + 2u), off_anchor);
    }
    return gaps;
}

struct ContactProblem {
    chain: Chain,
    compliances: HingeCompliances,
    normals: Normals,
    /// Each row's own compliance: the diagonal of the contact matrix.
    diagonal: RowValues,
    ridge: f32,
    gaps: RowValues,
};

fn entry(problem: ptr<function, ContactProblem>, row: u32, column: u32) -> f32 {
    return row_compliance((*problem).chain, (*problem).compliances, (*problem).normals, row, column);
}

/// The compliance the ridge is a share of, px per N: the largest normal diagonal, but never
/// less than the tip hinge's rise under a one-segment lever, so a strand with no lever
/// keeps a finite answer.
fn ridge_scale(diagonal: RowValues, chain: Chain, compliances: HingeCompliances) -> f32 {
    let segment_px = distance(chain[1], chain[0]);
    var largest = compliances[BRISTLE_SEGMENTS - 1u] * segment_px * segment_px * METRES_PER_PX;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        largest = max(largest, diagonal[normal_row(vertex)]);
    }
    return largest;
}

/// Whether a force on the row's vertex can move it by bending at all.
fn liftable(problem: ptr<function, ContactProblem>, row: u32) -> bool {
    return (*problem).diagonal[row] > (*problem).ridge;
}

fn active_rows(engaged: RowFlags, rows: ptr<function, ActiveRows>) -> u32 {
    var count = 0u;
    for (var row = 0u; row < MAX_CONTACT_ROWS; row++) {
        if engaged[row] {
            (*rows)[count] = row;
            count++;
        }
    }
    return count;
}

/// Solves the ridged system of the `count` active rows by Cholesky, in place: the same
/// arithmetic as the full system with the inactive rows left out, since their entries are
/// zero. Returns the forces in the order of `rows`.
fn cholesky_solved(problem: ptr<function, ContactProblem>, rows: ActiveRows, count: u32) -> RowValues {
    var lower: ContactMatrix;
    var rhs: RowValues;
    for (var i = 0u; i < count; i++) {
        rhs[i] = (*problem).gaps[rows[i]];
        for (var j = 0u; j < i; j++) {
            lower[i][j] = entry(problem, rows[i], rows[j]);
        }
        lower[i][i] = (*problem).diagonal[rows[i]] + (*problem).ridge;
    }
    return solved_by_cholesky(lower, rhs, count);
}

fn solved_by_cholesky(matrix: ContactMatrix, rhs: RowValues, count: u32) -> RowValues {
    // body unchanged from phase 1, with RowValues in place of PerContact
}

fn active_forces(problem: ptr<function, ContactProblem>, engaged: RowFlags) -> RowValues {
    var rows: ActiveRows;
    let count = active_rows(engaged, &rows);
    var forces: RowValues;
    if count == 1u {
        let row = rows[0];
        forces[row] = (*problem).gaps[row] / ((*problem).diagonal[row] + (*problem).ridge);
        return forces;
    }
    let solved = cholesky_solved(problem, rows, count);
    for (var i = 0u; i < count; i++) {
        forces[rows[i]] = solved[i];
    }
    return forces;
}

/// Drops the contacts that would pull the strand down, with their tangent rows. True if
/// any was dropped.
fn released(engaged: ptr<function, RowFlags>, forces: RowValues) -> bool {
    var dropped = false;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let row = normal_row(vertex);
        if (*engaged)[row] && forces[row] < 0.0 {
            (*engaged)[row] = false;
            (*engaged)[row + 1u] = false;
            (*engaged)[row + 2u] = false;
            dropped = true;
        }
    }
    return dropped;
}

/// How far the row forces lift each vertex (1..) along its normal, px.
fn rises(problem: ptr<function, ContactProblem>, forces: RowValues) -> PerContact {
    var turns: Turns;
    for (var row = 0u; row < MAX_CONTACT_ROWS; row++) {
        let force = forces[row];
        if force != 0.0 {
            let pushed = row_vertex(row);
            let along = row_direction((*problem).normals, row);
            for (var hinge = 0u; hinge < pushed; hinge++) {
                let arm_m = ((*problem).chain[pushed] - (*problem).chain[hinge]) * METRES_PER_PX;
                turns[hinge] += (*problem).compliances[hinge] * force * cross(arm_m, along);
            }
        }
    }
    var lifted: PerContact;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        lifted[vertex - 1u] = dot((*problem).normals[vertex], moved_by((*problem).chain, turns, vertex));
    }
    return lifted;
}

/// Adds the vertices the forces leave under their floor, without tangent rows: a contact
/// lands frictionless and is anchored where the step leaves it. True if any was added.
fn landed(problem: ptr<function, ContactProblem>, engaged: ptr<function, RowFlags>, forces: RowValues) -> bool {
    let lifted = rises(problem, forces);
    var added = false;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let row = normal_row(vertex);
        let under = (*problem).gaps[row] - lifted[vertex - 1u] > CONTACT_TOLERANCE_PX;
        if !(*engaged)[row] && under && liftable(problem, row) {
            (*engaged)[row] = true;
            added = true;
        }
    }
    return added;
}

fn any_under(gaps: RowValues) -> bool {
    var under = false;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        under = under || gaps[normal_row(vertex)] > 0.0;
    }
    return under;
}

fn ends_under(chain: Chain, heights_px: array<f32, BRISTLE_VERTICES>) -> bool {
    // unchanged from phase 1
}

fn contact_problem(chain: Chain, compliances: HingeCompliances, normals: Normals, gaps: RowValues) -> ContactProblem {
    var problem = ContactProblem(chain, compliances, normals, RowValues(), 0.0, gaps);
    for (var row = 0u; row < MAX_CONTACT_ROWS; row++) {
        let vertex = row_vertex(row);
        let along = row_direction(normals, row);
        var own = 0.0;
        for (var hinge = 0u; hinge < vertex; hinge++) {
            let lever = cross(chain[vertex] - chain[hinge], along);
            own += compliances[hinge] * dot(lever, lever);
        }
        problem.diagonal[row] = own * METRES_PER_PX;
    }
    problem.ridge = CONTACT_RIDGE_SHARE * ridge_scale(problem.diagonal, chain, compliances);
    return problem;
}
```

The two bodies marked "unchanged" are copied verbatim from the current file. Keep the normal-row arithmetic in the same order as phase 1: with μ = 0 every phase 1 lab test must reproduce its numbers.

- [ ] **Step 5: Write `bristle_friction.wgsl`** (complete file)

```wgsl
/// How far a sliding vertex must slip in a step to keep sliding, px: under it, it sticks
/// again on its last anchor, and a reversed slip sticks before it can reverse.
const SLIP_TOLERANCE_PX: f32 = 1.0e-3;

/// The canvas's Coulomb friction, and Baxter & Lin's anisotropy C_η.
struct Friction {
    static_mu: f32,
    kinetic_mu: f32,
    anisotropy: f32,
};

/// One strand's contacts after a solve.
struct Contacts {
    /// Each row's force, N: along the normal, or friction along a tangent.
    forces: RowValues,
    engaged: RowFlags,
    /// Kinetic friction on each sliding vertex, N, world.
    kinetic: VertexForces,
    sliding: VertexFlags,
};

fn tangential(v: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    return v - normal * dot(normal, v);
}

/// The direction toward the root in the relief's tangent plane: the way a pulled vertex
/// slides.
fn pull_direction(chain: Chain, normals: Normals, vertex: u32) -> vec3<f32> {
    let toward_root = tangential(chain[vertex - 1u] - chain[vertex], normals[vertex]);
    return toward_root / max(length(toward_root), MIN_SEGMENT_SPAN_PX);
}

/// Baxter & Lin: μ·(1 − C_η·max(0, d_pull·ŝ)²). A pulled vertex slides more easily than a
/// pushed one.
fn anisotropic_mu(mu: f32, friction: Friction, pull: vec3<f32>, slip: vec3<f32>) -> f32 {
    let along = max(dot(pull, slip), 0.0);
    return mu * (1.0 - friction.anisotropy * along * along);
}

fn kinetic_turns(free: Turns, chain: Chain, compliances: HingeCompliances, kinetic: VertexForces) -> Turns {
    var turns = free;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let force = kinetic[vertex];
        if any(force != vec3<f32>(0.0)) {
            for (var hinge = 0u; hinge < vertex; hinge++) {
                let arm_m = (chain[vertex] - chain[hinge]) * METRES_PER_PX;
                turns[hinge] += compliances[hinge] * cross(arm_m, force);
            }
        }
    }
    return turns;
}

/// Kinetic friction on the contacts that were sliding, against the slip the free step
/// would give them, with the normal force of the last iteration.
fn slide_on(contacts: ptr<function, Contacts>, chain: Chain, free: Turns, normals: Normals, friction: Friction, anchors: Anchors) {
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let anchor = anchors[vertex];
        if anchor.w < 0.0 {
            let slip = tangential(chain[vertex] + moved_by(chain, free, vertex) - anchor.xyz, normals[vertex]);
            let size = length(slip);
            if size > SLIP_TOLERANCE_PX {
                let heading = slip / size;
                let mu = anisotropic_mu(friction.kinetic_mu, friction, pull_direction(chain, normals, vertex), heading);
                (*contacts).kinetic[vertex] = mu * anchor.w * heading;
                (*contacts).sliding[vertex] = true;
            }
        }
    }
}

/// Stuck contacts whose friction leaves the static cone start to slide: they lose their
/// tangent rows and take the kinetic force along the friction they held instead. True if
/// any did.
fn slipped(contacts: ptr<function, Contacts>, problem: ptr<function, ContactProblem>, friction: Friction) -> bool {
    var any_slipped = false;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let row = normal_row(vertex);
        if (*contacts).engaged[row + 1u] {
            let normals = (*problem).normals;
            let held = (*contacts).forces[row + 1u] * row_direction(normals, row + 1u)
                + (*contacts).forces[row + 2u] * row_direction(normals, row + 2u);
            let size = length(held);
            let load = max((*contacts).forces[row], 0.0);
            let pull = pull_direction((*problem).chain, normals, vertex);
            if size > 0.0 && size > anisotropic_mu(friction.static_mu, friction, pull, -held / size) * load {
                let along = held / size;
                (*contacts).engaged[row + 1u] = false;
                (*contacts).engaged[row + 2u] = false;
                (*contacts).kinetic[vertex] = anisotropic_mu(friction.kinetic_mu, friction, pull, -along) * load * along;
                (*contacts).sliding[vertex] = true;
                any_slipped = true;
            }
        }
    }
    return any_slipped;
}

/// The canvas's forces on a strand: one-sided normal contact and Coulomb friction, as an
/// active set over normal and tangent rows (Baxter & Lin's and Chu & Tai's quasi-static
/// brushes). With μₛ = 0 it is the frictionless contact of phase 1.
fn contact_forces(chain: Chain, free: Turns, compliances: HingeCompliances, floors: Floors, normals: Normals, friction: Friction, anchors: Anchors) -> Contacts {
    var contacts: Contacts;
    let frictional = friction.static_mu > 0.0;
    if frictional {
        slide_on(&contacts, chain, free, normals, friction, anchors);
    }
    let gaps = gaps_after(chain, kinetic_turns(free, chain, compliances, contacts.kinetic), floors, normals, anchors);
    if !any_under(gaps) {
        return Contacts();
    }
    var problem = contact_problem(chain, compliances, normals, gaps);
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let row = normal_row(vertex);
        let engaged = gaps[row] > 0.0 && liftable(&problem, row);
        let stuck = engaged && frictional && anchors[vertex].w != 0.0 && !contacts.sliding[vertex];
        contacts.engaged[row] = engaged;
        contacts.engaged[row + 1u] = stuck;
        contacts.engaged[row + 2u] = stuck;
    }
    var settled = false;
    for (var round = 0u; round < CONTACT_ROUNDS && !settled; round++) {
        contacts.forces = active_forces(&problem, contacts.engaged);
        let dropped = released(&contacts.engaged, contacts.forces);
        var slid = false;
        if !dropped && frictional && slipped(&contacts, &problem, friction) {
            problem.gaps = gaps_after(chain, kinetic_turns(free, chain, compliances, contacts.kinetic), floors, normals, anchors);
            slid = true;
        }
        settled = !dropped && !slid && !landed(&problem, &contacts.engaged, contacts.forces);
    }
    if !settled {
        contacts.forces = active_forces(&problem, contacts.engaged);
    }
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let row = normal_row(vertex);
        let engaged = contacts.engaged[row];
        contacts.forces[row] = select(0.0, max(contacts.forces[row], 0.0), engaged);
        for (var tangent = 1u; tangent < ROW_KINDS; tangent++) {
            contacts.engaged[row + tangent] = engaged && contacts.engaged[row + tangent];
            contacts.forces[row + tangent] = select(0.0, contacts.forces[row + tangent], contacts.engaged[row + tangent]);
        }
        if !engaged {
            contacts.kinetic[vertex] = vec3<f32>(0.0);
            contacts.sliding[vertex] = false;
        }
    }
    return contacts;
}

/// Where friction holds each vertex next: a sliding contact's anchor moves with it; a
/// stuck one keeps its anchor; a contact without tangent rows (just landed) is anchored
/// where the step left it; a free vertex has none.
fn next_anchors(chain: Chain, contacts: Contacts, anchors: Anchors) -> Anchors {
    var next: Anchors;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let row = normal_row(vertex);
        let load = contacts.forces[row];
        if contacts.engaged[row] && load > 0.0 {
            if contacts.sliding[vertex] {
                next[vertex] = vec4<f32>(chain[vertex], -load);
            } else if contacts.engaged[row + 1u] {
                next[vertex] = vec4<f32>(anchors[vertex].xyz, load);
            } else {
                next[vertex] = vec4<f32>(chain[vertex], load);
            }
        }
    }
    return next;
}

fn normal_forces(contacts: Contacts) -> PerContact {
    var forces: PerContact;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        forces[vertex - 1u] = contacts.forces[normal_row(vertex)];
    }
    return forces;
}

/// The canvas's friction on each vertex, N, world: the stuck rows' force plus the kinetic.
fn friction_forces(contacts: Contacts, normals: Normals) -> VertexForces {
    var forces: VertexForces;
    for (var vertex = 1u; vertex < BRISTLE_VERTICES; vertex++) {
        let row = normal_row(vertex);
        forces[vertex] = contacts.forces[row + 1u] * row_direction(normals, row + 1u)
            + contacts.forces[row + 2u] * row_direction(normals, row + 2u)
            + contacts.kinetic[vertex];
    }
    return forces;
}
```

- [ ] **Step 6: Change `bristle_solve.wgsl`**

`StrandShape` gains two members after `tip_force_n`:

```wgsl
    friction: Friction,
    /// The plastic shift of every hinge's rest turn, world, at the clamp.
    plastic: Turns,
```

`free_turns` adds the shift, carried like the curl:

```wgsl
        let curl = transported(shape.curl, shape.clamp, before);
        let rest = select(vec3<f32>(0.0), curl, hinge > 0u) + transported(shape.plastic[hinge], shape.clamp, before);
```

Replace `summed`, `solve_iteration`, `lifted_onto`, `pushed_step` and `rested`:

```wgsl
fn summed(forces: PerContact) -> f32 {
    // unchanged
}

/// One Gauss-Newton step of the strand's bending energy under its canvas contacts and
/// friction, from `chain` (vertex 0 already on the ferrule). Moves `anchors` on with the
/// contacts. The forces are the equilibrium of the full step, exact only once the cap on
/// the turns no longer engages.
fn solve_iteration(chain: ptr<function, Chain>, shape: StrandShape, floors: Floors, anchors: ptr<function, Anchors>) -> Contacts {
    let edges = edges_of(*chain, shape.clamp);
    return pushed_step(chain, edges, free_turns(*chain, edges, shape), shape, shape.friction, floors, anchors);
}

/// The contact step alone: the strand keeps its bend and the canvas only pushes it out,
/// frictionless, leaving no anchors.
fn lifted_onto(chain: ptr<function, Chain>, shape: StrandShape, floors: Floors) -> Contacts {
    var none: Anchors;
    return pushed_step(chain, edges_of(*chain, shape.clamp), Turns(), shape, Friction(0.0, 0.0, 0.0), floors, &none);
}

fn pushed_step(chain: ptr<function, Chain>, edges: Edges, free: Turns, shape: StrandShape, friction: Friction, floors: Floors, anchors: ptr<function, Anchors>) -> Contacts {
    let normals = floor_normals(floors);
    let contacts = contact_forces(*chain, free, shape.compliances, floors, normals, friction, *anchors);
    var turns = kinetic_turns(free, *chain, shape.compliances, contacts.kinetic);
    for (var row = 0u; row < MAX_CONTACT_ROWS; row++) {
        let force = contacts.forces[row];
        let pushed = row_vertex(row);
        let along = row_direction(normals, row);
        for (var hinge = 0u; hinge < pushed; hinge++) {
            let arm_m = ((*chain)[pushed] - (*chain)[hinge]) * METRES_PER_PX;
            turns[hinge] += shape.compliances[hinge] * force * cross(arm_m, along);
        }
    }
    turn_chain(chain, edges, capped(turns), shape.segment_px);
    *anchors = next_anchors(*chain, contacts, *anchors);
    return contacts;
}
```

In `rested`, `let lifted = lifted_onto(...)` becomes `let lifted = normal_forces(lifted_onto(chain, shape, floors));`; the rest is unchanged.

- [ ] **Step 7: Fill `strand_shape`** (`bristle_shape.wgsl`)

```wgsl
    return StrandShape(
        direction_at(rest_clamp_local(rest), pose),
        direction_at(rest_curl_local(rest), pose),
        segment_length(rest),
        compliances,
        vec3<f32>(0.0),
        Friction(tuning.friction.x, tuning.friction.y, tuning.friction.z),
        Turns(),
    );
```

- [ ] **Step 8: Carry anchors through the serial dab** (`bristle_step.wgsl`)

```wgsl
fn held_anchors(bristle: u32) -> Anchors {
    var anchors: Anchors;
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        anchors[vertex] = states[anchor_slot(bristle, vertex)];
    }
    return anchors;
}

fn store_anchors(bristle: u32, anchors: Anchors) {
    for (var vertex = 0u; vertex < BRISTLE_VERTICES; vertex++) {
        states[anchor_slot(bristle, vertex)] = anchors[vertex];
    }
}
```

In `solved_dab`: `var anchors = held_anchors(bristle);` before the loop; the solve line becomes `force = summed(normal_forces(solve_iteration(&chain, shape, floors, &anchors)));`; after `store_chain(bristle, chain);` add `store_anchors(bristle, anchors);`.

- [ ] **Step 9: Run the lab**

Run: `dk 'cargo test --release -p cadmium-engine --lib bristles::lab 2>&1 | tail -30'`
Expected: all friction tests and all phase 1 lab tests PASS. If a phase 1 lab test changed numbers, the normal-row arithmetic order differs from phase 1: compare the loops with the old file in `git show HEAD:crates/cadmium-engine/src/shaders/bristle_contacts.wgsl` before touching any tolerance.

If `a_tip_lets_go_at_the_static_coefficient` or the kinetic ratios miss by more than the tolerance, report the measured numbers; do not widen the tolerances on your own.

- [ ] **Step 10: Run the suite without the equivalence test**

Run: `dk 'cargo test --release -p cadmium-engine 2>&1 | grep -E "^test |^test result|FAILED|panicked" | grep -vE " ok$"'`
Expected: only `parallel_solve` tests fail (the app still runs the frictionless grouped solve and the reference solve now has friction). Any other new failure: report it with its output.

- [ ] **Step 11: Commit**

```bash
git add -A crates/cadmium-engine
git commit -m "feat(engine): Coulomb friction in the serial bristle solve"
```

---

### Task 5: Plasticity in the serial solve

**Files:**
- Fill: `src/shaders/bristle_plastic.wgsl`
- Modify: `src/shaders/bristle_step.wgsl`, `src/shaders/bristle_lab.wgsl`
- Create: `src/engine/bristles/lab/tests/plasticity.rs` (declare `mod plasticity;` in `lab/tests.rs`)

- [ ] **Step 1: Write the failing lab tests**

```rust
use super::*;

const SLANT_DEGREES: f32 = 30.0;
/// Bends hinge 0 by about 0.03 rad over the test strand, past the 0.02 rad yield.
const HOLD_FORCE_N: f32 = 2.0e-5;
const CREEP_PER_S: f32 = 10.0;
const STEP_S: f32 = 0.01;
const HOLD_STEPS: u32 = 5;
const LONG_HOLD_STEPS: u32 = 400;
const RELEASE_STEPS: u32 = 10;
const YIELD_RAD: f32 = 0.02;
const CAP_RAD: f32 = 0.1;
const CREEP_TOLERANCE: f32 = 0.02;
const RESTED_RAD: f32 = 1e-3;
const STAYS_BENT_PX: f32 = 0.1;

fn held(steps: u32) -> LabStrand {
    strand(tilted(SLANT_DEGREES), AT_ORIGIN)
        .with_floor(FLOOR_FAR_BELOW_PX)
        .with_tip_force([0.0, 0.0, -HOLD_FORCE_N])
        .with_press(0.0, steps)
        .with_creep(CREEP_PER_S, STEP_S)
}

fn length3(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

#[test]
fn a_held_bend_creeps_the_rest_shape_past_the_yield() {
    let gpu = GpuContext::new().unwrap();
    let runs = run(&gpu, &[held(HOLD_STEPS)]);
    let share = CREEP_PER_S * STEP_S;
    let mut crept = 0;
    for (hinge, (&excess, shift)) in runs[0].excess_rad.iter().zip(&runs[0].plastic).enumerate() {
        let expected = if excess > YIELD_RAD {
            HOLD_STEPS as f32 * share * (excess - YIELD_RAD)
        } else {
            0.0
        };
        let got = length3(*shift);
        if expected == 0.0 {
            assert_eq!(got, 0.0, "hinge {hinge} crept under the yield");
        } else {
            crept += 1;
            assert!(
                (got - expected).abs() <= CREEP_TOLERANCE * expected,
                "hinge {hinge}: shift {got}, expected {expected}"
            );
        }
    }
    assert!(crept > 0, "no hinge was bent past the yield");
}

#[test]
fn a_bend_held_for_no_time_does_not_creep() {
    let gpu = GpuContext::new().unwrap();
    let runs = run(&gpu, &[held(HOLD_STEPS).with_creep(CREEP_PER_S, 0.0)]);
    assert!(runs[0].plastic.iter().all(|shift| *shift == [0.0; 3]));
}

#[test]
fn the_plastic_shift_stops_at_its_cap() {
    let gpu = GpuContext::new().unwrap();
    let runs = run(&gpu, &[held(LONG_HOLD_STEPS)]);
    let largest = runs[0].plastic.iter().map(|s| length3(*s)).fold(0.0, f32::max);
    assert!(largest <= CAP_RAD * (1.0 + 1e-5), "a shift reached {largest} rad");
    assert!(largest >= CAP_RAD * (1.0 - 1e-3), "the largest shift is {largest} rad");
}

#[test]
fn a_released_strand_rests_on_its_shifted_shape() {
    let gpu = GpuContext::new().unwrap();
    let unbent = strand(tilted(SLANT_DEGREES), AT_ORIGIN).with_floor(FLOOR_FAR_BELOW_PX);
    let released = held(LONG_HOLD_STEPS)
        .with_drag([0.0, 0.0], RELEASE_STEPS)
        .released_for_the_drag()
        .with_drag_seconds(0.0);
    let runs = run(&gpu, &[unbent, released]);
    let rested = &runs[1];
    let largest_excess = rested.excess_rad.iter().copied().fold(0.0, f32::max);
    assert!(largest_excess < RESTED_RAD, "the released strand still bends {largest_excess} rad");
    let tip = BRISTLE_VERTICES - 1;
    assert!(span_px(runs[0].chain_px[tip], rested.chain_px[tip]) > STAYS_BENT_PX);
}
```

In the release test the drag phase takes no time, so the shift set during the hold stays put while the strand springs back. Calibrate `HOLD_FORCE_N` so that hinge 0's excess is about 0.03 rad: print `excess_rad` once with `eprintln!`, set the constant, and remove the print.

- [ ] **Step 2: Run them to see them fail**

Run: `dk 'cargo test --release -p cadmium-engine --lib bristles::lab::tests::plasticity 2>&1 | tail -20'`
Expected: FAIL (the shifts are all zero).

- [ ] **Step 3: Write `bristle_plastic.wgsl`** (complete file)

```wgsl
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
```

- [ ] **Step 4: Creep in the lab** (`bristle_lab.wgsl`)

`LabState` gains `plastic: Turns`, and `lab_step` takes a `step_s: f32` argument (`strand.timing.x` in the press phase, `strand.timing.y` in the drag phase). In `lab_step`, set `shape.plastic = (*state).plastic` before the iterations (take `shape` by value as `var`), and after them (and after `rested`):

```wgsl
    var current = shape;
    current.plastic = (*state).plastic;
    (*state).plastic = crept((*state).chain, current, creep_share(strand.friction.w, step_s));
```

Restructure `lab_step` so the iterations use `current` (with the shift) too. Output the plastic block:

```wgsl
    var final_shape = shape;
    final_shape.plastic = state.plastic;
    let excess = excess_sizes(state.chain, final_shape);
    let plastic_first = friction_first + BRISTLE_VERTICES;
    for (var hinge = 0u; hinge < BRISTLE_SEGMENTS; hinge++) {
        lab_out[plastic_first + hinge] = vec4<f32>(state.plastic[hinge], excess[hinge]);
    }
```

- [ ] **Step 5: Creep in the serial dab** (`bristle_step.wgsl`)

```wgsl
fn held_plastic(bristle: u32, pose: Pose) -> Turns {
    var plastic: Turns;
    for (var hinge = 0u; hinge < BRISTLE_SEGMENTS; hinge++) {
        plastic[hinge] = direction_at(states[plastic_slot(bristle, hinge)].xyz, pose);
    }
    return plastic;
}

fn store_plastic(bristle: u32, plastic: Turns, pose: Pose) {
    for (var hinge = 0u; hinge < BRISTLE_SEGMENTS; hinge++) {
        states[plastic_slot(bristle, hinge)] = vec4<f32>(direction_in_ferrule(plastic[hinge], pose), 0.0);
    }
}
```

In `solved_dab`, inside the loop after `shape = strand_shape(rests[bristle], pose);` add `shape.plastic = held_plastic(bristle, pose);`. After the `rested` line:

```wgsl
    store_plastic(bristle, crept(chain, shape, creep_share(tuning.friction.w, dab.dt_seconds)), previous);
```

`shape` and `previous` there are the last iteration's, at the end pose. Reading the shift from `states` every iteration is the same value the whole dab (it is stored only at the end), so rereading it per pose costs one transform and no extra state.

- [ ] **Step 6: Run the lab and the friction integration tests**

Run: `dk 'cargo test --release -p cadmium-engine --lib bristles::lab 2>&1 | tail -20'`
Expected: PASS.
Run: `dk 'cargo test --release -p cadmium-engine --test bristles friction:: 2>&1 | tail -20'`
Expected: `no_plasticity_lets_the_brush_spring_back_straight` passes, and so does `a_wet_brush_never_sets_its_point_into_its_bend_memory`. The app still runs the grouped solve, so these exercise the reference path only once Task 6 lands. Note any failure for Task 8.

- [ ] **Step 7: Commit**

```bash
git add -A crates/cadmium-engine
git commit -m "feat(engine): plastic rest shift that creeps in the serial bristle solve"
```

---

### Task 6: Friction in the grouped solve

The grouped solve (`bristle_strand_groups.wgsl` + `bristle_group_contacts.wgsl`) mirrors the serial solve function by function. Each helper carries a `// mirrors <serial fn>` comment. Lane v owns vertex v, hinge v and the contact block of vertex v + 1 (rows 3v, 3v+1, 3v+2). Lane 0 of the strand runs the serial parts: the active set (`released`, `slipped`) and the Cholesky. This task brings it to Task 4's serial solve, term for term.

**Files:**
- Modify: `src/shaders/bristle_strand_groups.wgsl`, `src/shaders/bristle_group_contacts.wgsl`
- Create: `src/shaders/bristle_group_friction.wgsl` (add it to the `include_str!` list in `src/engine/pass/bristle_step.rs`)
- Modify: `tests/bristles/parallel_solve.rs`

- [ ] **Step 1: Extend the equivalence test** (failing until this task's code exists)

In `assert_same_strands` add, after the vertex loop:

```rust
    for (bristle, (a, b)) in reference.anchors.iter().zip(&grouped.anchors).enumerate() {
        for (vertex, (x, y)) in a.iter().zip(b).enumerate() {
            assert_eq!(x[3].signum(), y[3].signum(), "bristle {bristle} vertex {vertex}: modes {x:?} {y:?}");
            for axis in 0..3 {
                assert!(
                    (x[axis] - y[axis]).abs() <= POSITION_TOLERANCE_PX,
                    "bristle {bristle} vertex {vertex}: anchors {x:?} {y:?}"
                );
            }
        }
    }
```

In `the_grouped_solve_matches_the_serial_one_over_ridges`, after the existing `assert!`:

```rust
    let modes: Vec<f32> = serial.anchors.iter().flatten().map(|a| a[3]).collect();
    assert!(modes.iter().any(|&w| w > 0.0), "no contact stuck");
    assert!(modes.iter().any(|&w| w < 0.0), "no contact slid");
```

Run: `dk 'cargo test --release -p cadmium-engine --test bristles parallel_solve 2>&1 | tail -20'`
Expected: FAIL (the grouped solve has no friction).

- [ ] **Step 2: Workgroup state**

Row-indexed workgroup arrays (`group_gaps`, `group_diagonal`, `group_forces`, `group_engaged`, `group_added`) become `array<array<…, MAX_CONTACT_ROWS>, BRISTLE_STRANDS_PER_GROUP>`. `group_matrix` is already `array<ContactMatrix, …>` and grows with the alias. New arrays:

```wgsl
var<workgroup> group_anchors: array<array<vec4<f32>, BRISTLE_VERTICES>, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_kinetic: GroupVectors;
var<workgroup> group_sliding: array<array<u32, BRISTLE_VERTICES>, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_plastic: GroupHinges;
```

Each lane loads `group_anchors[s][v]` from `states[anchor_slot(...)]` at the start of `solve` (zero after a reset) and stores its vertex's anchor at the end. `group_plastic` stays zero in this task (Task 7 fills it). `free_turn` adds `transported(group_plastic[s][hinge], shape.clamp, before)` to `rest`, exactly as `free_turns` does.

- [ ] **Step 3: Mirror the serial functions, lane by lane**

Write each in `bristle_group_friction.wgsl` (friction) or `bristle_group_contacts.wgsl` (rows), keeping every loop in the serial order:

| serial (Task 4) | grouped | who |
|---|---|---|
| `slide_on` | `slide_vertex(lane)`: the vertex v+1 block of `slide_on`, writing `group_kinetic[s][v+1]`, `group_sliding[s][v+1]` | each hinge-owning lane, after the free turns barrier |
| `kinetic_turns` | `kinetic_turn(strand, hinge, free)`: `free + Σ_{vertex>hinge, ascending} α·cross(arm_m, kinetic[vertex])` | each hinge lane |
| `gaps_after` | `gap_rows(lane)`: the three rows of vertex v+1, from `group_turns` (which now hold the kinetic turns) | each hinge lane |
| `contact_problem` diagonal | `diagonal_rows(lane)`: three rows | each hinge lane |
| `row_compliance` matrix fill | lane v fills rows 3v..3v+2, every column below each row | each hinge lane |
| initial engagement | as `contact_forces`, per vertex block | lane 0 (in `open_contacts`) |
| `released` | `released` over blocks | lane 0 |
| `slipped` | `slipped_group(strand)`; when it returns true, the kinetic turns and the gaps are rebuilt by every lane after a barrier, so the loop count must stay workgroup-uniform (use `workgroupUniformLoad` of a flag, as `group_unsettled` does) | lane 0, then all lanes |
| `rises` + `landed` | `rise_turn` sums over all rows `row_vertex(row) > hinge`, ascending row; `land_row` checks the normal row of its vertex | each hinge lane |
| final clamp of forces / kinetic | as `contact_forces`' last loop | lane 0 |
| `pushed_step` turns | `pushed_turn(strand, hinge, free)`: kinetic turn first (as `kinetic_turns`), then Σ rows ascending | each hinge lane |
| `next_anchors` | `next_anchor(lane)` for vertex v+1, after `turned_position` gives the new chain | each hinge lane |
| `lifted_onto` (frictionless) | the existing `lifting` path passes friction off and leaves `group_anchors` untouched | — |

The order of each sum must match the serial nesting. A per-hinge accumulation over vertices or rows runs in ascending order, as the serial outer loop does. Check each against its serial function line by line.

- [ ] **Step 4: Run the equivalence test**

Run: `dk 'cargo test --release -p cadmium-engine --test bristles parallel_solve 2>&1 | tail -20'`
Expected: PASS. A mismatch usually means a sum in a different order or a missed barrier. Bisect by comparing one strand's anchors and forces after one dab (add a temporary test with a one-sample stroke) before touching tolerances.

- [ ] **Step 5: Run the whole engine suite**

Run: `dk 'cargo test --release -p cadmium-engine 2>&1 | grep -E "^test result|FAILED|panicked"'`
Expected: no failure except the phase 2 tests that are still ignored. List any new failure with its output for Task 8.

- [ ] **Step 6: Commit**

```bash
git add -A crates/cadmium-engine
git commit -m "feat(engine): Coulomb friction in the grouped bristle solve"
```

---

### Task 7: Plasticity in the grouped solve

**Files:**
- Modify: `src/shaders/bristle_strand_groups.wgsl` (and `bristle_group_friction.wgsl` if `bristle_strand_groups.wgsl` would pass 400 lines)
- Modify: `tests/bristles/parallel_solve.rs`

- [ ] **Step 1: Extend the equivalence test**

```rust
const SHIFT_TOLERANCE_RAD: f32 = 1.0e-5;
```

In `assert_same_strands`:

```rust
    for (bristle, (a, b)) in reference.plastic.iter().zip(&grouped.plastic).enumerate() {
        for (hinge, (x, y)) in a.iter().zip(b).enumerate() {
            for axis in 0..3 {
                assert!(
                    (x[axis] - y[axis]).abs() <= SHIFT_TOLERANCE_RAD,
                    "bristle {bristle} hinge {hinge}: shifts {x:?} {y:?}"
                );
            }
        }
    }
```

And in the ridges test: `assert!(serial.plastic.iter().flatten().any(|p| *p != [0.0; 3]), "nothing set");`

Run it. Expected: FAIL.

- [ ] **Step 2: Mirror `held_plastic`, `crept` and `store_plastic`**

- Load: each hinge lane loads `group_plastic[s][h] = direction_at(states[plastic_slot(b, h)].xyz, pose)` at every iteration's pose, as the serial dab does. It is the same stored value each time.
- Creep: after the floor corrections, each lane writes its final `at` into `group_chain`, then a barrier, then `place_edge` and a barrier, then `mend_short_edges` on lane 0 and a barrier. Each hinge lane then computes the `crept` body for its own hinge. `before` is the clamp for hinge 0 and `group_edges[s][h − 1]` otherwise, and the shape is the last iteration's. It stores `direction_in_ferrule(shift, last_pose)` into `states[plastic_slot]`. With `share = creep_share(tuning.friction.w, dab.dt_seconds)` ≤ 0, it stores the loaded shift back unchanged, as `crept` returns `shape.plastic`.

- [ ] **Step 3: Run the equivalence test and the engine suite**

Run: `dk 'cargo test --release -p cadmium-engine --test bristles parallel_solve 2>&1 | tail -20'`, then the whole engine suite as in Task 6.
Expected: PASS; no new failures.

- [ ] **Step 4: Commit**

```bash
git add -A crates/cadmium-engine
git commit -m "feat(engine): plastic rest shift in the grouped bristle solve"
```

---

### Task 8: Bring the phase 2 tests back

**Files:**
- Modify: `tests/bristles/friction.rs`, `tests/bristles/look.rs`, `tests/bristles/sizes.rs`, `tests/engine_shapes.rs`

- [ ] **Step 1: Remove the eight `#[ignore = "phase 2 of #62 …"]` lines**

Run: `grep -rn 'ignore = "phase 2 of #62' crates/cadmium-engine/tests`
Expected: 8 hits. Delete each attribute line.

- [ ] **Step 2: Run them**

Run: `dk 'cargo test --release -p cadmium-engine --test bristles friction:: look::an_s_curve sizes::the_largest_brush_pressed 2>&1 | tail -40'` and `dk 'cargo test --release -p cadmium-engine --test engine_shapes broadside 2>&1 | tail -20'`
Expected: PASS.

- [ ] **Step 3: Triage what fails**

For each failing test, find out whether the physics or the test is wrong, with numbers (print the measured values once):
- **Physics or calibration:** the [U] constants (`PLASTIC_YIELD_RAD`, `STATIC_FRICTION`, `KINETIC_FRICTION`) may move within their cited ranges (μ 0.3–0.5, shift cap 0.05–0.10 rad, yield up to 0.05 rad). Any change goes in the report with the before and after numbers.
- **The test encodes the old model:** do not change the assertion. Report it with the reason and the measured numbers, and leave it failing so the controller decides.
- Never widen a tolerance to make a test pass.

- [ ] **Step 4: Run the whole workspace**

Run: `dk 'cargo test --release --workspace 2>&1 | grep -E "^test result|FAILED|panicked"'` (the app crate needs a display: `dk 'xvfb-run -a cargo test --release --workspace --features cadmium-app/smoke 2>&1 | grep -E "^test result|FAILED|panicked"'`).
Expected: 0 failed.

- [ ] **Step 5: Commit**

```bash
git add -A crates/cadmium-engine
git commit -m "test(engine): bristle friction and plasticity tests run again"
```

---

### Task 9: Measure, sheets and results

**Files:**
- Modify: `docs/superpowers/specs/2026-11-20-bristle-friction-plasticity-design.md` (a "Results" section)

- [ ] **Step 1: Gate and stage runs**

Read `docs/superpowers/baselines/README.md` and run the full `bristle_cost` gate exactly as it says (5 interleaved repeats after a warm-up), against the baseline. Run it once with `CADMIUM_BENCH_STAGES=1` for µs/dab per stage, too. Compare against phase 1's head (`perf/strand-parallel-solve`, commit `bdd0862`) with interleaved prebuilt binaries, A B B A.

- [ ] **Step 2: Sheets**

Generate the before/after sheet for every factory brush with the sheet tool the README names (before: `bdd0862`; after: this branch). Save the images under `docs/superpowers/sheets/2026-11-20-friction/` if the README's convention keeps sheets in the repo; otherwise in the scratchpad, and report their paths.

- [ ] **Step 3: Write the results**

Append to the spec:

```markdown
## Results

| scenario | baseline mean / p95 | phase 1 | phase 2 |
|---|---|---|---|
| … one row per gate scenario, ms … |

Bristles stage at default size: <phase 1> → <phase 2> µs/dab.
Proposed phase 2 budget: <measured figure> over the baseline.
```

Fill in the measured numbers only; leave no placeholder.

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers
git commit -m "docs: results of bristle friction and plasticity"
```
