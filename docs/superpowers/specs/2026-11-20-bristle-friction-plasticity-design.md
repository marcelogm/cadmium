# Bristle friction and plasticity (phase 2 of #62)

Phase 2 adds canvas friction (static, kinetic, anisotropic) and plastic rest curvature
to the quasi-static strand solve of phase 1 (`perf/strand-parallel-solve`). It replaces
§2.2 "Plasticity" and §2.4 of `2026-11-16-xpbd-bristles-design.md` where they differ.

## Decisions

- **Budget:** no ceiling up front. The physically right formulation is built, measured,
  and the measured figure is proposed as the phase 2 budget on #62.
- **Friction:** coupled in the contact active set. Stuck contacts add tangential rows to
  the contact compliance matrix; sliding contacts add a kinetic force.
- **Plasticity:** yield with creep in time, Baxter's joint friction and Chu & Tai's
  zero-shift bounded by a cap per hinge.
- **μ is constant:** paint does not lubricate in this phase. No measured value exists for
  hair on oil paint; that belongs to calibration (phase 5) or the paint-transfer project.

## 1. State and parameters

New state in `states`, after the positions, inside each strand's `BRISTLE_STATE_VECTORS`:

| block | per | `xyz` | `w` |
|---|---|---|---|
| anchor | vertex 1..7 | where friction holds the vertex, px | the normal force of the last iteration, N: 0 free, > 0 stuck, < 0 sliding with \|fₙ\| |
| plastic shift | hinge 0..6 | rotation vector p added to the rest turn, ferrule frame | unused |

Both persist across strokes and reset with the positions: layout change, tool change,
clean.

Parameters, bridged until `Hair` arrives in phase 4:

| parameter | source | value |
|---|---|---|
| μₛ, μₖ | constants [U] | 0.5, 0.3 (Baxter & Lin give 0.3–0.5) |
| C_η, anisotropy | `HairSpec.drag_anisotropy` | presets, 0.2–0.45 |
| anisotropy exponent k | constant | 2 |
| creep rate, 1/s | `HairSpec.bend_memory` | presets, 1–4 |
| yield angle | constant [U] | 0.02 rad |
| shift cap | constant (Chu & Tai) | 0.1 rad per hinge |

`bend_memory = 0` turns plasticity off. The creep clock is the dab's `dt_seconds`,
clamped to 50 ms. `BristleSnapshot::rest_bends` reports p.

## 2. Solve

**Rows.** Each active-set row has a direction d at its vertex:
- an engaged contact has a normal row, d = n;
- an anchored contact also has two tangential rows: d = t₁, the pull direction (the
  segment toward the root, projected onto the tangent plane), and d = t₂ = n × t₁.
- When a vertex and the one above it are both anchored, the lower one drops its t₁ row.
  Inextensibility already ties its motion along the segment to the upper vertex's, so
  the row would be redundant and leave G near-singular.

The contact compliance matrix uses each row's direction in place of the normal:

G_ab = Σₕ αₕ ((x_a − xₕ)×d_a)·((x_b − xₕ)×d_b), over the hinges above both vertices.

G stays symmetric positive definite, so the shared Cholesky solves it unchanged. At most
21 rows (7 normal, 14 tangential).

**Gaps.** A normal row keeps phase 1's gap. A tangential row's gap is minus the
tangential displacement from the anchor that the free step would give the vertex.

**One iteration (always try to stick):**
1. Free turns, with the rest turn θ⁰ + p, p carried from the ferrule frame.
2. Every engaged contact with an anchor starts with its tangent rows, stuck or sliding.
   The active set runs phase 1's loop (drop fₙ < 0, add vertices the step puts under the
   floor) plus the cone test. The force fₜ it takes to hold the contact is compared with
   μₛ(−f̂ₜ)·|fₙ| if the contact was stuck, or μₖ(−f̂ₜ)·|fₙ| if it was sliding. The kinetic
   threshold is a hysteresis: a sliding contact sticks again only when holding it needs no
   more than kinetic friction, so it does not chatter. Past the threshold the contact
   slides: it loses its tangent rows and takes the kinetic force μₖ(ŝ)·|fₙ|·f̂ₜ, with slip
   direction ŝ = −f̂ₜ, and the system is solved again.
   - A held contact whose normal force comes out negative slides before it is released:
     fₙ < 0 lies outside every cone. Only a contact without tangent rows is released.
   - Each tangent row's pivot gets 10⁻³ of the vertex's normal compliance: the contact's
     shear compliance. A near-straight strand cannot move an anchored vertex along its axis
     to first order, so without it normal + t₁ + t₂ is rank-deficient and the ridge returns
     arbitrary forces. A held vertex creeps by that share of the drive.
   - The cone is tested against the normal force of the same solve that holds the contact
     (Coulomb's cone is on one solution's forces). The kinetic force's magnitude instead
     uses the previous iteration's |fₙ|, held in the anchor's w: the normal of a solve that
     pins the vertex can be orders of magnitude off, and a kinetic force built on it throws
     the vertex off the canvas.
