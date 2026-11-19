# XPBD Bristles — Phase 0 (Baseline) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Before the bristle physics is rewritten (#62), record what today's engine does
and costs. This gives later phases something to measure against:
- invariant tests;
- analytic oracles from the literature;
- a recorded GPU-cost baseline with a pass/fail check;
- a tool that renders and compares example sheets for every factory brush.

**Architecture:** Nothing in the engine's behaviour changes.
- New invariant tests go in `crates/cadmium-engine/tests/bristles/invariants.rs`.
- A pure, unit-tested `mechanics` module in `cadmium-engine` holds the closed-form physics
  (beam bending, elastocapillary coalescence, hair count) that phases 1–3 will build on
  and test against.
- The `bristle_cost` example learns to write its results as TSV and to fail when a run is
  more than 10% slower than a recorded baseline.
- A new `brush_sheets` example in `cadmium-studio` renders the brush editor's example
  sheet for every factory brush and builds a side-by-side HTML page from two renders.

**Tech Stack:** Rust 2024, wgpu compute (WGSL), `png` crate (already a dev-dependency of
both crates).

Spec: `docs/superpowers/specs/2026-11-16-xpbd-bristles-design.md`, section 5.

---

## Ground rules for every task

- **Branch:** `feat/xpbd-phase0`, created from `docs/xpbd-bristles-spec` in Task 0.
- **Running cargo:** every cargo command runs inside the `cadmium-dev` Docker image, never
  on the host. The host lacks the GTK dev packages, and the GPU is passed through to the
  container. From the repo root:

  ```bash
  DOCKER="docker run --rm --device /dev/dri --group-add $(stat -c %g /dev/dri/renderD128) --group-add $(stat -c %g /dev/dri/card* | head -1) --security-opt label=disable -u $(id -u):$(id -g) -v $PWD:$PWD -w $PWD -v $HOME/.cargo:/cargo -e CARGO_HOME=/cargo -e HOME=/tmp cadmium-dev"
  $DOCKER cargo test --release -p cadmium-engine --test bristles invariants
  ```

  The Bash tool needs `dangerouslyDisableSandbox: true` for Docker.
- **Lints:** CI runs `cargo clippy --workspace --all-targets --release -- -D warnings`
  with these limits from `clippy.toml`:
  - functions ≤ 40 lines;
  - cognitive complexity ≤ 10;
  - nesting ≤ 3;
  - ≤ 5 arguments.

  `.github/check-file-sizes.sh` rejects `.rs`/`.wgsl` files over 400 lines.
- **Code style:**
  - Repo convention puts unit tests in a sibling file: `foo.rs` ends with
    `#[cfg(test)] mod tests;`, and the tests live in `foo/tests.rs`.
  - Name constants; no magic numbers.
  - Doc comments state what a thing is, in plain sentences.
