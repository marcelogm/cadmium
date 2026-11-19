# Strand-parallel quasi-static solve

Date: 2026-11-19
Status: design approved
Parent: #62, phase 1 (spec: [2026-11-16-xpbd-bristles-design.md](2026-11-16-xpbd-bristles-design.md))

## Goal

Make the quasi-static bristle solve on this branch as fast as it can be **without changing
its physics**, so #62 can decide on measured numbers whether it is viable. There is no
target to hit: the result is the fastest version and its measured cost.

"Without changing its physics" means:

- the same equations, the same floor samples (disc, sweep, slope probes), the same contact
  rounds, the same iteration count per dab;
- the only allowed numeric differences come from reordered floating-point sums (for example
  the reduction behind the turn cap);
- no fewer hinges, iterations or strands, and no approximated floor. Those changes belong to
  a later decision, if this one says the solve still does not fit.

## Why it is slow today

`solve` in `bristle_step.wgsl` runs one strand per thread: 64 strands in the default brush,
so one or two waves on a GPU built for thousands of lanes. Each thread does a long serial
chain with nothing to hide its latency:

- **Floor reads.** Each vertex reads up to 8 discs per iteration (up to 4 along its sweep, 4
  for the slope by central differences). Each disc is 7 bilinear points, and each point
  reads 4 texels from 2 textures. That is up to 448 texel loads per vertex and about 3k per
  strand per iteration.
- **The contact solve.** The O(n²) gaps, diagonal and rises, the O(n³) contact matrix, a
  Cholesky of up to 7×7, and up to 8 active-set rounds.
- **The kinematics.** A serial quaternion prefix down the chain.

Measured on the research HEAD (RX 9060 XT, full `bristle_cost` run against the baseline):

| scenario | baseline mean / p95 (ms) | research HEAD mean / p95 (ms) |
|---|---|---|
| brush, default size | 0.78 / 0.86 | 1.40 / 1.66 |
| brush, 200 px | 0.91 / 1.47 | 1.13 / 1.77 |
| brush, 200 px over impasto | 0.78 / 1.24 | 1.07 / 1.64 |
| blender, default size over impasto | 0.71 / 0.78 | 1.35 / 1.57 |
| long brush stroke over impasto | 0.76 / 0.83 | 1.34 / 1.58 |
| long 200 px stroke over impasto | 0.72 / 1.12 | 0.96 / 1.47 |

The max-size scenarios stay within the gate. The small and medium brushes are the worst
cases, because there the bristle stage is a larger share of the frame.

## Method

The work climbs a ladder of steps. Each step is one commit and is measured before the next
one starts:

- a full gate run (`CADMIUM_BENCH_BASELINE`, 5 interleaved repeats, as the baseline was
  recorded);
- the stage mode from step 0, for µs per dab in Bristles;
- the equivalence test and the GPU lab, both passing.

A step stays only if it measures faster. A step that does not is reverted, and its finding
goes in the results table. The commit body carries the numbers, as on the research branch.

### Step 0: instrumentation

- **Stage timing.** `CADMIUM_BENCH_STAGES=1` makes the engine encode each `DabStage` in its
  own compute pass with `timestamp_writes`, and `bristle_cost` then reports µs per dab per
  stage. Splitting the passes changes the total a little, so this mode is for attribution
  only, and the gate runs in normal mode.
- **Equivalence test.** The current serial entry point stays in the module as
  `solve_reference`. A test paints a stroke over impasto twice, once with `solve` and once
  with `solve_reference`, from the same state. It compares `states` (positions and floors)
  and `normal_forces` with a narrow tolerance. This covers what the lab does not, because
  the lab uses analytic floor planes and no textures. The test passes trivially at step 0
  and becomes meaningful from step 1.

### Step 1: one strand per group of 8 lanes

- **Mapping.**
  - With `BRISTLE_VERTICES = 8` (7 hinges and 7 contact rows), a workgroup of 64 lanes
    solves 8 strands. Lane *v* of a strand's group owns vertex *v* and hinge *v*.
  - Lane 0 has no floor, since vertex 0 is held by the ferrule. It runs the strand's serial
    work: the Cholesky, the active-set decisions and the cap.
  - Lanes of strands past `bristle_total` take part in every barrier but do no work. Nothing
    returns early.
