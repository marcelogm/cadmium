# XPBD bristles: a physically grounded brush

Date: 2026-11-16
Status: approved design, awaiting spec review
Literature: [2026-11-16-xpbd-bristles-literature.md](2026-11-16-xpbd-bristles-literature.md)
(every claim there is tagged [V] read in the primary source, [P] indirect, [U] unverified)

## Why

The bristle simulation (`crates/cadmium-engine/src/shaders/bristle*.wgsl`,
`src/engine/bristles/`) does not behave like a real brush in three areas:

1. **Touch and pressure.** The first contact, the taper with pressure, and a light
   touch that should land only the point.
2. **Bristle motion.** Drag, bending, turning in curves and springing back.
3. **The mark.** Bristle streaks, edges and dry-brush gaps.

The model is a set of positional relaxations whose strength depends on the substep and
iteration counts ("bend toward rest by a stiffness share per iteration"). Several recent
commits patch its symptoms: the handle drop that forces a light touch to land, the hair
viscosity that calms swinging, the wetness follow share, and the contact hysteresis band.

The bug that started this work shows the pattern. Every stroke resets the rig and hangs
the hairs analytically into a converged wet point. The first clump step then sums one
spacing push per crowded neighbour (Jacobi without normalisation) and throws the bundle
outward on the very dab that touches the canvas. Normalising the push fixes the first
touch but breaks the spacing calibration everywhere else.

## Decisions

| Topic | Decision |
|---|---|
| Scope | Rewrite the bristle physics. Paint transfer keeps its algorithm and gets better inputs. |
| Grounding | Published models: XPBD, Small Steps, DER, Wetbrush, Baxter, Chu & Tai, DiVerdi, Bico. |
| Performance | Same GPU budget as today, checked by the `bristle_cost` gate (see `docs/superpowers/baselines/README.md`). |
| Parameters | New physical parameters. No migration of saved brushes, since nobody depends on them yet. |
| Validation | Invariants from the literature as tests, side-by-side sheets per phase, and two paper figures reproduced. |

## 1. Architecture, units, time step

**What stays.**
- The dab pipeline: Bristles → Contact → Exchange → Apply → Capillary → Refill → Spread.
- The root layout: `roots.rs`, owners, ranks and `cell_grids`.
- Only the Bristles pass is rewritten. Contact reads a few new outputs (section 4).

**Units.**
- Length uses the scale the relief already uses, `RELIEF_UM_PER_PX` = 40 µm/px
  (25 px/mm), so a 40 px brush has a 1.6 mm ferrule.
- Mass and time are SI.
- Each simulated strand stands for a tuft of n real hairs. Mass scales by n. Bending
  stiffness also scales by n, because parallel, unbonded hairs add their EI.
- Compliances come from E, the hair radius and n.

**State.**
- 8 vertices per strand (today 6), each with position, previous position and inverse mass.
- Bending is measured with the DER curvature binormal against a rest curvature carried
  from the ferrule frame. There is no per-edge quaternion and no twist.
- State persists across strokes.
- Reset only on a layout change, a tool change or a clean. The rest state is then reached
  by relaxing the dynamics in the air, never by an analytic hang. Until clumping exists
  (phase 3) the rest shape is the equilibrium in the air, so a reset places the strands
  on it directly.

**Time step (Small Steps).**
- The dab dt is the gap between sample timestamps, clamped to 50 ms. The settle clock
  keeps running while the brush is lifted, so a stroke's first dab no longer has dt = 0.
- Substeps: n = ceil(dt / h_max), capped by the budget (about 16). One solver sweep per
  substep. The handle pose is interpolated across the dab.
- Integration happens in the ferrule frame. The ferrule's acceleration (linear and
  rotational, Wetbrush eq. 2) enters scaled by β_B ≈ 0.05.
  - Positions and previous positions are carried rigidly with the ferrule pose at every
    substep, so the frame's own motion adds no velocity.
  - The ferrule's linear velocity change between dabs enters on the dab's first substep,
    scaled by β_B. Rotation is carried rigidly and adds no fictitious force (no Coriolis
    or centrifugal term); revisit if turning flats need it.
- A dab with dt = 0 runs one projection substep and leaves the velocities as they were.
- Damping comes only from XPBD constraint damping and from β_B.

**Removed:** `handle_drop_px` and `DROP_RELAX_SHARE`, `HAIR_VISCOSITY_PER_SECOND`,
`bend_toward_rest` by share, `OVERLAP_SHARE_PER_SIDE`, `WETNESS_FOLLOW_SHARE`, the
contact band hysteresis (`RELEASE_BAND_SHARE`), the per-stroke reset and `hang_at_rest`.