3. After the step, anchors update:
   - a new contact lands frictionless on its first iteration and is anchored where that
     step leaves it;
   - a stuck contact keeps its anchor;
   - a sliding contact's anchor moves to the vertex;
   - a released contact clears its anchor;
   - without friction (μₛ = 0) no anchor is written.

**Anisotropy (Baxter & Lin).** μ(ŝ) = μ·(1 − C_η·max(0, d_pull·ŝ)ᵏ), with d_pull the
direction from the vertex toward the one above it (toward the root), projected onto the
tangent plane. A pulled tip glides; a pushed tip catches.

**Plasticity.** Once per dab, after its iterations. For each hinge, e = θ − (θ⁰ + p) in
the ferrule frame. When |e| > yield:

p ← p + min(creep·dt, 1)·(|e| − yield)·ê, then |p| capped at 0.1 rad.

At dt = 0, p does not change. p does not relax back on its own.

**End-of-dab floor corrections.** Unchanged: contact-only, normal, straight up. They do
not touch the anchors.

**Both solves.** `solve_reference` (serial) and the grouped solve run the same algorithm,
term for term. In the grouped solve, lane v owns vertex v + 1's rows: their directions,
gaps and matrix entries. The active rows' Cholesky is factored across the strand's lanes
(`bristle_group_cholesky.wgsl`, lane v owning active rows v, v + 8, …), which keeps its
arithmetic on the vector unit as in the serial solve; the active set's decisions (release,
slip, settled) stay on lane 0. The equivalence test also compares anchors and p.

## 3. Tests and measurement

**GPU lab, against closed forms** (oracles in `mechanics`):
- **Stick and slip.** A strand curled like the app's (−0.02 rad) with its tip pressed
  on a flat floor. Hold tests drag sideways, where bending holds the tip; slide tests drag
  along the strand's plane, where an inextensible strand cannot give and the tip slides
  at once. The root translates in small steps, each iteration taking its share. While stuck, fₜ grows linearly with the displacement, as
  the CPU oracle gives from the same G. It lets go at μₛ·fₙ, then slides at fₜ = μₖ·fₙ.
  Within 2%.
- **Anisotropy.** The same test pushed and pulled: the thresholds' ratio is
  (1 − C_η·cos²), within 2%.
- **Two contacts.** A deep press with at least two stuck contacts, then a small drag:
  every stuck vertex holds its anchor within 1e-3 px and stays inside its cone, which a
  per-contact projection would not do.
- **Plasticity.** A hinge held bent by a tip load over N dabs:
  - p follows the closed form of the creep up to the cap;
  - at dt = 0, p does not change;
  - released, the strand returns to θ⁰ + p, not θ⁰.
- **Step independence.** A drag with substeps half as long ends within tolerance.

**Integration.**
- The 8 phase 2 ignores come back: `friction` ×5, `look::an_s_curve…`,
  `sizes::…no_holes`, `engine_shapes::…broadside_wide_and_edgewise_thin`.
- Every test passing today still passes, in particular `no_plasticity…` and
  `a_wet_brush_never_sets_its_point…`.
- A test that passes only once it is changed is reported on #62 with the reason.

**Measurement.**
- Full gate and `CADMIUM_BENCH_STAGES=1`, steps compared by interleaved prebuilt binaries
  (A B B A).
- The measured figure is proposed on #62 as the phase 2 budget. Optimisations stay only
  if they leave the physics unchanged.

**Visual.** A before/after sheet for every factory brush.

## Out of scope

- Friction that depends on paint or wetness.
- Plastic recovery other than a reset.
- Density and clumping (phase 3); `hair_loads` and the contact bit (phase 4).

## Results

Full gate (5 interleaved repeats after a warm-up, AMD Radeon RX 9060 XT, RADV), GPU mean / p95 ms per frame. Phase 1 is `bdd0862`, phase 2 is this branch; prebuilt binaries run A B B A twice, median of the four runs each (two for friction off, a throwaway build that calls `use_frictionless_bristles(true)`).