- **Commit messages:** short conventional subject.
  - **No `Co-Authored-By`, no "Generated with Claude Code", no AI attribution of any kind**
    (the user's global rule overrides any harness default).

## File structure

| File | Status | Responsibility |
|---|---|---|
| `crates/cadmium-engine/tests/bristles/invariants.rs` | create (exists untracked as a draft; replace its content) | Model-independent bristle behaviours from the spec, §5 |
| `crates/cadmium-engine/tests/bristles/main.rs` | modify | Register `mod invariants;` (already added in the working tree) |
| `crates/cadmium-engine/tests/bristles/clumping.rs` | revert | Drop the uncommitted light-touch test; it moves to `invariants.rs` |
| `crates/cadmium-engine/src/mechanics.rs` | create | Closed-form mechanics in SI units: beam bending, elastocapillarity, hair count, px↔m |
| `crates/cadmium-engine/src/mechanics/tests.rs` | create | Checks the formulas against the numbers in the papers |
| `crates/cadmium-engine/src/lib.rs` | modify | `pub mod mechanics;` |
| `crates/cadmium-engine/examples/bristle_cost/timing.rs` | modify | `report` returns a `Measured` row |
| `crates/cadmium-engine/examples/bristle_cost/record.rs` | create | Write and read TSV results; list scenarios over budget |
| `crates/cadmium-engine/examples/bristle_cost/main.rs` | modify | `CADMIUM_BENCH_OUT`, `CADMIUM_BENCH_BASELINE` |
| `docs/superpowers/baselines/2026-11-16-bristle-cost.tsv` | create (generated) | Recorded cost baseline |
| `crates/cadmium-studio/examples/brush_sheets/main.rs` | create | `render` and `compare` commands |
| `crates/cadmium-studio/examples/brush_sheets/page.rs` | create | Side-by-side HTML page |
| `docs/superpowers/baselines/README.md` | create | What the baselines are and how to regenerate and compare them |

---

### Task 0: Branch and clean working tree

**Files:**
- Revert: `crates/cadmium-engine/tests/bristles/clumping.rs`

The working tree on `docs/xpbd-bristles-spec` holds three uncommitted changes:
1. a light-touch test appended to `clumping.rs`;
2. a draft `tests/bristles/invariants.rs`;
3. `mod invariants;` added to `tests/bristles/main.rs`.

Keep 2 and 3; Task 1 rewrites the draft. Revert 1; Task 1 re-adds the test in
`invariants.rs`.

- [ ] **Step 1: Create the branch and revert `clumping.rs`**

```bash
git switch -c feat/xpbd-phase0
git checkout -- crates/cadmium-engine/tests/bristles/clumping.rs
git status --short
```

Expected output, exactly:
```
 M crates/cadmium-engine/tests/bristles/main.rs
?? crates/cadmium-engine/tests/bristles/invariants.rs
```
(The plan file itself is committed before execution starts, so it does not show.)

- [ ] **Step 2: Check that the shaders match `main`**

```bash
git diff main -- crates/cadmium-engine/src/shaders/ | wc -l
```

Expected: `0`.

---

### Task 1: Invariant tests

**Files:**
- Create (overwrite the draft): `crates/cadmium-engine/tests/bristles/invariants.rs`
- Modify: `crates/cadmium-engine/tests/bristles/main.rs` (`mod invariants;` sits after `mod frame;`; check it is there)

These tests describe what any faithful brush model must do. Three already pass on
today's model and guard against regressions. The light-touch test fails today; the
investigation in #62 found the cause. It is committed as `#[ignore]` with the reason, so
phase 1 can enable it.

- [ ] **Step 1: Write `invariants.rs`**

```rust
//! Behaviours the bristle physics must show whatever the model, from the literature the
//! XPBD rewrite is grounded in (docs/superpowers/specs/2026-11-16-xpbd-bristles-design.md,
//! section 5). A test today's model fails is ignored and names the phase of #62 that must
//! make it pass.

use cadmium_engine::brush::factory_brushes;
use cadmium_engine::canvas::StateTexture;
use cadmium_engine::engine::BristleSnapshot;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::StrokeSample;

use crate::support::{frame, session};

const ROW_Y: f64 = 128.0;
const FROM_X: f64 = 100.0;
const LENGTH_PX: f64 = 240.0;
const PEN_SPEED_PX_PER_S: f64 = 400.0;
const MS_PER_S: f64 = 1000.0;
const TILT_DEG: f32 = 20.0;
const STEADY_PRESSURE: f32 = 0.6;
const COARSE_RATE_HZ: f64 = 120.0;
const FINE_RATE_HZ: f64 = 480.0;
const MAX_RELATIVE_GAP: f64 = 0.1;
const VISIBLE_UM: f32 = 0.5;
/// Columns this far from either end of the line are left out of the mark's width.
const END_MARGIN_PX: usize = 40;
const SPLAY_PRESSURES: [f32; 4] = [0.15, 0.35, 0.6, 0.9];
const DENSITY_SCALE: f32 = 2.0;

const LIGHT_TOUCH_PRESSURE: f32 = 0.04;
const LIGHT_TOUCH_TILT_DEG: f32 = 25.0;
const LIGHT_TOUCH_DIAMETER_PX: f32 = 24.0;
const LIGHT_TOUCH_SAMPLES: u32 = 12;
const LIGHT_TOUCH_STEP_PX: f64 = 2.0;
const LIGHT_TOUCH_INTERVAL_MS: u32 = 6;
const POINTED_BRUSHES: [&str; 2] = ["Round · Synthetic", "Round · Kolinsky Sable"];

/// A straight line at `pressure`, sampled at `rate_hz` while the pen moves at a steady speed.
fn line_at_rate(pressure: f32, rate_hz: f64) -> Vec<StrokeSample> {
    let seconds = LENGTH_PX / PEN_SPEED_PX_PER_S;
    let samples = (seconds * rate_hz).round() as u32;
    (0..=samples)
        .map(|index| {
            let t = f64::from(index) / f64::from(samples);
            StrokeSample {
                x: FROM_X + LENGTH_PX * t,
                y: ROW_Y,
                pressure,
                tilt_x_deg: TILT_DEG,
                tilt_y_deg: 0.0,
                time_ms: (t * seconds * MS_PER_S).round() as u32,
            }
        })
        .collect()
}

/// Feeds `samples` as one stroke, one frame per sample, the way the app feeds the pen,
/// and leaves the pen down.
fn press(gpu: &GpuContext, s: &mut PaintSession, samples: &[StrokeSample]) {
    for (index, sample) in samples.iter().enumerate() {
        if index == 0 {
            s.begin_stroke(*sample);
        } else {
            s.add_samples(&[*sample]);
        }
        frame(gpu, s);
    }
}

/// Paints `samples` as one stroke and lifts the pen.
fn paint(gpu: &GpuContext, s: &mut PaintSession, samples: &[StrokeSample]) {
    press(gpu, s, samples);
    s.end_stroke();
    frame(gpu, s);
}

/// Mean count of visibly painted pixels per column, away from the line's ends.
fn mean_mark_width_px(gpu: &GpuContext, s: &PaintSession) -> f64 {
    let volume = s.canvas.read_f32(gpu, StateTexture::WetVol).unwrap();
    let width = s.canvas.spec.width as usize;
    let first = FROM_X as usize + END_MARGIN_PX;
    let last = (FROM_X + LENGTH_PX) as usize - END_MARGIN_PX;
    let total: usize = (first..=last)
        .map(|x| {
            let column = volume.iter().skip(x).step_by(width);
            column.filter(|&&v| v > VISIBLE_UM).count()
        })
        .sum();
    total as f64 / (last - first + 1) as f64
}

/// Mean distance of the tips from the roots' centre, in the canvas plane.
fn mean_splay_px(snapshot: &BristleSnapshot) -> f32 {
    let count = snapshot.roots_px.len() as f32;
    let centre = snapshot.roots_px.iter().fold([0.0, 0.0], |sum, root| {
        [sum[0] + root[0] / count, sum[1] + root[1] / count]
    });
    let total: f32 = snapshot
        .tips_px
        .iter()
        .map(|tip| (tip[0] - centre[0]).hypot(tip[1] - centre[1]))
        .sum();
    total / count
}

/// Spread of the tips across the stroke (along y), from the outermost tip on each side.
fn tip_spread_across_px(snapshot: &BristleSnapshot) -> f32 {
    let (low, high) = snapshot
        .tips_px
        .iter()
        .fold((f32::MAX, f32::MIN), |(low, high), tip| {
            (low.min(tip[1]), high.max(tip[1]))
        });
    high - low
}

fn relative_gap(a: f64, b: f64) -> f64 {
    (a - b).abs() / a.abs().max(b.abs()).max(f64::MIN_POSITIVE)
}

fn splay_while_pressed(gpu: &GpuContext, pressure: f32) -> f32 {
    let mut s = session(gpu);
    press(gpu, &mut s, &line_at_rate(pressure, COARSE_RATE_HZ));
    mean_splay_px(&s.engine.bristle_snapshot(gpu).unwrap())
}

fn mark_width_px(gpu: &GpuContext, rate_hz: f64, density_scale: f32) -> f64 {
    let mut s = session(gpu);
    s.brush.head.density_per_px *= density_scale;
    s.reload_brush(gpu);
    paint(gpu, &mut s, &line_at_rate(STEADY_PRESSURE, rate_hz));
    mean_mark_width_px(gpu, &s)
}

fn light_touch_sample(index: u32) -> StrokeSample {
    StrokeSample {
        x: FROM_X + LIGHT_TOUCH_STEP_PX * f64::from(index),
        y: ROW_Y,
        pressure: LIGHT_TOUCH_PRESSURE,
        tilt_x_deg: LIGHT_TOUCH_TILT_DEG,
        tilt_y_deg: 0.0,
        time_ms: index * LIGHT_TOUCH_INTERVAL_MS,
    }
}

/// The tips' spread across the stroke after its first dab and after its last.
fn light_touch_spreads(gpu: &GpuContext, s: &mut PaintSession) -> (f32, f32) {
    s.limit_dabs_per_frame(Some(1));
    s.begin_stroke(light_touch_sample(0));
    frame(gpu, s);
    let first = tip_spread_across_px(&s.engine.bristle_snapshot(gpu).unwrap());
    for index in 1..LIGHT_TOUCH_SAMPLES {
        s.add_samples(&[light_touch_sample(index)]);
        while s.has_queued_dabs() {
            frame(gpu, s);
        }
    }
    let last = tip_spread_across_px(&s.engine.bristle_snapshot(gpu).unwrap());
    (first, last)
}

#[test]
fn harder_pressure_splays_the_tips_wider() {
    let gpu = GpuContext::new().unwrap();
    let splays: Vec<f32> = SPLAY_PRESSURES
        .iter()
        .map(|&pressure| splay_while_pressed(&gpu, pressure))
        .collect();
    assert!(
        splays.windows(2).all(|pair| pair[1] > pair[0]),
        "splay at pressures {SPLAY_PRESSURES:?}: {splays:?}"
    );
}

#[test]
fn the_mark_does_not_depend_on_how_often_the_pen_is_sampled() {
    let gpu = GpuContext::new().unwrap();
    let coarse = mark_width_px(&gpu, COARSE_RATE_HZ, 1.0);
    let fine = mark_width_px(&gpu, FINE_RATE_HZ, 1.0);
    assert!(
        relative_gap(coarse, fine) <= MAX_RELATIVE_GAP,
        "mark width {coarse} px at {COARSE_RATE_HZ} Hz, {fine} px at {FINE_RATE_HZ} Hz"
    );
}

#[test]
fn the_mark_does_not_depend_on_how_many_hairs_are_simulated() {
    let gpu = GpuContext::new().unwrap();
    let base = mark_width_px(&gpu, COARSE_RATE_HZ, 1.0);
    let denser = mark_width_px(&gpu, COARSE_RATE_HZ, DENSITY_SCALE);
    assert!(
        relative_gap(base, denser) <= MAX_RELATIVE_GAP,
        "mark width {base} px, {denser} px with {DENSITY_SCALE}x hair density"
    );
}

#[test]
#[ignore = "phase 1 of #62: each stroke hangs the hairs into one point and the first clump step flings them apart"]
fn a_light_first_touch_lands_the_wet_point_instead_of_splaying_the_hairs() {
    let gpu = GpuContext::new().unwrap();
    for brush in factory_brushes()
        .into_iter()
        .filter(|brush| POINTED_BRUSHES.contains(&brush.name.as_str()))
    {
        let mut s = session(&gpu);
        s.brush.wear(&brush);
        s.brush.geometry.max_diameter_px = LIGHT_TOUCH_DIAMETER_PX;
        s.reload_brush(&gpu);
        s.flush(&gpu);
        let (first_px, later_px) = light_touch_spreads(&gpu, &mut s);
        assert!(
            first_px <= later_px,
            "{}: the first dab splays the tips {first_px} px across, the stroke then holds them to {later_px} px",
            brush.name
        );
    }
}
```

- [ ] **Step 2: Run the invariants**

Run: `$DOCKER cargo test --release -p cadmium-engine --test bristles invariants`

Expected: `test result: ok. 3 passed; 0 failed; 1 ignored`. These three passed on today's
model while the plan was being written (pressure splay, sampling rate, resolution).

- [ ] **Step 3: Check that the ignored test fails for the recorded reason**

Run: `$DOCKER cargo test --release -p cadmium-engine --test bristles invariants -- --ignored`

Expected: FAIL with a message like
`Round · Synthetic: the first dab splays the tips 15.7… px across, the stroke then holds them to 10.4… px`.

If it passes instead, stop and report: the bug would no longer reproduce.

- [ ] **Step 4: Clippy on the test target**

Run: `$DOCKER cargo clippy -p cadmium-engine --test bristles --release -- -D warnings`

Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/cadmium-engine/tests/bristles/invariants.rs crates/cadmium-engine/tests/bristles/main.rs
git commit -m "test(engine): bristle invariants from the XPBD spec"
```

---

### Task 2: `mechanics` module (closed-form oracles)

**Files:**
- Create: `crates/cadmium-engine/src/mechanics.rs`
- Create: `crates/cadmium-engine/src/mechanics/tests.rs`
- Modify: `crates/cadmium-engine/src/lib.rs` (add `pub mod mechanics;` after `pub mod history;`)

These formulas let phases 1–3 compute compliances from material constants and give the
GPU tests their analytic expectations. All in SI units, `f64`.

Sources (all in `docs/superpowers/specs/2026-11-16-xpbd-bristles-literature.md`):
- Euler-Bernoulli cantilever: δ = FL³/(3EI), I = πr⁴/4.
- Bico et al. 2004:
  - L_EC = √(κ/γ);
  - a pair sticks when L⁴ = (9/2)d²L_EC²;
  - a cluster of N closes when L⁴ = (9/16)N³d²L_EC².
- Hair count in a ferrule: N = φ·πR²/(πd²/4) = 4φR²/d².

- [ ] **Step 1: Write the failing tests** in `crates/cadmium-engine/src/mechanics/tests.rs`

```rust
use super::*;

const NYLON_MODULUS_PA: f64 = 3.5e9;
const NYLON_RADIUS_M: f64 = 50e-6;
const HAIR_LENGTH_M: f64 = 15e-3;
const TIP_FORCE_N: f64 = 1e-3;
/// Bico et al. 2004: polyester lamellae 100 µm thick in silicone oil.
const BICO_STIFFNESS_N_M: f64 = 5.1e-4;
const BICO_SURFACE_TENSION_N_PER_M: f64 = 20.6e-3;
const BICO_SPACING_M: f64 = 1e-3;
const TOLERANCE: f64 = 1e-3;

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= TOLERANCE * expected.abs()
}