## 2. Solver and constraints

**Solver: Gauss-Seidel within a strand, Jacobi between strands.**
- One thread owns one strand, so the chain constraints are swept sequentially at no cost
  to parallelism. A sweep carries a correction from root to tip, where Jacobi would need
  about one iteration per vertex.
- The sweep direction alternates between substeps (root→tip, then tip→root) to remove
  ordering bias.
- If inextensibility is still too soft, a direct per-strand solve (Deul et al. 2018) is
  plan B, decided by measurement.
- Strand-to-strand constraints use Jacobi with constraint averaging (Macklin et al. 2014),
  relaxation ω in 1 to 1.5.
- λ is accumulated per constraint within a substep. With one sweep per substep it resets
  every substep.

**Order inside a substep:**
1. Predict.
2. Per-strand Gauss-Seidel: stretch, bend, canvas contact and static friction.
3. Barrier.
4. Jacobi: density and wet clumping.
5. Barrier.
6. Velocity update, including dynamic friction (Müller et al. 2020).

**2.1 Stretch.**
- Segment distance with zero compliance.
- Vertex 0 has infinite mass and is pinned to the ferrule.
- The first segment is clamped: its rest direction in the ferrule frame is a bend
  constraint at vertex 0, so each strand acts as a cantilever.

**2.2 Bending.**
- Uses the DER curvature binormal κbᵢ = 2(eᵢ₋₁×eᵢ)/(|eᵢ₋₁||eᵢ| + eᵢ₋₁·eᵢ), compared
  with the rest κb⁰ᵢ carried along the chain from the ferrule frame.
- Compliance α = Δs/(n·E·I), with I = πr⁴/4.
- The radius tapers from root to tip by `tip_taper`, so the tip is (taper)⁴ as stiff as
  the root.
- The rest curvature comes from the head shape (splay and belly) plus a small random
  curvature, so each strand buckles in a consistent direction.
- Plasticity: when |κ − κ⁰| exceeds `yield_angle`, κ⁰ creeps toward κ at `creep_rate`.
  This is Baxter's joint friction and Chu & Tai's zero-shift, capped at 0.05–0.10 rad.

**2.3 Canvas contact.**
- One-sided constraint per vertex: z ≥ floor(x, y) + r(s).
- The floor is today's relief: dry height, plus wet volume scaled by the wet-support
  parameter, minus the canvas tooth.
- The normal force is fₙ = λₙ/h².

**2.4 Friction.**
- Static friction is applied at position level: tangential slip since the anchor is
  cancelled while it stays below μₛ·|Δxₙ|.
- Dynamic friction is a velocity update, Δv = −v̂ₜ·min(h·μₖ·fₙ, |vₜ|).
- Starting values μₛ ≈ 0.5 and μₖ ≈ 0.3 (Baxter & Lin give 0.3–0.5). Hair-on-canvas
  friction is not measured anywhere we found, so these stay estimates.
- Anisotropy (Baxter & Lin): μ·(1 − C_η·max(0, d_pull·Δx̂)^k), so a pushed tip catches
  and a pulled tip glides.

**2.5 Strand density.**
- Pairs of vertices on neighbouring strands keep at least rᵢ + rⱼ apart (the Wetbrush
  minimum-density idea).
- Neighbours are k ≈ 6 nearest roots, fixed at layout time. They replace the 8-member
  clusters, so strands in different clusters no longer pass through each other.

**2.6 Wet clumping (elastocapillary, Bico et al. 2004).**
- At layout time, neighbours are paired into a tree of pairs of pairs.
- A level with N strands closes when the bending length L available between the clamp
  and the capillary bridge satisfies L⁴ ≥ (9/16)·N³·d²·L_EC².
- L_EC = √(κ/γ_eff), with γ_eff = γ·wetness and γ ≈ 0.033 N/m for linseed oil [U].
- Bico's law is for lamellae. Applying it to round fibres (Py 2007) is not verified, so
  the reproduction test in section 5 checks the lamella form.
- A closed level becomes a compliant attraction between its strands near the tip. Its
  compliance comes from a capillary force of about 2γ per wetted length.
- A level that has closed reopens only above L_c·(1 + h_open). Capillary bridges break at
  a different distance than they form, because of contact-angle hysteresis. The value of
  h_open is an estimate.