| scenario | baseline | phase 1 | phase 2 | phase 2, friction off |
|---|---|---|---|---|
| brush, default size | 0.78 / 0.86 | 1.00 / 1.12 | 1.95 / 2.25 | 1.21 / 1.49 |
| brush, 200 px | 0.91 / 1.47 | 1.26 / 2.30 | 1.44 / 2.21 | 0.97 / 1.55 |
| brush, max size | 1.64 / 3.23 | 1.55 / 2.85 | 2.10 / 3.48 | 1.68 / 3.09 |
| brush, 200 px over impasto | 0.78 / 1.24 | 0.98 / 1.78 | 1.37 / 2.11 | 0.88 / 1.39 |
| brush, max size over impasto | 1.23 / 2.41 | 1.51 / 2.94 | 1.85 / 3.00 | 1.33 / 2.37 |
| knife, default size over impasto | 0.19 / 0.21 | 0.23 / 0.26 | 0.17 / 0.19 | 0.18 / 0.20 |
| blender, default size over impasto | 0.71 / 0.78 | 0.84 / 0.93 | 1.79 / 1.98 | 1.15 / 1.37 |
| long brush stroke over impasto | 0.76 / 0.83 | 0.88 / 0.94 | 1.83 / 1.95 | 1.16 / 1.24 |
| long 200 px stroke over impasto | 0.72 / 1.12 | 0.72 / 1.10 | 1.31 / 1.98 | 0.80 / 1.20 |
| idle settle after a stroke | 0.03 / 0.03 | 0.03 / 0.03 | 0.04 / 0.04 | 0.04 / 0.04 |
| shade the full canvas | 0.41 / 0.43 | 0.51 / 0.53 | 0.41 / 0.42 | 0.41 / 0.43 |
| present the full canvas | 0.05 / 0.05 | 0.06 / 0.06 | 0.05 / 0.05 | 0.05 / 0.05 |

Bristles stage, µs/dab (`CADMIUM_BENCH_STAGES=1`, mean of two passes):

| scenario | phase 1 | phase 2 | phase 2, friction off |
|---|---|---|---|
| brush, default size | 70.1 | 366.5 | 182.1 |
| blender, default size over impasto | 53.0 | 215.0 | 111.1 |
| long brush stroke over impasto | 59.4 | 210.7 | 110.4 |
| brush, 200 px | 199.7 | 927.0 | 502.0 |
| brush, max size | 260.0 | 1294.6 | 691.7 |

- Bristles stage at default size: 70.1 → 366.5 µs/dab (5.2×); the frame went 1.00 → 1.95 ms. The other stages did not grow.
- Friction is half of it: with static friction zeroed the stage reads 182.1 µs/dab, still 2.6× phase 1.
- The untouched `shade the full canvas` read 0.51 ms in every phase 1 run and 0.41 in every phase 2 run: the lighter phase 1 load let the clocks drop, so phase 1's figures sit high and the phase 1 → 2 step is, if anything, understated.
- Proposed phase 2 budget: +162% over the baseline (worst scenario, `brush, default size` p95 0.86 → 2.25 ms; worst mean `blender, default size over impasto` +153%).

### Tests

Of the 8 phase 2 ignores, three came back (`friction::a_brush_stays_bent…`,
`…does_not_bend_it_further`, `…pulled_tips_slide_more_easily…`). The others, and one test
the spec said would keep passing, are ignored again with the measured shortfall:

- `friction::bristle_tips_trail_behind_the_handle…` (phase 5): the tips trail 6.2 px,
  0.12 of the length, as far as μₖ·fₙ bends the hairs at their estimated stiffness; the
  test asks 0.2.
- `friction::on_a_turn_the_tips_lag_the_new_direction` (phase 5): the tips lag the old
  heading by 4.9 px of the 5 asked, for the same reason.
- `look::an_s_curve_leaves_different_tracks…` (phase 5): friction does not paint it. The
  apexes read 26/25 and 24/24 px for μ from 0 to 10, and tips trailing 6 px move the track
  by d²/2R ≈ 0.23 px. Reproduce it from a calibrated trail or drop it.
- `sizes::the_largest_brush_pressed_without_clumping_leaves_no_holes` (phase 3): a pressed
  curled tip needs |fₜ| ≈ fₙ to hold, past μₛ, so the dab splays and leaves a 3977 px²
  hole; held fast it still leaves 3 px². It needs packing.
- `engine_shapes::a_flat_brush_paints_broadside_wide_and_edgewise_thin` (phase 3): without
  packing the flat bundle splays edgewise to 130 px against 217 broadside; even μ = 10
  gives a ratio of 2.0.
- `friction::a_wet_brush_never_sets_its_point_into_its_bend_memory` (phase 3), newly
  ignored: without clumping there is no wet point, and the press splay sets as plasticity
  should (−0.0103 at pressure 0.1, −0.408 at 1), so only the clumped set minus the
  unclumped one can tell the point apart.

`document::export::paint_on_the_painting_is_exported_where_it_was_laid` no longer asks the
changed rows to equal the wet rows widened by the relief's reach. It asks that every wet
row changes and no row past the reach does: with friction the stroke's edge is too thin
for the relief to shade the next row.