#[test]
fn a_tenth_of_a_millimetre_nylon_hair_has_the_bending_stiffness_of_the_literature() {
    // The literature review derives EI ≈ 1.7e-8 N·m² for d = 0.1 mm, E = 3.5 GPa.
    let stiffness = bending_stiffness(NYLON_MODULUS_PA, NYLON_RADIUS_M);
    assert!(close(stiffness, 1.718e-8), "{stiffness}");
}

#[test]
fn a_cantilever_tip_load_and_its_deflection_agree_with_the_beam_stiffness() {
    let stiffness = bending_stiffness(NYLON_MODULUS_PA, NYLON_RADIUS_M);
    let deflection = cantilever_tip_deflection(TIP_FORCE_N, HAIR_LENGTH_M, stiffness);
    let spring_n_per_m = 3.0 * stiffness / HAIR_LENGTH_M.powi(3);
    assert!(close(deflection * spring_n_per_m, TIP_FORCE_N), "{deflection}");
}

#[test]
fn the_elastocapillary_length_of_bicos_lamellae_is_fifteen_centimetres() {
    // Bico et al. report L_EC ≈ 150 mm for these lamellae.
    let length = elastocapillary_length(BICO_STIFFNESS_N_M, BICO_SURFACE_TENSION_N_PER_M);
    assert!(close(length, 0.157_34), "{length}");
}