- Results without extra parameters: a wet brush closes into a point, a pressed brush
  opens when contact beats capillarity, and a drying brush splits into tufts (Chu & Tai's
  dry brush).
- Removed: `wet_point`, `point_release`, `point_film`, `gathering`, the pull to centroid.

## 3. From brush to constraints, and the handle

**Hair material** (`Hair`, replaces `HairSpec`):

| Field | Unit | Feeds |
|---|---|---|
| `modulus` E | GPa | bending compliance |
| `density` ρ | kg/m³ | vertex mass |
| `diameter` d at the root | µm | I, the density distance, d in clumping |
| `tip_taper` | 0.5–1 | r(s) along the strand |
| `static_friction`, `kinetic_friction` | μ | friction |
| `friction_anisotropy` | 0–1 | friction |
| `yield_angle`, `creep_rate` | rad, 1/s | plasticity |
| `wicking`, `load_limit` | unchanged | paint side |

Presets:
- Nylon/Taklon: E ≈ 3–4 GPa [P].
- Sable and squirrel: keratin, 2–4 GPa [U].
- Hog: about 6 GPa [U].
- Diameters, friction coefficients and the packing fraction are [U].
- All presets live in one table that cites the source or the [U] tag next to each number.

**Head geometry** (`HeadSpec`):
- thickness, length, dome, slant, belly and fan stay. They place roots and set strand
  length and rest curvature.
- `point` is removed. The point is an effect of wet clumping, not of the shape.

**Simulation resolution.**
- `density_per_px` only picks how many strands are simulated.
- The real hair count is N_real = φ·A_ferrule/(πd²/4), with packing φ ≈ 0.7 [U].
- Each strand carries n = N_real/N_sim hairs, so the bundle's total stiffness does not
  depend on resolution.

**Pressure → handle height** (DiVerdi/Adobe, US 8,760,438):
- The handle sits on the pen axis above the nib, as today.
- h(p) = h_touch − p·`press_depth`·L.
- h_touch is the height at which the brush's *rest shape*, placed in the dab's pose,
  first touches: the floor is read under each rest vertex, so relief and impasto under
  the hairs count. A max-reduction at the start of the bristle step measures it.
  - The rest shape includes the rest curvature, and from phase 2 the plastic one.
  - It is not the deformed current state: a pressed hair already lies on the floor, so
    measuring the current state would return the handle's own height and lower the
    handle by the press depth again on every dab.
- The CPU sends only the press depth p·`press_depth`·L, or the clearance when lifted;
  the GPU adds h_touch.
- p = 0 therefore means only the tip touches, by definition.
- The patent's reduced press depth for stiff brushes is kept as a calibration option.
- A force target with a feedback loop was considered and rejected: no source uses it,
  and it risks oscillating inside the step.

**Lift and settle.** The handle rises on lift and the dynamics keep running on the frame
clock, as today. The next stroke starts from the settled state.

## 4. Interface with paint

Paint transfer keeps its algorithm.

**Physics → paint.**
- Vertex positions and the floor under them, through the shared accessors
  (`position_slot`) in one WGSL file, so Contact follows the new layout.
- The strand radius per vertex, including the taper. Contact moves from a radius per
  strand to a radius per segment.
- `hair_loads`: the strand's normal force over the dab, Σ λₙ/h², divided by the mean
  over the touching strands. This keeps "1 = typical load", so Exchange is unchanged.
- A contact bit per vertex: λₙ > 0 in any substep of the dab. It replaces the distance
  band and its 1.2× hysteresis. The `swept` interpolation between dabs stays.
- `owners`, `ranks` and `cell_grids` are unchanged.
- The relief bisection (`resting_base`) disappears from the bristle step.

**Paint → physics.**
- Wetness per strand: tip volume along the lanes, as today. It sets γ_eff.
- Relief (dry height, wet volume) and canvas tooth, as today.

**Deferred to a paint-transfer project.** All of these become available but Exchange
does not use them yet:
- absolute normal force in N;
- per-strand slip velocity;
- capacity that falls with compression and canvas proximity (Wetbrush eqs. 12–13).

## 5. Validation, robustness, phases

**Invariant tests (GPU).** The phase column says which phase brings each test in;
phase 0 holds the ones today's model can already run.