- **Workgroup memory, per strand.** `chain`, `sampled`, `edges`, `turns`, `normals`, floor
  heights and slopes, `gaps`, `forces`, `engaged`, the diagonal, and the 7×7 contact matrix.
  This is about 2 KB per strand and 16 KB per workgroup.
- **One iteration, in phases separated by barriers.**
  1. Carry and floor, per vertex. This is the bulk of the loads.
  2. Edge *e*, per lane. The degenerate-edge fallback (`select(previous, …)`) depends on
     the edge before. A short serial pass on lane 0 fixes it, and runs only when some edge
     in the strand is degenerate.
  3. Free turn, gap and diagonal, per lane.
  4. Contact matrix: each lane builds its own row.
  5. Active set: up to `CONTACT_ROUNDS` rounds with a workgroup-uniform count.
     - Lane 0 solves and picks the contacts to drop, and the lanes compute the rises in
       parallel.
     - The loop leaves early once every strand in the workgroup has settled, read through
       `workgroupUniformLoad` of a shared counter.
     - The "no vertex under its floor" shortcut becomes a per-strand flag.
  6. Contact turns per hinge, then a max-reduction for the cap.
  7. Kinematics: each lane recomputes the quaternion prefix down to its own vertex, in the
     serial order, so its result matches the serial one.
- **Uniformity.** The dab's iteration loop is uniform already, because `substeps` comes from
  the `dab` uniform. Only the active-set loop needs the uniform count above.
- **Out of scope.** The `touch` dispatch (its own reduction), and Contact, Exchange and the
  rest of the dab.

### Step 2: texel cache

- **Today.** Every `surface_px` reads 4 texels from each of 2 textures and combines them
  through `load_bearing_um`.
- **The cache.** Each lane bounds the texels its disc, sweep and slope probes touch, plus
  one for the bilinear. It loads each texel once, already combined by `surface_um_at` (one
  float per texel), into a window in workgroup memory, and runs the bilinears from it.
- **Fallback.** The window has a fixed maximum size (8×8 to start). A lane whose bounds
  exceed it, for a long sweep or a wide strand, falls back to direct loads. The arithmetic is
  the same on both paths.
- **Rejected.** One shared window for the whole workgroup. The strands span the whole brush,
  so its bounds would be too wide.

### Step 3: subgroups

- **Feature.** `wgpu::Features::SUBGROUP` becomes an optional feature, next to
  `TIMESTAMP_QUERY` in `gpu.rs`.
- **What changes.** With a strand's 8 lanes aligned within one subgroup, broadcasts and
  reductions become `subgroupShuffle` and `subgroupShuffleXor` (a 3-step butterfly over 8
  lanes). The strand state leaves workgroup memory, and the strand's internal
  `workgroupBarrier`s go away.
- **Workgroup size.** It can shrink to the subgroup size, which raises occupancy.
- **Variants.** `shader.rs` builds two variants of the solve, the way it builds the snippets
  today, and picks one by the feature. Without the feature, the step 1 path runs. The
  equivalence test runs both.

### Step 4: remaining serial work

Each item stays only if it measures faster:

- **Cholesky by column.** Lane 0 takes the pivot and the other lanes update their rows. That
  is 7 synchronisations per solve, so it may cost more than the serial version.
- **Quaternion scan.** A Hillis-Steele scan (3 steps) replaces the redundant prefix per lane.

### Step 5: occupancy and layout

- Try 4, 8 and 16 strands per workgroup.
- Read register use and spills from the RADV/ACO shader statistics.
- Check that `position_slot` keeps a strand's 8 vertices contiguous, so its loads and stores
  coalesce. It does today: `state_slot(bristle) + vertex`.

## Results

The table fills in as the steps land. Rows: research HEAD, then one row per kept or reverted
step. Columns: gate mean and p95 for `brush, default size`, `blender, default size over
impasto` and `long brush stroke over impasto`, plus Bristles µs per dab in the default
scenario.

| step | default mean / p95 | blender mean / p95 | long stroke mean / p95 | Bristles µs/dab | kept |
|---|---|---|---|---|---|
| research HEAD | 1.40 / 1.66 | 1.35 / 1.57 | 1.34 / 1.58 | — | — |

## Outcome

The final comment on #62 gives the table and one verdict:

- **fits the gate:** phase 1 continues on this solve;
- **fits a stated budget:** the budget is renegotiated to the measured number;
- **does not fit:** the next decision is a physics change (fewer hinges, a cheaper floor, a
  closed-form single contact) or another model.