#[test]
fn a_cluster_of_two_closes_at_bicos_pair_length() {
    let l_ec = elastocapillary_length(BICO_STIFFNESS_N_M, BICO_SURFACE_TENSION_N_PER_M);
    let pair = (4.5 * BICO_SPACING_M.powi(2) * l_ec.powi(2)).powf(0.25);
    assert!(close(coalescence_length(2, BICO_SPACING_M, l_ec), pair));
}

#[test]
fn larger_clusters_need_longer_hairs_to_close() {
    let l_ec = elastocapillary_length(BICO_STIFFNESS_N_M, BICO_SURFACE_TENSION_N_PER_M);
    let lengths: Vec<f64> = [2, 4, 8, 16]
        .iter()
        .map(|&hairs| coalescence_length(hairs, BICO_SPACING_M, l_ec))
        .collect();
    assert!(lengths.windows(2).all(|pair| pair[1] > pair[0]), "{lengths:?}");
}

#[test]
fn a_forty_pixel_ferrule_holds_about_one_hundred_eighty_tenth_millimetre_hairs() {
    let radius_m = metres_from_px(20.0);
    let hairs = real_hair_count(0.7, radius_m, 2.0 * NYLON_RADIUS_M);
    assert!(close(hairs, 179.2), "{hairs}");
}

#[test]
fn a_pixel_is_forty_micrometres() {
    assert!(close(metres_from_px(25.0), 1e-3));
}
```

- [ ] **Step 2: Run them and check they fail to compile**

Add `pub mod mechanics;` to `crates/cadmium-engine/src/lib.rs`, after `pub mod history;`.
Create `crates/cadmium-engine/src/mechanics.rs` containing only:

```rust
#[cfg(test)]
mod tests;
```

Run: `$DOCKER cargo test --release -p cadmium-engine --lib mechanics`

Expected: compile errors `cannot find function bending_stiffness` (and the others).

- [ ] **Step 3: Implement** `crates/cadmium-engine/src/mechanics.rs`

```rust
//! Closed-form mechanics of hairs, in SI units: the analytic results the bristle simulation
//! takes its constants from and is tested against. Sources are listed in
//! docs/superpowers/specs/2026-11-16-xpbd-bristles-literature.md.

use crate::engine::RELIEF_UM_PER_PX;

const METRES_PER_MICROMETRE: f64 = 1e-6;
/// Bico et al. 2004, eq. 2: a cluster of N hairs closes when L⁴ = (9/16)·N³·d²·L_EC².
const CLUSTER_CLOSING_FACTOR: f64 = 9.0 / 16.0;

/// Length in metres of `px` canvas pixels, at the scale the canvas relief uses.
pub fn metres_from_px(px: f64) -> f64 {
    px * f64::from(RELIEF_UM_PER_PX) * METRES_PER_MICROMETRE
}

/// Second moment of area of a round hair of `radius_m`: πr⁴/4.
pub fn second_moment_of_area(radius_m: f64) -> f64 {
    std::f64::consts::FRAC_PI_4 * radius_m.powi(4)
}

/// Bending stiffness EI, in N·m², of a round hair of `radius_m` and Young's modulus `modulus_pa`.
pub fn bending_stiffness(modulus_pa: f64, radius_m: f64) -> f64 {
    modulus_pa * second_moment_of_area(radius_m)
}

/// How far, in metres, a clamped hair of `length_m` and stiffness `stiffness_n_m2` bends
/// under `force_n` at its tip (Euler-Bernoulli): FL³/(3EI).
pub fn cantilever_tip_deflection(force_n: f64, length_m: f64, stiffness_n_m2: f64) -> f64 {
    force_n * length_m.powi(3) / (3.0 * stiffness_n_m2)
}