| Test | Source | Phase | Asserts |
|---|---|---|---|
| Cantilever | Euler-Bernoulli | 1 | A single strand with tip load F deflects FL³/(3EI) within 5%. |
| Head stiffness | Baxter & Lin | 1 | The whole head bends about 90° under 1–2 N. |
| Step independence | XPBD | 1 | The same stroke with h_max and h_max/2 ends within tolerance. |
| Resolution independence | §3 | 0 (width), 1 (splay) | Doubling `density_per_px` keeps mark width and splay within tolerance. |
| Light touch | DiVerdi | 0, ignored until 1 | At p = 0 only the tip touches, and the first dab does not splay the bundle (`a_light_first_touch_lands_the_wet_point_instead_of_splaying_the_hairs`). |
| Splay vs pressure | DAB, DiVerdi | 0 | Splay grows monotonically with pressure. |
| Friction | Baxter & Lin, Müller | 2 | The strand holds below μₛ and slides above it. A pushed tip catches and a pulled tip glides. |
| Trail and spring back | Baxter | 2 | Tips lag when dragging and in turns, and return on lift. Without plasticity they return to rest. |
| Coalescence | Bico | 3 | Two strands stick iff L⁴ ≥ (9/2)d²L_EC². Cluster size grows with L. Drying splits clusters. |
| Stability | — | 1 | Nothing goes below the floor. Everything stays finite with bad timestamps and at dt = 50 ms. |
| Determinism | — | existing | A repeated stroke and dab batching across frames stay bit-identical. Paint conservation stays exact. |

**Existing tests.**
- Each is reviewed. Those that describe real behaviour stay: trailing, dry threads,
  unbroken fine lines, painting under the nib.
- Those that pin removed mechanisms go: the wet point kept out of bend memory, the
  handle drop, the old clump spacing.
- Layout fingerprints are updated for the neighbour lists and clump tree.

**Visual validation.**
- Every phase ends with a side-by-side sheet, before and after, for every factory brush.
- Two paper figures are reproduced: Bico's coalescence and DiVerdi's mark vs pressure.

**Performance.**
- Record a `bristle_cost` baseline before any change.
- A phase closes only if a full `bristle_cost` run passes the gate against that baseline:
  per-frame GPU mean and p95, median of five interleaved runs after a warm-up, each at
  most 10% and 0.05 ms over the baseline. `docs/superpowers/baselines/README.md` has the
  exact rule and commands.

**Robustness.**
- dt is clamped to 50 ms and substeps are capped by the budget.
- The correction per substep is capped. This is a guard, not physics.
- A strand that turns non-finite returns to rest, and the event is logged.

**Phases, one PR each; the app keeps working after each:**
0. **Baseline.** Record the bench numbers (TSV and gate) and a sheet tool tied to the
   `xpbd-baseline` tag. Add the closed-form oracles (`mechanics`) and the invariants
   today's model can run.
1. **XPBD core.** New state, Small Steps, ferrule-frame integration, stretch, bend and
   clamp, frictionless canvas contact, pressure → height, persistent state. The old
   step is removed. GPU cantilever, head-stiffness and step-independence tests run
   against `mechanics`. Until the later phases:
   - **Removed with the old step:** friction, bend memory, the wet point and clumping.
     Tests that need them are `#[ignore]`d and name the phase that brings them back.
   - **Material bridge:** `Hair` arrives in phase 4, so E comes from `HairSpec.stiffness`
     (about 1 GPa at 0 to 7 GPa at 1), and ρ, d and the taper are named constants
     tagged [U]. One module holds the bridge; phase 4 replaces it.
   - **Contact:** z ≥ floor, without + r(s), because Contact still decides touching by a
     clearance of one radius. The r(s) offset arrives with the contact bit in phase 4.
   - **`hair_loads`:** a flat 1 for every hair until phase 4 brings the normal force.
2. **Friction.** Static and kinetic friction, anisotropy, plasticity.
3. **Clumping.** Neighbour density and elastocapillary clumping with hysteresis.
4. **Interface and parameters.** Normal force → `hair_loads`, the contact bit, `Hair`
   in physical units with presets, the brush editor, and the brush file format (no
   migration). The old fields and constants are removed.
5. **Calibration.** Sheets, paper figures, presets, and the README, which still says
   brushes are stamped.

## Future work

- Gravity, together with a fluid simulation for wetter paints.
- The paint-transfer project: absolute force, slip velocity, compression-dependent capacity.
- Non-linear bending energy (Chu & Tai's |θ|³), if calibration shows the need.
- Twist and per-edge frames, if flat brushes need them.

## Open items, all estimates to calibrate

- Hair diameters for sable, squirrel and hog; hair-on-canvas friction; linseed oil
  surface tension; the packing fraction φ; h_open.
- Whether Bico's lamella law carries over to round fibres (Py 2007, not read).
