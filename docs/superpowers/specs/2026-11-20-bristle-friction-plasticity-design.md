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
- a stuck contact also has two tangential rows, d = t₁ and d = t₂, a basis of the
  relief's tangent plane.

The contact compliance matrix uses each row's direction in place of the normal:

G_ab = Σₕ αₕ ((x_a − xₕ)×d_a)·((x_b − xₕ)×d_b), over the hinges above both vertices.

G stays symmetric positive definite, so the shared Cholesky solves it unchanged. At most
21 rows (7 normal, 14 tangential); typically 6 (two stuck contacts).

**Gaps.** A normal row keeps phase 1's gap. A tangential row's gap is minus the
tangential displacement from the anchor that the free step would give the vertex.

**One iteration:**
1. Free turns, with two new terms:
   - the rest turn is θ⁰ + p, p carried from the ferrule frame;
   - each sliding contact adds an external force of magnitude μₖ(ŝ)·|fₙ| from the last
     iteration, against its slip direction ŝ, as the lab's tip load does.
2. The active set runs phase 1's loop (drop fₙ < 0, add vertices the step puts under the
   floor) plus one test: a stuck contact with |fₜ| > μₛ(−f̂ₜ)·fₙ starts to slide. It loses
   its tangential rows and the system is solved again.
3. After the step, anchors update:
   - a new contact starts stuck, anchored where it touched;
   - a stuck contact keeps its anchor;
   - a sliding contact's anchor moves to the vertex. If this step's slip reversed ŝ or
     fell under the slip tolerance, it sticks again. A sliding contact does not retry
     sticking every substep, which would chatter;
   - a released contact clears its anchor.

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
term for term. In the grouped solve, lane v builds vertex v's rows and columns; the
Cholesky and the active set stay on lane 0. The equivalence test also compares anchors
and p.

## 3. Tests and measurement

**GPU lab, against closed forms** (oracles in `mechanics`):
- **Stick and slip.** A strand with its tip resting on a flat floor; the root translates
  horizontally in small steps. While stuck, fₜ grows linearly with the displacement, as
  the CPU oracle gives from the same G. It lets go at μₛ·fₙ, then slides at fₜ = μₖ·fₙ.
  Within 2%.
- **Anisotropy.** The same test pushed and pulled: the thresholds' ratio is
  (1 − C_η·cos²), within 2%.
- **Two contacts.** The tip and the vertex above it, both stuck: the coupled forces match
  the CPU solve of the 6×6 system.
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