/// Elastocapillary length √(κ/γ), in metres (Bico et al. 2004).
pub fn elastocapillary_length(stiffness_n_m2: f64, surface_tension_n_per_m: f64) -> f64 {
    (stiffness_n_m2 / surface_tension_n_per_m).sqrt()
}

/// The free length, in metres, at which a cluster of `hairs` hairs set `spacing_m` apart
/// closes: L = ((9/16)·N³·d²·L_EC²)^¼ (Bico et al. 2004, eq. 2; N = 2 is their eq. 1).
pub fn coalescence_length(hairs: u32, spacing_m: f64, elastocapillary_m: f64) -> f64 {
    let n = f64::from(hairs);
    (CLUSTER_CLOSING_FACTOR * n.powi(3) * spacing_m.powi(2) * elastocapillary_m.powi(2))
        .powf(0.25)
}

/// How many hairs of `hair_diameter_m` fill a ferrule of `ferrule_radius_m` at packing
/// fraction `packing`: φ·πR² / (πd²/4).
pub fn real_hair_count(packing: f64, ferrule_radius_m: f64, hair_diameter_m: f64) -> f64 {
    4.0 * packing * ferrule_radius_m.powi(2) / hair_diameter_m.powi(2)
}

#[cfg(test)]
mod tests;
```

- [ ] **Step 4: Run the tests**

Run: `$DOCKER cargo test --release -p cadmium-engine --lib mechanics`

Expected: `test result: ok. 7 passed`.

A failing value check is a mistake in the formula, not in the paper number. Fix the
formula; do not loosen the expected value.

- [ ] **Step 5: Clippy**

Run: `$DOCKER cargo clippy -p cadmium-engine --lib --release -- -D warnings`

Expected: no warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/cadmium-engine/src/mechanics.rs crates/cadmium-engine/src/mechanics/tests.rs crates/cadmium-engine/src/lib.rs
git commit -m "feat(engine): closed-form hair mechanics for the XPBD bristles"
```

---

### Task 3: `bristle_cost` writes results and checks a baseline

**Files:**
- Modify: `crates/cadmium-engine/examples/bristle_cost/timing.rs` (functions `report`, `report_quarters`)
- Create: `crates/cadmium-engine/examples/bristle_cost/record.rs`
- Modify: `crates/cadmium-engine/examples/bristle_cost/main.rs`

Behaviour after this task:
- `CADMIUM_BENCH_OUT=<file>` writes one TSV row per scenario, without the quarter rows.
- `CADMIUM_BENCH_BASELINE=<file>` compares each scenario with the same name in the file.
  It prints every scenario whose mean or p95 is more than 10% slower and then exits with
  status 1. GPU time is used when both runs have it, wall time otherwise.

- [ ] **Step 1: Make `report` return what it printed.** In `timing.rs`, replace
  `pub(crate) fn report(...)` with:

```rust
/// One scenario's frame costs, summarised: mean and 95th percentile, in ms.
pub(crate) struct Measured {
    pub(crate) name: String,
    pub(crate) frames: usize,
    pub(crate) gpu: Option<(f64, f64)>,
    pub(crate) wall: (f64, f64),
}

fn measured(name: &str, costs: &[FrameCost]) -> Measured {
    let gpu: Option<Vec<f64>> = costs.iter().map(|cost| cost.gpu_ms).collect();
    Measured {
        name: name.to_string(),
        frames: costs.len(),
        gpu: gpu.map(mean_and_percentile),
        wall: mean_and_percentile(costs.iter().map(|cost| cost.wall_ms).collect()),
    }
}

pub(crate) fn report(name: &str, costs: &[FrameCost]) -> Measured {
    let row = measured(name, costs);
    let gpu = row.gpu.map_or_else(
        || format!("{:>9} {:>9}", "-", "-"),
        |(mean, p95)| format!("{mean:>9.2} {p95:>9.2}"),
    );
    let (wall_mean, wall_p95) = row.wall;
    println!(
        "{name:<36} {:>6} {gpu} {wall_mean:>9.2} {wall_p95:>9.2}",
        row.frames
    );
    row
}
```

`report_quarters` keeps calling `report(name, chunk);` and ignores the returned row.

- [ ] **Step 2: Create `record.rs`**

```rust
//! A run's results as tab-separated values, and the check of a run against a recorded one.

use std::fmt::Write as _;
use std::path::Path;

use crate::timing::Measured;

pub(crate) const OUT_VARIABLE: &str = "CADMIUM_BENCH_OUT";
pub(crate) const BASELINE_VARIABLE: &str = "CADMIUM_BENCH_BASELINE";
/// How much slower than the baseline a scenario may get: the budget of the XPBD rewrite (#62).
pub(crate) const ALLOWED_SLOWDOWN: f64 = 1.10;
const HEADER: &str = "scenario\tframes\tgpu_mean_ms\tgpu_p95_ms\twall_mean_ms\twall_p95_ms";
const NO_VALUE: &str = "-";

pub(crate) fn write(path: &Path, adapter: &str, rows: &[Measured]) -> std::io::Result<()> {
    let mut text = format!("# adapter: {adapter}\n{HEADER}\n");
    for row in rows {
        let (gpu_mean, gpu_p95) = row.gpu.map_or_else(
            || (NO_VALUE.to_string(), NO_VALUE.to_string()),
            |(mean, p95)| (format!("{mean:.4}"), format!("{p95:.4}")),
        );
        let (wall_mean, wall_p95) = row.wall;
        let _ = writeln!(
            text,
            "{}\t{}\t{gpu_mean}\t{gpu_p95}\t{wall_mean:.4}\t{wall_p95:.4}",
            row.name, row.frames
        );
    }
    std::fs::write(path, text)
}

pub(crate) fn read(path: &Path) -> std::io::Result<Vec<Measured>> {
    let text = std::fs::read_to_string(path)?;
    Ok(text
        .lines()
        .filter(|line| !line.starts_with('#') && *line != HEADER)
        .filter_map(parse_row)
        .collect())
}

fn parse_row(line: &str) -> Option<Measured> {
    let fields: Vec<&str> = line.split('\t').collect();
    let [name, frames, gpu_mean, gpu_p95, wall_mean, wall_p95] = fields.as_slice() else {
        return None;
    };
    Some(Measured {
        name: (*name).to_string(),
        frames: frames.parse().ok()?,
        gpu: gpu_mean.parse().ok().zip(gpu_p95.parse().ok()),
        wall: (wall_mean.parse().ok()?, wall_p95.parse().ok()?),
    })
}

/// The scenarios measured in both runs that got slower than `ALLOWED_SLOWDOWN` allows,
/// judged on GPU time when both runs have it and on wall time otherwise.
pub(crate) fn over_budget(baseline: &[Measured], current: &[Measured]) -> Vec<String> {
    current
        .iter()
        .filter_map(|now| {
            let before = baseline.iter().find(|row| row.name == now.name)?;
            slower_than(before, now)
        })
        .collect()
}

fn slower_than(before: &Measured, now: &Measured) -> Option<String> {
    let (then, current, clock) = match (before.gpu, now.gpu) {
        (Some(then), Some(current)) => (then, current, "gpu"),
        _ => (before.wall, now.wall, "wall"),
    };
    let slower =
        current.0 > then.0 * ALLOWED_SLOWDOWN || current.1 > then.1 * ALLOWED_SLOWDOWN;
    slower.then(|| {
        format!(
            "{}: {clock} mean {:.2} -> {:.2} ms, p95 {:.2} -> {:.2} ms",
            now.name, then.0, current.0, then.1, current.1
        )
    })
}
```