The stick-phase oracle (`lab::tests::stick`) presses 0.1 px instead of 1 px. G is first
order: the load's softening of the strand, which G leaves out, takes 5.5% off fₜ at
the 1 px press and 0.7% at 0.1 px. It also checks only fₜ along the drag. Across the drag
the strand is inextensible, and that component changes with the square of the drag.

### Optimisation ladder

The grouped solve's physics unchanged: every kept step leaves its output bit-identical to `a7575cf` (a hash of chains, anchors, plastic shifts and normal forces over ridges with and without friction, and on flat canvas), so the serial reference did not change. Each step against the previous kept one, prebuilt binaries A B B A, Bristles µs/dab at default size (mean of eight stage runs, or four for a step rejected on its first round). The absolute figures drift with the clocks between sessions; each row's pair was measured together.

Attribution at `a7575cf` (default size, throwaway builds that break the physics): with the Cholesky reduced to its diagonal the stage read 157 µs/dab against ~370, without held_back 231, without any contact 82. The solve held two 21×21 private copies of the matrix (in `group_active_forces` and the by-value `solved_by_cholesky`), and spilled 1030 VGPRs into 582 KB of scratch. Moving them to workgroup memory on lane 0 alone changed the output: values loaded from a uniform address run on the scalar ALU, which does not fuse the multiply-adds. Factored across the lanes, each row indexed by its lane, the arithmetic stays on the vector ALU as before. Later attribution, by running one phase twice (idempotent phases, physics intact): the matrix fill cost ~55 µs/dab, the rounds' solves ~36 (their gather ~10), gaps and diagonals ~7.

| step | Bristles µs/dab | kept |
|---|---|---|
| Cholesky in workgroup memory, factored across the lanes | 393.2 → 312.6 | yes |
| a column's pivot with its rows' sums and the forward pass | 311.9 → 308.8 | yes |
| fill only the entries the rounds can reach | 328.6 → 199.4 | yes |
| levers cached in registers, hinge loop unrolled | 199.7 → 209.3 | no |
| a column's push shared by the vertex's three rows | 199.5 → 194.4 | yes |
| row directions without copying the chain | 195.8 → 191.8 | yes |
| settled flags folded into the unsettled count | 191.5 → 191.6 | no |
| active rows listed across the lanes | 170.7 → 168.8 | yes |
| held_back's copies per lane, unread diagonals skipped | 175.2 → 166.0 | yes |
| tangent diagonals only for anchored vertices | 168.2 → 167.8 | no |

Not taken: skipping held_back when no tangent row is engaged (the serial solve holds back every engaged row, so it would change the physics); skipping the creep pass when `creep_share` is 0 (every factory brush has bend memory, so the benchmark would not see it); filling a row only once the rounds engage it (a throwaway that skips the rows added later bounds it at −5%, before the extra phase it needs).

Final gate, 5 interleaved repeats after a warm-up, GPU mean / p95 ms per frame, median of A B B A twice (`a7575cf` against the ladder's last commit, in one session):

| scenario | baseline | phase 1 | phase 2 before the ladder | after |
|---|---|---|---|---|
| brush, default size | 0.78 / 0.86 | 1.00 / 1.12 | 1.99 / 2.27 | 1.12 / 1.27 |
| blender, default size over impasto | 0.71 / 0.78 | 0.84 / 0.93 | 1.90 / 2.20 | 1.08 / 1.21 |
| long brush stroke over impasto | 0.76 / 0.83 | 0.88 / 0.94 | 1.96 / 2.25 | 1.08 / 1.21 |
| brush, 200 px | 0.91 / 1.47 | 1.26 / 2.30 | 1.40 / 2.18 | 0.86 / 1.37 |
| brush, max size | 1.64 / 3.23 | 1.55 / 2.85 | 1.98 / 3.34 | 1.82 / 3.40 |
| brush, 200 px over impasto | 0.78 / 1.24 | 0.98 / 1.78 | 1.42 / 2.17 | 0.81 / 1.27 |
| brush, max size over impasto | 1.23 / 2.41 | 1.51 / 2.94 | 1.92 / 3.17 | 1.43 / 2.67 |
| long 200 px stroke over impasto | 0.72 / 1.12 | 0.72 / 1.10 | 1.38 / 2.12 | 0.73 / 1.10 |

Bristles stage, µs/dab (mean of two full stage passes, same session): default size 340.9 → 171.4, blender 230.4 → 93.4, long stroke 229.6 → 90.2, 200 px 755.2 → 336.1, max size 997.5 → 493.1.

- The solve shader now uses no scratch (was 582 KB) and no spilled VGPRs (was 1030); its code fell from 190 KB to 96 KB.
- Proposed phase 2 budget: +55% over the baseline (worst scenario, `blender, default size over impasto` p95 0.78 → 1.21 ms; worst at default size, `brush, default size` p95 0.86 → 1.27, +48%).