- [ ] **Step 3: Wire it in `main.rs`**
  - Add `mod record;` beside the other modules.
  - Add `use std::path::Path;`.
  - Import `Measured` from `timing`.
  - Extend the module doc comment with these two lines, placed after the `CADMIUM_PACED`
    line:

```rust
//! `CADMIUM_BENCH_OUT=<file>` also writes each scenario's results to the file as
//! tab-separated values; `CADMIUM_BENCH_BASELINE=<file>` compares them with a file written
//! that way and exits with status 1 when a scenario got more than 10% slower.
```

  Replace `fn main` with these three functions:

```rust
fn measure_all(gpu: &GpuContext, timestamps: Option<&Timestamps>) -> Vec<Measured> {
    println!(
        "{:<36} {:>6} {:>9} {:>9} {:>9} {:>9}",
        "scenario (ms per frame)", "frames", "gpu mean", "gpu p95", "wall mean", "wall p95"
    );
    let mut rows = Vec::new();
    for scenario in SCENARIOS.iter().filter(|scenario| selected(scenario)) {
        let costs = measure(gpu, timestamps, scenario);
        rows.push(report(scenario.name, &costs));
        if scenario.motion == Motion::LongStroke {
            report_quarters(&costs);
        }
    }
    rows
}

fn check_against(path: &Path, rows: &[Measured]) {
    let baseline = record::read(path).expect("the baseline file is readable");
    let slower = record::over_budget(&baseline, rows);
    if slower.is_empty() {
        println!(
            "within {:.0}% of the baseline",
            (record::ALLOWED_SLOWDOWN - 1.0) * 100.0
        );
        return;
    }
    for line in &slower {
        println!("over budget: {line}");
    }
    std::process::exit(1);
}

fn main() {
    let gpu = GpuContext::new().expect("a Vulkan adapter to measure on");
    let info = gpu.adapter.get_info();
    let timestamps = timestamps(&gpu);
    println!(
        "adapter: {} ({:?}, {})",
        info.name, info.device_type, info.driver_info
    );
    println!(
        "GPU time: {}",
        if timestamps.is_some() {
            "timestamp queries"
        } else {
            "unavailable, wall clock only"
        }
    );
    let rows = measure_all(&gpu, timestamps.as_ref());
    if let Some(path) = std::env::var_os(record::OUT_VARIABLE) {
        record::write(Path::new(&path), &info.name, &rows).expect("the results file is writable");
    }
    if let Some(path) = std::env::var_os(record::BASELINE_VARIABLE) {
        check_against(Path::new(&path), &rows);
    }
}
```

- [ ] **Step 4: Try it on one scenario**

```bash
mkdir -p target/bench
$DOCKER env CADMIUM_SCENARIO="brush, default size" CADMIUM_BENCH_OUT=target/bench/try.tsv cargo run --release -p cadmium-engine --example bristle_cost
cat target/bench/try.tsv
$DOCKER env CADMIUM_SCENARIO="brush, default size" CADMIUM_BENCH_BASELINE=target/bench/try.tsv cargo run --release -p cadmium-engine --example bristle_cost; echo "exit $?"
```

Expected:
- the file has `# adapter: …`, the header and one row;
- the second run prints `within 10% of the baseline` and `exit 0`. A second run over
  budget is noise; rerun once.

Then check the failing path. Edit `target/bench/try.tsv` and change the GPU mean in the
row to `0.0001`. Rerun the second command. Expected: `over budget: brush, default size:
gpu mean 0.00 -> …` and `exit 1`.

- [ ] **Step 5: Clippy and file sizes**

Run:
```bash
$DOCKER cargo clippy -p cadmium-engine --examples --release -- -D warnings
.github/check-file-sizes.sh
```

Expected: no output from either.

- [ ] **Step 6: Commit**

```bash
git add crates/cadmium-engine/examples/bristle_cost/
git commit -m "feat(bench): record bristle_cost results and check them against a baseline"
```

---

### Task 4: Record the cost baseline

**Files:**
- Create: `docs/superpowers/baselines/2026-11-16-bristle-cost.tsv`

- [ ] **Step 1: Run the full bench twice**

The second run checks that the first is reproducible within the budget.

```bash
mkdir -p docs/superpowers/baselines
$DOCKER env CADMIUM_BENCH_OUT=docs/superpowers/baselines/2026-11-16-bristle-cost.tsv cargo run --release -p cadmium-engine --example bristle_cost
$DOCKER env CADMIUM_BENCH_BASELINE=docs/superpowers/baselines/2026-11-16-bristle-cost.tsv cargo run --release -p cadmium-engine --example bristle_cost; echo "exit $?"
```

Expected: the second run prints `within 10% of the baseline` and `exit 0`.

If a scenario is over budget on a repeat of the *same* code, the 10% gate is not usable
on this machine. Stop and report the scenario and its numbers; do not raise the
tolerance.

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/baselines/2026-11-16-bristle-cost.tsv
git commit -m "docs: bristle_cost baseline before the XPBD rewrite"
```

---

### Task 5: `brush_sheets` example

**Files:**
- Create: `crates/cadmium-studio/examples/brush_sheets/main.rs`
- Create: `crates/cadmium-studio/examples/brush_sheets/page.rs`

Two commands:
- `render <dir>` paints the brush editor's example sheet (S-curve plus pressure ramp from
  0.04 to 1, via `ExampleSheet`, exactly what the editor shows). It does this for every
  factory brush, writes `<dir>/NN.png`, and writes `<dir>/brushes.tsv` with `NN<TAB>name`
  lines.
- `compare <before dir> <after dir> <out dir>` copies both renders into `<out dir>/before`
  and `<out dir>/after`, then writes `<out dir>/index.html` showing them side by side,
  one row per brush.

Rendering is deterministic, so the same code on the same GPU gives the same PNGs.

- [ ] **Step 1: Write `main.rs`**

```rust
//! Renders the brush editor's example sheet for every factory brush, and compares two
//! renders side by side.
//!
//! cargo run --release -p cadmium-studio --example brush_sheets -- render <dir>
//! cargo run --release -p cadmium-studio --example brush_sheets -- compare <before> <after> <out>
//!
//! `render` writes `<dir>/NN.png` per brush and `<dir>/brushes.tsv` naming them. `compare`
//! copies two renders into `<out>/before` and `<out>/after` and writes `<out>/index.html`.

mod page;

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use cadmium_engine::brush::factory_brushes;
use cadmium_engine::canvas::{Primer, Weave};
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::params::SimParams;
use cadmium_studio::{ExampleRequest, ExampleSheet, StudioPaint, Thumbnail};

const BRUSHES_FILE: &str = "brushes.tsv";
const PAINT_SLOTS: [&str; 3] = ["titanium_white", "cadmium_yellow", "cadmium_red"];
const YELLOW_SHARE: f32 = 0.55;
const RED_SHARE: f32 = 0.45;
const FULL_LOAD: f32 = 1.0;
const MEDIUM: f32 = 0.1;

/// The paint every sheet is painted with: a cadmium orange, fully loaded, on linen.
fn sheet_paint() -> StudioPaint {
    StudioPaint {
        slots: PAINT_SLOTS.iter().map(|slot| slot.to_string()).collect(),
        primer: Primer::WhiteGesso,
        weave: Weave::Linen,
        recipe: vec![(1, YELLOW_SHARE), (2, RED_SHARE)],
        load: FULL_LOAD,
        medium: MEDIUM,
        params: SimParams::default(),
    }
}

fn write_png(path: &Path, shot: &Thumbnail) {
    let file = File::create(path).expect("the sheet file is writable");
    let mut encoder = png::Encoder::new(BufWriter::new(file), shot.width, shot.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("a PNG header");
    writer.write_image_data(&shot.rgba).expect("the sheet's pixels");
}

fn render(dir: &Path) {
    std::fs::create_dir_all(dir).expect("the output directory is creatable");
    let gpu = GpuContext::new().expect("a Vulkan adapter to paint on");
    let paint = sheet_paint();
    let mut sheet = ExampleSheet::new();
    let mut names = String::new();
    for (index, brush) in factory_brushes().iter().enumerate() {
        let request = ExampleRequest { brush, paint: &paint };
        let shot = sheet.paint(&gpu, request).expect("the sheet paints");
        write_png(&dir.join(format!("{index:02}.png")), &shot);
        names.push_str(&format!("{index:02}\t{}\n", brush.name));
        println!("{index:02} {}", brush.name);
    }
    std::fs::write(dir.join(BRUSHES_FILE), names).expect("the brush list is writable");
}

fn usage() -> ! {
    eprintln!("usage: brush_sheets render <dir> | compare <before> <after> <out>");
    std::process::exit(2);
}

fn main() {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    match args.as_slice() {
        [command, dir] if command.as_os_str() == "render" => render(dir),
        [command, before, after, out] if command.as_os_str() == "compare" => {
            page::compare(before, after, out);
        }
        _ => usage(),
    }
}
```

- [ ] **Step 2: Write `page.rs`**

```rust
//! The side-by-side page of two renders.

use std::fmt::Write as _;
use std::path::Path;

use crate::BRUSHES_FILE;

const BEFORE: &str = "before";
const AFTER: &str = "after";
const STYLE: &str = "body{margin:0;padding:16px;background:#ecebe6;color:#1f1d1a;\
font:14px system-ui,sans-serif}h1{font-size:20px}h2{font-size:15px;margin:20px 0 6px}\
.pair{display:grid;grid-template-columns:1fr 1fr;gap:8px}figure{margin:0}\
img{width:100%;height:auto;display:block;border:1px solid #d6d2c8}\
figcaption{font-size:12px;color:#6b665d}";

/// `NN` and brush name, one per rendered sheet, from a render's brush list.
fn brushes(dir: &Path) -> Vec<(String, String)> {
    let text = std::fs::read_to_string(dir.join(BRUSHES_FILE)).expect("a render's brush list");
    text.lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(index, name)| (index.to_string(), name.to_string()))
        .collect()
}

fn copy_render(from: &Path, to: &Path, sheets: &[(String, String)]) {
    std::fs::create_dir_all(to).expect("the copy's directory is creatable");
    for (index, _) in sheets {
        let file = format!("{index}.png");
        std::fs::copy(from.join(&file), to.join(&file)).expect("the sheet copies");
    }
}

fn row(html: &mut String, index: &str, name: &str) {
    let _ = write!(
        html,
        "<section><h2>{name}</h2><div class=\"pair\">\
         <figure><img src=\"{BEFORE}/{index}.png\" alt=\"{name}, before\"><figcaption>before</figcaption></figure>\
         <figure><img src=\"{AFTER}/{index}.png\" alt=\"{name}, after\"><figcaption>after</figcaption></figure>\
         </div></section>"
    );
}

/// Copies the `before` and `after` renders into `out` and writes `out/index.html`.
pub(crate) fn compare(before: &Path, after: &Path, out: &Path) {
    let sheets = brushes(before);
    copy_render(before, &out.join(BEFORE), &sheets);
    copy_render(after, &out.join(AFTER), &sheets);
    let mut html = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>Brush sheets</title>\
         <style>{STYLE}</style><h1>Brush sheets: before and after</h1>"
    );
    for (index, name) in &sheets {
        row(&mut html, index, name);
    }
    std::fs::write(out.join("index.html"), html).expect("the page is writable");
    println!("{}", out.join("index.html").display());
}
```

`BRUSHES_FILE` must be `pub(crate)` in `main.rs` for `page.rs` to use it. Change its
declaration to `pub(crate) const BRUSHES_FILE: &str = "brushes.tsv";`.

- [ ] **Step 3: Render twice and compare the renders**

Rendering the same code twice must give byte-identical sheets.

```bash
$DOCKER cargo run --release -p cadmium-studio --example brush_sheets -- render target/brush_sheets/a
$DOCKER cargo run --release -p cadmium-studio --example brush_sheets -- render target/brush_sheets/b
cmp -s target/brush_sheets/a/01.png target/brush_sheets/b/01.png && echo identical
$DOCKER cargo run --release -p cadmium-studio --example brush_sheets -- compare target/brush_sheets/a target/brush_sheets/b target/brush_sheets/page
ls target/brush_sheets/page
```

Expected:
- 15 lines `NN Brush · Name` per render;
- `identical`;
- the page directory holds `after  before  index.html`.

- [ ] **Step 4: Check the sheet still shows the first-touch splay**

Read `target/brush_sheets/a/01.png` (Round · Kolinsky Sable) with the Read tool. It must
show the S-curve above the ramp, with spatter and a detached blob at the left start of
the ramp. That is the baseline look that #62 phase 1 must fix.

- [ ] **Step 5: Clippy and file sizes**

```bash
$DOCKER cargo clippy -p cadmium-studio --examples --release -- -D warnings
.github/check-file-sizes.sh
```

Expected: no output.

- [ ] **Step 6: Commit**

```bash
git add crates/cadmium-studio/examples/brush_sheets/
git commit -m "feat(studio): render and compare example sheets for every factory brush"
```

---

### Task 6: Baselines README

**Files:**
- Create: `docs/superpowers/baselines/README.md`

- [ ] **Step 1: Write the README**

````markdown
# Baselines for the XPBD bristle rewrite (#62)

Every phase of #62 is measured against the engine as it was before the rewrite. The
baseline engine is the tag `xpbd-baseline`: the last commit of phase 0, which still has
the old bristle step.

## GPU cost

`2026-11-16-bristle-cost.tsv` holds `bristle_cost` results recorded on the baseline. The
adapter is named in the file's first line. A phase closes only if this passes on the same
adapter:

```bash
CADMIUM_BENCH_BASELINE=docs/superpowers/baselines/2026-11-16-bristle-cost.tsv \
  cargo run --release -p cadmium-engine --example bristle_cost
```

It exits with status 1 and lists the scenarios whose mean or p95 GPU time per frame is
more than 10% over the baseline. On a different adapter, record a new baseline from the
tag first, with `CADMIUM_BENCH_OUT`.

## Example sheets

The sheets are not committed. Rendering is deterministic, so they are regenerated from
the tag:

```bash
git worktree add ../cadmium-baseline xpbd-baseline
(cd ../cadmium-baseline && cargo run --release -p cadmium-studio --example brush_sheets -- render "$PWD/target/brush_sheets/baseline")
cargo run --release -p cadmium-studio --example brush_sheets -- render target/brush_sheets/current
cargo run --release -p cadmium-studio --example brush_sheets -- compare \
  ../cadmium-baseline/target/brush_sheets/baseline target/brush_sheets/current target/brush_sheets/page
```

Open `target/brush_sheets/page/index.html` to see each factory brush before and after.

## Invariant tests

`crates/cadmium-engine/tests/bristles/invariants.rs` holds the behaviours from the
spec's section 5. A test the baseline fails is `#[ignore]`d and names the phase that must
make it pass.
````

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/baselines/README.md
git commit -m "docs: how the XPBD phases are measured against the baseline"
```

---

### Task 7: Full verification and the baseline tag

- [ ] **Step 1: Format, lints, sizes, full tests**

```bash
$DOCKER cargo fmt --check
$DOCKER cargo clippy --workspace --all-targets --release -- -D warnings
.github/check-file-sizes.sh
$DOCKER cargo test --release -p cadmium-engine -p cadmium-studio --no-fail-fast
```

Expected:
- no output from fmt, clippy or the size check;
- every test binary reports `ok`;
- the only ignored tests are those already ignored on `main`, plus the light-touch
  invariant.

If `cargo fmt --check` complains, run `$DOCKER cargo fmt`, check the diff, and commit
`style: cargo fmt`.

- [ ] **Step 2: Check that the engine's behaviour is untouched**

```bash
git diff main --stat -- crates/cadmium-engine/src/shaders crates/cadmium-engine/src/engine crates/cadmium-engine/src/session crates/cadmium-studio/src
```

Expected: no output. Phase 0 must not change what the engine paints.

- [ ] **Step 3: Tag the baseline**

```bash
git tag xpbd-baseline
git log --oneline -1 xpbd-baseline
```

The tag is not pushed here. Pushing it, and opening the PR, are part of finishing the
branch with the user.
