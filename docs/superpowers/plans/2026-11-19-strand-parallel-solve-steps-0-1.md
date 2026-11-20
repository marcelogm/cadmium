# Strand-parallel solve, steps 0 and 1: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Measure where the quasi-static bristle solve spends its GPU time, then spread each strand's solve over 8 lanes of a workgroup without changing its physics.

**Architecture:**
- **Instrumentation.** An optional per-stage timer in the engine encodes each dab stage in its own timed compute pass, and `bristle_cost` reports it.
- **Reference solve.** The serial solve stays in the module as `solve_reference`. A hidden engine switch selects it, so an integration test can compare the two solves on the same strokes.
- **Grouped solve.** The new `solve` maps lane *v* of a strand's group to vertex/hinge *v*. It keeps the strand state in workgroup memory and builds every value from the same terms, in the same order, as the serial solve.

**Tech Stack:** Rust 2024, wgpu 30 (Vulkan), WGSL compute shaders, `cargo test` on the GPU.

Spec: `docs/superpowers/specs/2026-11-19-strand-parallel-solve-design.md`. Steps 2–5 of the spec get their own plans, written after step 1 is measured.

---

## Working environment

- Worktree: `/home/marcelo/Desenvolvimento/cadmium-qs`, branch `perf/strand-parallel-solve`.
- **Every `cargo` command runs in the `cadmium-dev` Docker image, never on the host.** The host has no GTK dev packages. Docker needs the command sandbox disabled. Prefix each command with:

```bash
cd /home/marcelo/Desenvolvimento/cadmium-qs && $(cat target/dk) bash -c '<cargo command>'
```

`target/dk` already holds the `docker run …` prefix: GPU device, groups, repo mount, and `CARGO_HOME`. If it is missing, recreate it:

```bash
cd /home/marcelo/Desenvolvimento/cadmium-qs && G=$(stat -c %g /dev/dri/* | sort -u | sed 's/^/--group-add /' | tr '\n' ' ') && mkdir -p target && echo "docker run --rm --device /dev/dri $G --security-opt label=disable -u $(id -u):$(id -g) -e HOME=/tmp -e CARGO_HOME=$HOME/.cargo -v $HOME/.cargo:$HOME/.cargo -v $PWD:$PWD -w $PWD cadmium-dev" > target/dk
```

- The shell prints a zoxide warning on every command. Ignore it.
- Commit messages: short conventional-commit subject. **No `Co-Authored-By` or any other attribution line.**
- Before each commit, run `cargo fmt --all` and `cargo clippy --release -p cadmium-engine --all-targets -- -D warnings` (in Docker).

## File structure

| File | Change | Responsibility |
|---|---|---|
| `crates/cadmium-engine/src/engine/stage_timer.rs` | create | `DabStageTimer`: timestamp pairs per dab stage, tally per stage |
| `crates/cadmium-engine/src/engine/stage_timer/tests.rs` | create | unit test of the tally |
| `crates/cadmium-engine/src/engine.rs` | modify | `stage_timer` field, timed encode path, `mod stage_timer` |
| `crates/cadmium-engine/src/engine/pass.rs` | modify | `DabPasses::encode_timed`, `DabPasses::choose_strand_solve` |
| `crates/cadmium-engine/tests/stage_times.rs` | create | integration test of the stage timer |
| `crates/cadmium-engine/examples/bristle_cost/stages.rs` | create | `CADMIUM_BENCH_STAGES` tally and report |
| `crates/cadmium-engine/examples/bristle_cost/bench.rs` | modify | resolve and read stage times each frame |
| `crates/cadmium-engine/examples/bristle_cost/main.rs` | modify | enable stage timing, print per scenario, doc |
| `crates/cadmium-engine/src/engine/pass/bristle_step.rs` | modify | `StrandSolve` (grouped or serial), two solve pipelines |
| `crates/cadmium-engine/src/engine/pass/bristle_step/tests.rs` | create | unit test of `StrandSolve` |
| `crates/cadmium-engine/src/shaders/bristle_step.wgsl` | modify | serial entry renamed `solve_reference` |
| `crates/cadmium-engine/src/shaders/bristle_contacts.wgsl` | modify | factor + substitution extracted as `solved_by_cholesky` |
| `crates/cadmium-engine/src/shaders/bristle_strand_groups.wgsl` | create | the grouped `solve` entry point |
| `crates/cadmium-engine/src/engine/bristles/state.rs` | modify | `BRISTLE_STRANDS_PER_GROUP` |
| `crates/cadmium-engine/src/engine/bristles.rs` | modify | re-export it |
| `crates/cadmium-engine/src/engine/shader.rs` | modify | emit it as a WGSL constant |
| `crates/cadmium-engine/tests/bristles/parallel_solve.rs` | create | grouped vs serial equivalence |
| `crates/cadmium-engine/tests/bristles/main.rs` | modify | `mod parallel_solve;` |
| `docs/superpowers/specs/2026-11-19-strand-parallel-solve-design.md` | modify | results table |

---

### Task 1: Per-stage GPU timer in the engine

**Files:**
- Create: `crates/cadmium-engine/src/engine/stage_timer.rs`
- Create: `crates/cadmium-engine/src/engine/stage_timer/tests.rs`
- Modify: `crates/cadmium-engine/src/engine.rs`
- Modify: `crates/cadmium-engine/src/engine/pass.rs`
- Test: `crates/cadmium-engine/tests/stage_times.rs`

- [ ] **Step 1: Write the failing integration test**

Create `crates/cadmium-engine/tests/stage_times.rs`:

```rust
pub mod support;

use cadmium_engine::gpu::GpuContext;
use support::{line, session};

#[test]
fn timed_dab_stages_report_every_dab_of_the_frame_once() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    if !s.engine.time_dab_stages(&gpu) {
        eprintln!("skipped: the adapter has no timestamp queries");
        return;
    }
    let samples = line(128.0, 60.0, 300.0, 0.8, 20.0);
    s.begin_stroke(samples[0]);
    s.add_samples(&samples[1..]);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    s.encode_frame(&gpu, 0.0, &mut encoder);
    s.engine.resolve_dab_stage_times(&mut encoder);
    gpu.queue.submit([encoder.finish()]);

    let times = s.engine.take_dab_stage_times(&gpu).unwrap();
    let stage = |name: &str| {
        times
            .iter()
            .find(|time| time.stage == name)
            .unwrap_or_else(|| panic!("no {name} stage in {times:?}"))
    };
    assert!(stage("Bristles").dabs > 0, "the frame painted no dab");
    assert_eq!(stage("Bristles").dabs, stage("Exchange").dabs);
    assert!(stage("Bristles").gpu_ms > 0.0);
    assert!(
        s.engine.take_dab_stage_times(&gpu).unwrap().is_empty(),
        "a second read reports the same frame again"
    );
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --release -p cadmium-engine --test stage_times`
Expected: compile error, `no method named time_dab_stages found for struct WetOilEngine`.

- [ ] **Step 3: Write the tally unit test**

Create `crates/cadmium-engine/src/engine/stage_timer/tests.rs`:

```rust
use super::*;

#[test]
fn the_tally_sums_each_stage_over_its_dabs_in_first_seen_order() {
    let passes = [DabStage::Bristles, DabStage::Exchange, DabStage::Bristles];
    let ticks = [10, 30, 30, 35, 40, 70];
    let times = tally(&passes, &ticks, 1.0e6);
    assert_eq!(
        times,
        vec![
            DabStageTime {
                stage: "Bristles".to_string(),
                dabs: 2,
                gpu_ms: 50.0,
            },
            DabStageTime {
                stage: "Exchange".to_string(),
                dabs: 1,
                gpu_ms: 5.0,
            },
        ]
    );
}
```

With a period of 10⁶ ns per tick, a tick is a millisecond. The Bristles passes span 20 and 30 ticks, and the Exchange pass spans 5.

- [ ] **Step 4: Write the stage timer**

Create `crates/cadmium-engine/src/engine/stage_timer.rs`:

```rust
//! Times each stage of each dab in its own compute pass, to attribute a frame's GPU time to
//! the stages. Splitting the passes costs a little, so this is a measuring aid for
//! `bristle_cost`, never the app's path.

use super::WetOilEngine;
use crate::gpu::{GpuContext, GpuLost, read_mapped, staging_buffer};
use crate::tool::DabStage;

const MAX_QUERIES: u32 = 4096;
const QUERIES_PER_PASS: u32 = 2;
const MAX_TIMED_PASSES: usize = (MAX_QUERIES / QUERIES_PER_PASS) as usize;
const QUERY_BYTES: u64 = wgpu::QUERY_SIZE as u64;
const NANOSECONDS_PER_MILLISECOND: f64 = 1.0e6;

/// One stage's GPU time over the dabs timed since the last read.
#[derive(Clone, Debug, PartialEq)]
pub struct DabStageTime {
    pub stage: String,
    pub dabs: u32,
    pub gpu_ms: f64,
}

pub(crate) struct DabStageTimer {
    queries: wgpu::QuerySet,
    resolved: wgpu::Buffer,
    staging: wgpu::Buffer,
    period_ns: f64,
    passes: Vec<DabStage>,
    resolved_passes: usize,
}

impl DabStageTimer {
    /// A timer, or `None` on an adapter without timestamp queries.
    pub(crate) fn new(gpu: &GpuContext) -> Option<DabStageTimer> {
        if !gpu
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
        {
            return None;
        }
        let bytes = u64::from(MAX_QUERIES) * QUERY_BYTES;
        Some(DabStageTimer {
            queries: gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("dab_stage_timer"),
                ty: wgpu::QueryType::Timestamp,
                count: MAX_QUERIES,
            }),
            resolved: gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("dab_stage_timer_resolved"),
                size: bytes,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            staging: staging_buffer(gpu, "dab_stage_timer_staging", bytes),
            period_ns: f64::from(gpu.queue.get_timestamp_period()),
            passes: Vec::new(),
            resolved_passes: 0,
        })
    }

    /// The first query of the next timed pass, which runs `stage`; `None` once the query
    /// set is full, and that pass then runs untimed.
    pub(crate) fn claim(&mut self, stage: DabStage) -> Option<u32> {
        if self.passes.len() >= MAX_TIMED_PASSES {
            return None;
        }
        let first = self.passes.len() as u32 * QUERIES_PER_PASS;
        self.passes.push(stage);
        Some(first)
    }

    pub(crate) fn writes(&self, first: u32) -> wgpu::ComputePassTimestampWrites<'_> {
        wgpu::ComputePassTimestampWrites {
            query_set: &self.queries,
            beginning_of_pass_write_index: Some(first),
            end_of_pass_write_index: Some(first + 1),
        }
    }

    /// Copies every timestamp claimed so far to the staging buffer, after the work.
    pub(crate) fn resolve(&mut self, encoder: &mut wgpu::CommandEncoder) {
        let count = self.passes.len() as u32 * QUERIES_PER_PASS;
        if count == 0 {
            return;
        }
        encoder.resolve_query_set(&self.queries, 0..count, &self.resolved, 0);
        encoder.copy_buffer_to_buffer(
            &self.resolved,
            0,
            &self.staging,
            0,
            u64::from(count) * QUERY_BYTES,
        );
        self.resolved_passes = self.passes.len();
    }

    /// The stage times resolved and submitted since the last read; empty if none were.
    pub(crate) fn take(&mut self, gpu: &GpuContext) -> Result<Vec<DabStageTime>, GpuLost> {
        let passes = std::mem::take(&mut self.passes);
        let resolved = std::mem::replace(&mut self.resolved_passes, 0);
        if resolved == 0 {
            return Ok(Vec::new());
        }
        let used = resolved * QUERIES_PER_PASS as usize * QUERY_BYTES as usize;
        let ticks: Vec<u64> = read_mapped(gpu, &self.staging, |bytes| {
            bytemuck::cast_slice::<u8, u64>(&bytes[..used]).to_vec()
        })?;
        Ok(tally(&passes[..resolved], &ticks, self.period_ns))
    }
}

/// Each stage's dab count and summed time, in the order the stages first ran.
fn tally(passes: &[DabStage], ticks: &[u64], period_ns: f64) -> Vec<DabStageTime> {
    let mut times: Vec<DabStageTime> = Vec::new();
    for (index, stage) in passes.iter().enumerate() {
        let first = index * QUERIES_PER_PASS as usize;
        let gpu_ms = ticks[first + 1].saturating_sub(ticks[first]) as f64 * period_ns
            / NANOSECONDS_PER_MILLISECOND;
        let name = format!("{stage:?}");
        match times.iter_mut().find(|time| time.stage == name) {
            Some(time) => {
                time.dabs += 1;
                time.gpu_ms += gpu_ms;
            }
            None => times.push(DabStageTime {
                stage: name,
                dabs: 1,
                gpu_ms,
            }),
        }
    }
    times
}

impl WetOilEngine {
    /// Times every dab stage in its own compute pass from now on. A measuring aid for
    /// `bristle_cost`: false, and nothing changes, when the adapter has no timestamps.
    #[doc(hidden)]
    pub fn time_dab_stages(&mut self, gpu: &GpuContext) -> bool {
        self.stage_timer = DabStageTimer::new(gpu);
        self.stage_timer.is_some()
    }

    /// Resolves the stage times of the dabs encoded so far, at the end of a frame.
    #[doc(hidden)]
    pub fn resolve_dab_stage_times(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if let Some(timer) = self.stage_timer.as_mut() {
            timer.resolve(encoder);
        }
    }

    /// The stage times resolved since the last read, once their frame was submitted.
    #[doc(hidden)]
    pub fn take_dab_stage_times(&mut self, gpu: &GpuContext) -> Result<Vec<DabStageTime>, GpuLost> {
        match self.stage_timer.as_mut() {
            Some(timer) => timer.take(gpu),
            None => Ok(Vec::new()),
        }
    }
}

#[cfg(test)]
mod tests;
```

- [ ] **Step 5: Wire the timer into the engine**

In `crates/cadmium-engine/src/engine.rs`:

1. After `mod shader` and its `pub(crate)` (the `mod` list near the top), add:

```rust
mod stage_timer;
```

2. Next to the other `pub use` lines, add:

```rust
pub use stage_timer::DabStageTime;
```

3. Next to the other private `use` lines (after `use placement::…`), add:

```rust
use stage_timer::DabStageTimer;
```

4. In `struct WetOilEngine`, after `dip_pass: DipPass,`, add the field:

```rust
    stage_timer: Option<DabStageTimer>,
```

5. In `WetOilEngine::new`, after `dip_pass: DipPass::new(gpu),`, add:

```rust
            stage_timer: None,
```

6. In `encode_dabs` of `impl PaintEngine for WetOilEngine`, replace:

```rust
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("dab_exchange"),
            timestamp_writes: None,
        });
        self.dab_passes.encode(&bind_groups, &mut pass, &placements);
        drop(pass);
        mark_dab_regions(canvas, &placements);
```

with:

```rust
        if let Some(timer) = self.stage_timer.as_mut() {
            self.dab_passes
                .encode_timed(&bind_groups, encoder, &placements, timer);
        } else {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("dab_exchange"),
                timestamp_writes: None,
            });
            self.dab_passes.encode(&bind_groups, &mut pass, &placements);
        }
        mark_dab_regions(canvas, &placements);
```

In `crates/cadmium-engine/src/engine/pass.rs`:

1. Add `use super::stage_timer::DabStageTimer;` after `use super::placement::Placement;`.
2. In `impl DabPasses`, after `fn encode(…)`, add:

```rust
    /// `encode`, with each stage of each dab in its own pass timed by `timer`.
    pub(super) fn encode_timed(
        &self,
        bind_groups: &DabBindGroups,
        encoder: &mut wgpu::CommandEncoder,
        placements: &[Placement],
        timer: &mut DabStageTimer,
    ) {
        for (index, placement) in placements.iter().enumerate() {
            let dab = PlacedDab::at(index, placement);
            for stage in placement.profile.dab_stages {
                let first = timer.claim(*stage);
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("dab_stage_timed"),
                    timestamp_writes: first.map(|first| timer.writes(first)),
                });
                self.encode_stage(*stage, bind_groups, &mut pass, dab);
            }
        }
    }
```

- [ ] **Step 6: Run both tests to verify they pass**

Run: `cargo test --release -p cadmium-engine --lib stage_timer && cargo test --release -p cadmium-engine --test stage_times`
Expected: both PASS. On an adapter without timestamps the integration test prints `skipped`; the RX 9060 XT has them.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all
git add crates/cadmium-engine/src/engine/stage_timer.rs crates/cadmium-engine/src/engine/stage_timer/tests.rs crates/cadmium-engine/src/engine.rs crates/cadmium-engine/src/engine/pass.rs crates/cadmium-engine/tests/stage_times.rs
git commit -m "feat(engine): time each dab stage in its own pass on request"
```

---

### Task 2: Stage report in `bristle_cost`

**Files:**
- Create: `crates/cadmium-engine/examples/bristle_cost/stages.rs`
- Modify: `crates/cadmium-engine/examples/bristle_cost/bench.rs`
- Modify: `crates/cadmium-engine/examples/bristle_cost/main.rs`

- [ ] **Step 1: Write the tally with its unit test**

Create `crates/cadmium-engine/examples/bristle_cost/stages.rs`:

```rust
//! `CADMIUM_BENCH_STAGES=1`: each scenario also reports each dab stage's GPU time per dab.

use std::collections::BTreeMap;

use cadmium_engine::engine::DabStageTime;

const STAGES_VARIABLE: &str = "CADMIUM_BENCH_STAGES";
const MICROSECONDS_PER_MILLISECOND: f64 = 1000.0;

pub(crate) fn wanted() -> bool {
    std::env::var_os(STAGES_VARIABLE).is_some()
}

/// Each stage's dabs and GPU time over a scenario.
#[derive(Default)]
pub(crate) struct StageTally {
    totals: BTreeMap<String, (u64, f64)>,
}

impl StageTally {
    pub(crate) fn add(&mut self, times: &[DabStageTime]) {
        for time in times {
            let total = self.totals.entry(time.stage.clone()).or_default();
            total.0 += u64::from(time.dabs);
            total.1 += time.gpu_ms;
        }
    }

    /// Each stage's mean µs per dab, by stage name.
    pub(crate) fn per_dab_us(&self) -> Vec<(String, f64, u64)> {
        self.totals
            .iter()
            .filter(|(_, (dabs, _))| *dabs > 0)
            .map(|(stage, (dabs, ms))| {
                (
                    stage.clone(),
                    ms * MICROSECONDS_PER_MILLISECOND / *dabs as f64,
                    *dabs,
                )
            })
            .collect()
    }

    pub(crate) fn print(&self) {
        for (stage, us, dabs) in self.per_dab_us() {
            println!("  {stage:<10} {us:>8.1} µs/dab over {dabs} dabs");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tally_reports_microseconds_per_dab_by_stage() {
        let mut tally = StageTally::default();
        let time = |stage: &str, dabs, gpu_ms| DabStageTime {
            stage: stage.to_string(),
            dabs,
            gpu_ms,
        };
        tally.add(&[time("Bristles", 2, 0.4), time("Exchange", 2, 0.1)]);
        tally.add(&[time("Bristles", 2, 0.2)]);
        let per_dab = tally.per_dab_us();
        assert_eq!(per_dab.len(), 2);
        assert_eq!(per_dab[0].0, "Bristles");
        assert!((per_dab[0].1 - 150.0).abs() < 1e-9);
        assert_eq!(per_dab[0].2, 4);
        assert!((per_dab[1].1 - 50.0).abs() < 1e-9);
    }
}
```

- [ ] **Step 2: Wire it into the bench**

In `crates/cadmium-engine/examples/bristle_cost/main.rs`:

1. Add `mod stages;` after `mod scenarios;`.
2. Add `use stages::StageTally;` after `use scenarios::…;`.
3. In `measure`, add `stages: None,` to the `Bench { … }` literal, after `frames_after_lift: 0,`.
4. In `measure`, replace:

```rust
    bench.take_up(scenario);
    let costs = match scenario.motion {
```

with:

```rust
    bench.take_up(scenario);
    if stages::wanted() && bench.session.engine.time_dab_stages(gpu) {
        bench.stages = Some(StageTally::default());
    }
    let costs = match scenario.motion {
```

5. In `measure`, before the final `costs`, add:

```rust
    if let Some(tally) = &bench.stages {
        println!("{} stages:", scenario.name);
        tally.print();
    }
```

6. In the module doc at the top, after the `CADMIUM_PACED` paragraph, add:

```rust
//! `CADMIUM_BENCH_STAGES=1` also times each dab stage in its own compute pass and prints each
//! stage's GPU time per dab after each scenario. Splitting the passes costs a little, so the
//! frame times of such a run are not the ones the gate compares: use it to attribute, and
//! check the gate on a run without it.
```

In `crates/cadmium-engine/examples/bristle_cost/bench.rs`:

1. Add `use crate::stages::StageTally;` after `use crate::scenarios::Scenario;`.
2. Add a field to `struct Bench`, after `frames_after_lift`:

```rust
    pub(crate) stages: Option<StageTally>,
```

3. In `fn frame`, replace:

```rust
        let gpu_ms = finish_frame(self.gpu, self.timestamps, encoder);
```

with:

```rust
        if self.stages.is_some() {
            self.session.engine.resolve_dab_stage_times(&mut encoder);
        }
        let gpu_ms = finish_frame(self.gpu, self.timestamps, encoder);
        if let Some(tally) = self.stages.as_mut() {
            let times = self
                .session
                .engine
                .take_dab_stage_times(self.gpu)
                .expect("the GPU stays available while measuring");
            tally.add(&times);
        }
```

`take_up` runs before the tally starts, so the impasto layer and the take-up are not counted.

- [ ] **Step 3: Run the unit test and a filtered stage run**

Run: `cargo test --release -p cadmium-engine --example bristle_cost`
Expected: `the_tally_reports_microseconds_per_dab_by_stage` PASS.

Run: `CADMIUM_BENCH_STAGES=1 CADMIUM_SCENARIO="brush, default size" cargo run --release -p cadmium-engine --example bristle_cost`
Expected: the usual row, then `brush, default size stages:` with one line per stage (Bristles, Contact, Exchange, …), each with a µs/dab figure and the same dab count.

- [ ] **Step 4: Commit**

```bash
cargo fmt --all
git add crates/cadmium-engine/examples/bristle_cost/
git commit -m "feat(bench): report each dab stage's GPU time per dab"
```

- [ ] **Step 5: Record the research HEAD's stage times**

Run: `CADMIUM_BENCH_STAGES=1 CADMIUM_BENCH_REPEATS=3 cargo run --release -p cadmium-engine --example bristle_cost 2>&1 | tee target/stages-head.txt`

Note the Bristles µs/dab for `brush, default size`, `blender, default size over impasto` and `long brush stroke over impasto`. With repeats each scenario prints once per pass, so take the median of the three. Put the default-size figure into the `research HEAD` row of the results table in the spec, Bristles µs/dab column. Commit it with the next spec edit (Task 6).

---

### Task 3: Keep the serial solve as `solve_reference`, selectable

**Files:**
- Modify: `crates/cadmium-engine/src/shaders/bristle_step.wgsl`
- Modify: `crates/cadmium-engine/src/engine/pass/bristle_step.rs`
- Create: `crates/cadmium-engine/src/engine/pass/bristle_step/tests.rs`
- Modify: `crates/cadmium-engine/src/engine/pass.rs`
- Modify: `crates/cadmium-engine/src/engine.rs`
- Test: `crates/cadmium-engine/tests/bristles/parallel_solve.rs`, `crates/cadmium-engine/tests/bristles/main.rs`

- [ ] **Step 1: Write the equivalence test**

Create `crates/cadmium-engine/tests/bristles/parallel_solve.rs`:

```rust
use cadmium_engine::canvas::StateTexture;
use cadmium_engine::engine::BristleSnapshot;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;

use crate::support::{drag, line, session};

const WIDTH: usize = 512;
const HEIGHT: usize = 256;
const CENTRE_Y: f64 = 128.0;
const RIDGE_PERIOD_PX: usize = 16;
const RIDGE_WIDTH_PX: usize = 6;
const RIDGE_HEIGHT_UM: f32 = 1200.0;
const TILT_DEG: f32 = 20.0;
/// The two solves add the same terms in the same order, so they differ only where the
/// compiler fuses a multiply-add one way in one and the other way in the other.
const POSITION_TOLERANCE_PX: f32 = 1.0e-3;
const FORCE_TOLERANCE_SHARE: f32 = 1.0e-3;
const FORCE_FLOOR_N: f32 = 1.0e-9;

fn on_ridges(gpu: &GpuContext, reference: bool) -> PaintSession {
    let mut s = session(gpu);
    let heights: Vec<f32> = (0..WIDTH * HEIGHT)
        .map(|pixel| {
            if pixel % WIDTH % RIDGE_PERIOD_PX < RIDGE_WIDTH_PX {
                RIDGE_HEIGHT_UM
            } else {
                0.0
            }
        })
        .collect();
    s.canvas.write_f32(gpu, StateTexture::DryHeight, &heights);
    s.engine.use_reference_bristle_solve(reference);
    s
}

fn assert_same_strands(reference: &BristleSnapshot, grouped: &BristleSnapshot) {
    assert_eq!(reference.chains_px.len(), grouped.chains_px.len());
    for (bristle, (serial, parallel)) in reference.chains_px.iter().zip(&grouped.chains_px).enumerate() {
        for (vertex, (a, b)) in serial.iter().zip(parallel).enumerate() {
            for axis in 0..3 {
                assert!(
                    (a[axis] - b[axis]).abs() <= POSITION_TOLERANCE_PX,
                    "bristle {bristle} vertex {vertex}: reference {a:?}, grouped {b:?}"
                );
            }
        }
    }
    for (bristle, (a, b)) in reference
        .normal_forces_n
        .iter()
        .zip(&grouped.normal_forces_n)
        .enumerate()
    {
        let scale = a.abs().max(b.abs()).max(FORCE_FLOOR_N);
        assert!(
            (a - b).abs() <= FORCE_TOLERANCE_SHARE * scale,
            "bristle {bristle}: reference {a} N, grouped {b} N"
        );
    }
}

fn snapshots(gpu: &GpuContext, reference: &PaintSession, grouped: &PaintSession) -> (BristleSnapshot, BristleSnapshot) {
    (
        reference.engine.bristle_snapshot(gpu).unwrap(),
        grouped.engine.bristle_snapshot(gpu).unwrap(),
    )
}

#[test]
fn the_grouped_solve_matches_the_serial_one_over_ridges() {
    let gpu = GpuContext::new().unwrap();
    let mut reference = on_ridges(&gpu, true);
    let mut grouped = on_ridges(&gpu, false);
    // Short, so a last-bit difference has few dabs to grow over.
    let samples = line(CENTRE_Y, 60.0, 140.0, 0.9, TILT_DEG);
    drag(&gpu, &mut reference, &samples);
    drag(&gpu, &mut grouped, &samples);
    let (serial, parallel) = snapshots(&gpu, &reference, &grouped);
    assert!(
        serial.normal_forces_n.iter().any(|&force| force > 0.0),
        "the stroke pressed no strand onto the ridges"
    );
    assert_same_strands(&serial, &parallel);
}

#[test]
fn the_grouped_solve_matches_the_serial_one_from_a_worn_brush() {
    let gpu = GpuContext::new().unwrap();
    let mut reference = on_ridges(&gpu, true);
    let mut grouped = on_ridges(&gpu, true);
    // Both wear the brush with the serial solve, so they start the next stroke alike.
    let wearing = line(CENTRE_Y, 60.0, 300.0, 0.9, TILT_DEG);
    drag(&gpu, &mut reference, &wearing);
    drag(&gpu, &mut grouped, &wearing);
    grouped.engine.use_reference_bristle_solve(false);
    let samples = line(CENTRE_Y, 300.0, 360.0, 0.6, TILT_DEG);
    drag(&gpu, &mut reference, &samples);
    drag(&gpu, &mut grouped, &samples);
    let (serial, parallel) = snapshots(&gpu, &reference, &grouped);
    assert!(serial.normal_forces_n.iter().any(|&force| force > 0.0));
    assert_same_strands(&serial, &parallel);
}
```

In `crates/cadmium-engine/tests/bristles/main.rs`, add `mod parallel_solve;` after `mod paint;`, keeping the list alphabetical.

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --release -p cadmium-engine --test bristles parallel_solve`
Expected: compile error, `no method named use_reference_bristle_solve`.

- [ ] **Step 3: Add the `solve_reference` entry point**

In `crates/cadmium-engine/src/shaders/bristle_step.wgsl`, replace the `solve` entry point at the end of the file:

```wgsl
/// Solves one strand along the path `touch` recorded. No barriers, so the workgroup can be
/// small and each thread gets the registers the solve needs.
@compute @workgroup_size(BRISTLE_SOLVE_WORKGROUP_SIZE)
fn solve(@builtin(global_invocation_id) id: vec3<u32>) {
```

with:

```wgsl
/// Solves one strand per thread along the path `touch` recorded: the serial solve the
/// grouped `solve` is checked against. No barriers, so the workgroup can be small and each
/// thread gets the registers the solve needs.
@compute @workgroup_size(BRISTLE_SOLVE_WORKGROUP_SIZE)
fn solve_reference(@builtin(global_invocation_id) id: vec3<u32>) {
    solve_serially(id.x);
}

/// `solve_reference` under the name the app dispatches, until the grouped solve replaces it.
@compute @workgroup_size(BRISTLE_SOLVE_WORKGROUP_SIZE)
fn solve(@builtin(global_invocation_id) id: vec3<u32>) {
    solve_serially(id.x);
}

fn solve_serially(bristle: u32) {
```

Then, in that function's body, delete the line `let bristle = id.x;`. The rest of the body is unchanged. It still starts with `if bristle >= dab.bristle_total { return; }`.

- [ ] **Step 4: Write the `StrandSolve` unit test**

Create `crates/cadmium-engine/src/engine/pass/bristle_step/tests.rs`:

```rust
use super::*;

#[test]
fn each_strand_solve_names_its_own_entry_point() {
    assert_eq!(StrandSolve::Grouped.entry_point(), "solve");
    assert_eq!(StrandSolve::Serial.entry_point(), "solve_reference");
}

#[test]
fn the_serial_solve_covers_every_strand_one_per_thread() {
    let [x, y] = StrandSolve::Serial.workgroups();
    assert_eq!(y, 1);
    assert_eq!(x * BRISTLE_SOLVE_WORKGROUP_SIZE, MAX_BRISTLES as u32);
}
```

- [ ] **Step 5: Add `StrandSolve` and the second pipeline**

In `crates/cadmium-engine/src/engine/pass/bristle_step.rs`:

1. Replace:

```rust
const SINGLE_WORKGROUP: [u32; 2] = [1, 1];
const SOLVE_WORKGROUPS: [u32; 2] = [
    MAX_BRISTLES.div_ceil(BRISTLE_SOLVE_WORKGROUP_SIZE as usize) as u32,
    1,
];
const FIRST_RECORD_OFFSET: wgpu::DynamicOffset = 0;

pub(super) struct BristleStepPass {
    compute: ComputeStage,
    touch: wgpu::ComputePipeline,
    solve: wgpu::ComputePipeline,
}
```

with:

```rust
const SINGLE_WORKGROUP: [u32; 2] = [1, 1];
const SERIAL_WORKGROUPS: [u32; 2] = [
    MAX_BRISTLES.div_ceil(BRISTLE_SOLVE_WORKGROUP_SIZE as usize) as u32,
    1,
];
const FIRST_RECORD_OFFSET: wgpu::DynamicOffset = 0;

/// Which strand solve the step dispatches. Both give the same strands; `Serial` is kept so
/// tests can check `Grouped` against it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StrandSolve {
    Grouped,
    Serial,
}

impl StrandSolve {
    fn entry_point(self) -> &'static str {
        match self {
            StrandSolve::Grouped => "solve",
            StrandSolve::Serial => "solve_reference",
        }
    }

    fn workgroups(self) -> [u32; 2] {
        match self {
            StrandSolve::Grouped | StrandSolve::Serial => SERIAL_WORKGROUPS,
        }
    }
}

pub(super) struct BristleStepPass {
    compute: ComputeStage,
    touch: wgpu::ComputePipeline,
    grouped: wgpu::ComputePipeline,
    serial: wgpu::ComputePipeline,
    strand_solve: StrandSolve,
}
```

2. In `new`, replace:

```rust
        BristleStepPass {
            touch: compute.pipeline(gpu, "bristle_touch", "touch"),
            solve: compute.pipeline(gpu, "bristle_solve", "solve"),
            compute,
        }
```

with:

```rust
        BristleStepPass {
            touch: compute.pipeline(gpu, "bristle_touch", "touch"),
            grouped: compute.pipeline(gpu, "bristle_solve", StrandSolve::Grouped.entry_point()),
            serial: compute.pipeline(
                gpu,
                "bristle_solve_reference",
                StrandSolve::Serial.entry_point(),
            ),
            strand_solve: StrandSolve::Grouped,
            compute,
        }
```

3. Add a method in `impl BristleStepPass`:

```rust
    pub(super) fn choose(&mut self, strand_solve: StrandSolve) {
        self.strand_solve = strand_solve;
    }
```

4. Replace `dispatch_both`:

```rust
    /// The touch height and the dab's path first, then one thread per strand.
    fn dispatch_both(&self, pass: &mut wgpu::ComputePass<'_>) {
        dispatch(pass, &self.touch, SINGLE_WORKGROUP);
        dispatch(pass, &self.solve, SOLVE_WORKGROUPS);
    }
```

with:

```rust
    /// The touch height and the dab's path first, then the chosen strand solve.
    fn dispatch_both(&self, pass: &mut wgpu::ComputePass<'_>) {
        dispatch(pass, &self.touch, SINGLE_WORKGROUP);
        let solve = match self.strand_solve {
            StrandSolve::Grouped => &self.grouped,
            StrandSolve::Serial => &self.serial,
        };
        dispatch(pass, solve, self.strand_solve.workgroups());
    }
```

5. At the end of the file, add:

```rust
#[cfg(test)]
mod tests;
```

`compute.pipeline(gpu, label, entry)` takes the entry point as `&str`. If its signature is different, check `crates/cadmium-engine/src/compute.rs` and adapt the call, not the design.

In `crates/cadmium-engine/src/engine/pass.rs`:

1. Change `mod bristle_step;` to `pub(super) mod bristle_step;`.
2. Add `use bristle_step::StrandSolve;` after `use bristle_step::BristleStepPass;`.
3. Add to `impl DabPasses`:

```rust
    pub(super) fn choose_strand_solve(&mut self, strand_solve: StrandSolve) {
        self.bristles.choose(strand_solve);
    }
```

In `crates/cadmium-engine/src/engine.rs`:

1. Add `use pass::bristle_step::StrandSolve;` after `use pass::{DabPasses, DabResources};`.
2. Add to the inherent `impl WetOilEngine` block (the one with `resize_tip`):

```rust
    /// Solves the strands one per thread, with the serial solve, instead of the grouped
    /// one: for tests that check the two agree.
    #[doc(hidden)]
    pub fn use_reference_bristle_solve(&mut self, reference: bool) {
        let strand_solve = if reference {
            StrandSolve::Serial
        } else {
            StrandSolve::Grouped
        };
        self.dab_passes.choose_strand_solve(strand_solve);
    }
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test --release -p cadmium-engine --lib bristle_step && cargo test --release -p cadmium-engine --test bristles parallel_solve`
Expected: all PASS. The equivalence tests pass trivially for now, because both entry points run the serial solve. They become meaningful in Task 5.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all
git add crates/cadmium-engine/src/shaders/bristle_step.wgsl crates/cadmium-engine/src/engine/pass/bristle_step.rs crates/cadmium-engine/src/engine/pass/bristle_step/tests.rs crates/cadmium-engine/src/engine/pass.rs crates/cadmium-engine/src/engine.rs crates/cadmium-engine/tests/bristles/parallel_solve.rs crates/cadmium-engine/tests/bristles/main.rs
git commit -m "test(engine): keep the serial strand solve as a reference to check against"
```

---

### Task 4: Extract the Cholesky factor and substitution

The grouped solve builds its contact matrix from workgroup memory. Pulling the factor and the substitution out lets both solves share them.

**Files:**
- Modify: `crates/cadmium-engine/src/shaders/bristle_contacts.wgsl`

- [ ] **Step 1: Run the lab tests as the guard**

Run: `cargo test --release -p cadmium-engine --lib lab`
Expected: PASS (cantilever, tip contact, buckling, step halving, ramp). These tests guard the refactor.

- [ ] **Step 2: Split `cholesky_solved`**

In `crates/cadmium-engine/src/shaders/bristle_contacts.wgsl`, replace the whole `cholesky_solved` function with:

```wgsl
/// Solves the ridged system of the `count` active rows by Cholesky, in place: the same
/// arithmetic as the full system with the inactive rows left out, since their entries are
/// zero. Returns the forces in the order of `rows`.
fn cholesky_solved(problem: ptr<function, ContactProblem>, rows: ActiveRows, count: u32) -> PerContact {
    var lower: ContactMatrix;
    var rhs: PerContact;
    for (var i = 0u; i < count; i++) {
        rhs[i] = (*problem).gaps[rows[i]];
        for (var j = 0u; j < i; j++) {
            lower[i][j] = entry(problem, rows[i], rows[j]);
        }
        lower[i][i] = (*problem).diagonal[rows[i]] + (*problem).ridge;
    }
    return solved_by_cholesky(lower, rhs, count);
}

/// Solves L·Lᵀ·x = `rhs` for the first `count` rows, factoring the lower triangle of
/// `matrix` (its diagonal included) in place.
fn solved_by_cholesky(matrix: ContactMatrix, rhs: PerContact, count: u32) -> PerContact {
    var lower = matrix;
    var x = rhs;
    for (var column = 0u; column < count; column++) {
        var pivot = lower[column][column];
        for (var k = 0u; k < column; k++) {
            pivot -= lower[column][k] * lower[column][k];
        }
        let root = sqrt(max(pivot, MIN_PIVOT));
        lower[column][column] = root;
        for (var row = column + 1u; row < count; row++) {
            var value = lower[row][column];
            for (var k = 0u; k < column; k++) {
                value -= lower[row][k] * lower[column][k];
            }
            lower[row][column] = value / root;
        }
    }
    for (var row = 0u; row < count; row++) {
        for (var k = 0u; k < row; k++) {
            x[row] -= lower[row][k] * x[k];
        }
        x[row] /= lower[row][row];
    }
    for (var step = 0u; step < count; step++) {
        let row = count - 1u - step;
        for (var k = row + 1u; k < count; k++) {
            x[row] -= lower[k][row] * x[k];
        }
        x[row] /= lower[row][row];
    }
    return x;
}
```

The factor and substitution loops are copied verbatim from the old body. Only the construction moved.

- [ ] **Step 3: Run the guards**

Run: `cargo test --release -p cadmium-engine --lib lab && cargo test --release -p cadmium-engine --test bristles parallel_solve`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/cadmium-engine/src/shaders/bristle_contacts.wgsl
git commit -m "refactor(engine): share the contact Cholesky between strand solves"
```

---

### Task 5: The grouped strand solve

**Files:**
- Create: `crates/cadmium-engine/src/shaders/bristle_strand_groups.wgsl`
- Modify: `crates/cadmium-engine/src/shaders/bristle_step.wgsl`
- Modify: `crates/cadmium-engine/src/engine/bristles/state.rs`
- Modify: `crates/cadmium-engine/src/engine/bristles.rs`
- Modify: `crates/cadmium-engine/src/engine/shader.rs`
- Modify: `crates/cadmium-engine/src/engine/pass/bristle_step.rs`
- Modify: `crates/cadmium-engine/src/engine/pass/bristle_step/tests.rs`

- [ ] **Step 1: Extend the `StrandSolve` unit test**

Append to `crates/cadmium-engine/src/engine/pass/bristle_step/tests.rs`:

```rust
#[test]
fn the_grouped_solve_covers_every_strand_with_a_lane_per_vertex() {
    let [x, y] = StrandSolve::Grouped.workgroups();
    assert_eq!(y, 1);
    assert_eq!(x * BRISTLE_STRANDS_PER_GROUP, MAX_BRISTLES as u32);
    assert_eq!(
        BRISTLE_STRANDS_PER_GROUP * BRISTLE_VERTICES as u32,
        BRISTLE_SOLVE_WORKGROUP_SIZE
    );
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --release -p cadmium-engine --lib bristle_step`
Expected: compile error, `cannot find value BRISTLE_STRANDS_PER_GROUP`.

- [ ] **Step 3: Add the constant**

In `crates/cadmium-engine/src/engine/bristles/state.rs`, after the `BRISTLES_PER_THREAD` line, add:

```rust
/// Strands the grouped solve fits in one workgroup: one lane per vertex of each.
pub(crate) const BRISTLE_STRANDS_PER_GROUP: u32 =
    BRISTLE_SOLVE_WORKGROUP_SIZE / BRISTLE_VERTICES as u32;
```

and next to the other `const _: () = assert!(…)` lines:

```rust
const _: () = assert!(BRISTLE_STRANDS_PER_GROUP * BRISTLE_VERTICES as u32 == BRISTLE_SOLVE_WORKGROUP_SIZE);
const _: () = assert!(MAX_BRISTLES as u32 % BRISTLE_STRANDS_PER_GROUP == 0);
```

In `crates/cadmium-engine/src/engine/bristles.rs`, add `BRISTLE_STRANDS_PER_GROUP` to the `pub(crate) use state::{…}` list, after `BRISTLE_STATE_VECTORS`.

In `crates/cadmium-engine/src/engine/shader.rs`:
1. Add `BRISTLE_STRANDS_PER_GROUP` to the `use super::bristles::{…}` list.
2. In `bristle_constants`, after the `BRISTLE_SOLVE_WORKGROUP_SIZE` line, add:

```rust
         const BRISTLE_STRANDS_PER_GROUP: u32 = {BRISTLE_STRANDS_PER_GROUP}u;\n\
```

In `crates/cadmium-engine/src/engine/pass/bristle_step.rs`:
1. Change the import to `use crate::engine::bristles::{BRISTLE_SOLVE_WORKGROUP_SIZE, BRISTLE_STRANDS_PER_GROUP, MAX_BRISTLES};`.
2. In `tests.rs`, add `use crate::engine::bristles::BRISTLE_VERTICES;` at the top.
3. Replace `workgroups`:

```rust
    fn workgroups(self) -> [u32; 2] {
        match self {
            StrandSolve::Grouped => [MAX_BRISTLES as u32 / BRISTLE_STRANDS_PER_GROUP, 1],
            StrandSolve::Serial => SERIAL_WORKGROUPS,
        }
    }
```

4. In `new`, append the grouped shader to the module body. Replace:

```rust
            &shader_source(
                &BRISTLE_SNIPPETS,
                include_str!("../../shaders/bristle_step.wgsl"),
            ),
```

with:

```rust
            &shader_source(
                &BRISTLE_SNIPPETS,
                &format!(
                    "{}\n{}",
                    include_str!("../../shaders/bristle_step.wgsl"),
                    include_str!("../../shaders/bristle_strand_groups.wgsl"),
                ),
            ),
```

- [ ] **Step 4: Remove the temporary serial `solve`**

In `crates/cadmium-engine/src/shaders/bristle_step.wgsl`, delete this block, which Task 3 added:

```wgsl
/// `solve_reference` under the name the app dispatches, until the grouped solve replaces it.
@compute @workgroup_size(BRISTLE_SOLVE_WORKGROUP_SIZE)
fn solve(@builtin(global_invocation_id) id: vec3<u32>) {
    solve_serially(id.x);
}
```

- [ ] **Step 5: Write the grouped solve**

Create `crates/cadmium-engine/src/shaders/bristle_strand_groups.wgsl`:

```wgsl
// The strand solve of `solve_reference`, spread over the lanes of a workgroup. Each strand
// gets BRISTLE_VERTICES lanes, and lane v owns vertex v, hinge v and contact row v (vertex
// v + 1). Every value is built from the same terms in the same order as the serial solve,
// so the two agree but for the compiler's freedom to fuse multiply-adds. Lane 0 of a strand
// keeps the strand's serial work: the Cholesky, the active-set decisions, the cap and the
// force sum. Phases are separated by workgroup barriers, so every loop that holds one runs
// a workgroup-uniform count.

alias GroupVectors = array<array<vec3<f32>, BRISTLE_VERTICES>, BRISTLE_STRANDS_PER_GROUP>;
alias GroupHinges = array<array<vec3<f32>, BRISTLE_SEGMENTS>, BRISTLE_STRANDS_PER_GROUP>;
alias GroupRows = array<array<f32, BRISTLE_SEGMENTS>, BRISTLE_STRANDS_PER_GROUP>;
alias GroupFlags = array<array<u32, BRISTLE_SEGMENTS>, BRISTLE_STRANDS_PER_GROUP>;

var<workgroup> group_chain: GroupVectors;
var<workgroup> group_normals: GroupVectors;
var<workgroup> group_heights: array<array<f32, BRISTLE_VERTICES>, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_edges: GroupHinges;
var<workgroup> group_short: GroupFlags;
var<workgroup> group_turns: GroupHinges;
var<workgroup> group_rise_turns: GroupHinges;
var<workgroup> group_gaps: GroupRows;
var<workgroup> group_diagonal: GroupRows;
var<workgroup> group_forces: GroupRows;
var<workgroup> group_engaged: GroupFlags;
var<workgroup> group_added: GroupFlags;
var<workgroup> group_matrix: array<ContactMatrix, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_ridge: array<f32, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_scale: array<f32, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_solving: array<u32, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_dropped: array<u32, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_settled: array<u32, BRISTLE_STRANDS_PER_GROUP>;
var<workgroup> group_unsettled: u32;

struct Lane {
    group: u32,
    strand: u32,
    vertex: u32,
    bristle: u32,
    live: bool,
};

/// What one iteration leaves the lane: its vertex, and on lane 0 the canvas's force, N.
struct Stepped {
    at: vec3<f32>,
    force: f32,
};

fn lane_of(group: u32, local: u32) -> Lane {
    let strand = local / BRISTLE_VERTICES;
    let bristle = group * BRISTLE_STRANDS_PER_GROUP + strand;
    return Lane(group, strand, local % BRISTLE_VERTICES, bristle, bristle < dab.bristle_total);
}

/// Lane 0 of a live strand, where the strand's serial work runs.
fn leads(lane: Lane) -> bool {
    return lane.live && lane.vertex == 0u;
}

/// A live lane with a hinge (and a contact row) of its own: every lane but the tip's.
fn owns_hinge(lane: Lane) -> bool {
    return lane.live && lane.vertex < BRISTLE_SEGMENTS;
}

/// The lane's vertex as the dab finds it: hung at rest after a reset, else where the last
/// dab left it (`place_at_rest`, `held_chain`).
fn held_position(lane: Lane, start: Pose) -> vec3<f32> {
    if dab.bristle_reset != 0u {
        var local = rest_chain_local(rests[lane.bristle]);
        return from_ferrule(local[lane.vertex], start);
    }
    return states[position_slot(lane.bristle, lane.vertex)].xyz;
}

/// Publishes the lane's vertex and the floor under it (`floors_under`, `floor_normals`).
fn place_vertex(lane: Lane, at: vec3<f32>, sampled: vec3<f32>, radius_px: f32) {
    var height = 0.0;
    var slope = vec2<f32>(0.0);
    if lane.vertex > 0u {
        height = swept_floor_px(sampled.xy, at.xy, radius_px);
        slope = floor_slope(at.xy, radius_px);
    }
    group_chain[lane.strand][lane.vertex] = at;
    group_heights[lane.strand][lane.vertex] = height;
    group_normals[lane.strand][lane.vertex] = normalize(vec3<f32>(-slope, 1.0));
}

/// Edge `lane.vertex` (`edges_of`), with a short edge marked for `mend_short_edges`.
fn place_edge(lane: Lane) {
    let s = lane.strand;
    let edge = lane.vertex;
    let span = group_chain[s][edge + 1u] - group_chain[s][edge];
    let size = length(span);
    group_edges[s][edge] = span / max(size, MIN_SEGMENT_SPAN_PX);
    group_short[s][edge] = select(1u, 0u, size > MIN_SEGMENT_SPAN_PX);
}

/// A short edge takes the heading of the edge above it, as `edges_of` does.
fn mend_short_edges(strand: u32, clamp: vec3<f32>) {
    var previous = clamp;
    for (var edge = 0u; edge < BRISTLE_SEGMENTS; edge++) {
        if group_short[strand][edge] != 0u {
            group_edges[strand][edge] = previous;
        }
        previous = group_edges[strand][edge];
    }
}

/// Hinge `lane.vertex`'s step of the bending energy alone (`free_turns`).
fn free_turn(lane: Lane, shape: StrandShape) -> vec3<f32> {
    let s = lane.strand;
    let hinge = lane.vertex;
    var before = shape.clamp;
    if hinge > 0u {
        before = group_edges[s][hinge - 1u];
    }
    let curl = transported(shape.curl, shape.clamp, before);
    let rest = select(vec3<f32>(0.0), curl, hinge > 0u);
    let tip = group_chain[s][BRISTLE_VERTICES - 1u];
    let lever_m = (tip - group_chain[s][hinge]) * METRES_PER_PX;
    let loaded = shape.compliances[hinge] * cross(lever_m, shape.tip_force_n);
    return rest - rotation_between(before, group_edges[s][hinge]) + loaded;
}

/// How far inside the relief `vertex` would end after the free turns (`gaps_after`).
fn gap_of(strand: u32, vertex: u32) -> f32 {
    var moved = vec3<f32>(0.0);
    for (var hinge = 0u; hinge < vertex; hinge++) {
        moved += cross(group_turns[strand][hinge], group_chain[strand][vertex] - group_chain[strand][hinge]);
    }
    let normal = group_normals[strand][vertex];
    let under = (group_heights[strand][vertex] - group_chain[strand][vertex].z) * normal.z;
    return under - dot(normal, moved);
}

/// `vertex`'s own contact compliance (`contact_problem`'s diagonal).
fn diagonal_of(strand: u32, vertex: u32, compliances: HingeCompliances) -> f32 {
    var own = 0.0;
    for (var hinge = 0u; hinge < vertex; hinge++) {
        let lever = cross(group_chain[strand][vertex] - group_chain[strand][hinge], group_normals[strand][vertex]);
        own += compliances[hinge] * dot(lever, lever);
    }
    return own * METRES_PER_PX;
}

/// `contact_compliance` over the strand's workgroup copy.
fn group_compliance(strand: u32, compliances: HingeCompliances, vertex: u32, pushed: u32) -> f32 {
    var rise = 0.0;
    for (var hinge = 0u; hinge < min(vertex, pushed); hinge++) {
        let lever = group_chain[strand][vertex] - group_chain[strand][hinge];
        let arm_m = (group_chain[strand][pushed] - group_chain[strand][hinge]) * METRES_PER_PX;
        rise += compliances[hinge] * dot(cross(arm_m, group_normals[strand][pushed]), cross(lever, group_normals[strand][vertex]));
    }
    return rise;
}

/// `CONTACT_RIDGE_SHARE` of `ridge_scale`.
fn group_ridge_of(strand: u32, compliances: HingeCompliances) -> f32 {
    let segment_px = distance(group_chain[strand][1], group_chain[strand][0]);
    var largest = compliances[BRISTLE_SEGMENTS - 1u] * segment_px * segment_px * METRES_PER_PX;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        largest = max(largest, group_diagonal[strand][row]);
    }
    return CONTACT_RIDGE_SHARE * largest;
}

/// Whether the free step leaves a vertex under its floor (`any_under`), and if so the ridge
/// and the first active set of `contact_forces`.
fn open_contacts(strand: u32, compliances: HingeCompliances) {
    var under = false;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        under = under || group_gaps[strand][row] > 0.0;
        group_forces[strand][row] = 0.0;
    }
    group_solving[strand] = select(0u, 1u, under);
    group_settled[strand] = 0u;
    group_dropped[strand] = 0u;
    if !under {
        return;
    }
    let ridge = group_ridge_of(strand, compliances);
    group_ridge[strand] = ridge;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        let engaged = group_gaps[strand][row] > 0.0 && group_diagonal[strand][row] > ridge;
        group_engaged[strand][row] = select(0u, 1u, engaged);
    }
}

/// `active_forces` over the strand's workgroup copy.
fn group_active_forces(strand: u32) -> PerContact {
    var engaged: Engaged;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        engaged[row] = group_engaged[strand][row] != 0u;
    }
    var rows: ActiveRows;
    let count = active_rows(engaged, &rows);
    let ridge = group_ridge[strand];
    var forces: PerContact;
    if count == 1u {
        let row = rows[0];
        forces[row] = group_gaps[strand][row] / (group_diagonal[strand][row] + ridge);
        return forces;
    }
    var lower: ContactMatrix;
    var rhs: PerContact;
    for (var i = 0u; i < count; i++) {
        rhs[i] = group_gaps[strand][rows[i]];
        for (var j = 0u; j < i; j++) {
            lower[i][j] = group_matrix[strand][rows[i]][rows[j]];
        }
        lower[i][i] = group_diagonal[strand][rows[i]] + ridge;
    }
    let solved = solved_by_cholesky(lower, rhs, count);
    for (var i = 0u; i < count; i++) {
        forces[rows[i]] = solved[i];
    }
    return forces;
}

/// One round's forces, then the contacts that would pull the strand down dropped
/// (`released`).
fn solve_round(strand: u32) {
    let forces = group_active_forces(strand);
    var dropped = 0u;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        group_forces[strand][row] = forces[row];
        if group_engaged[strand][row] != 0u && forces[row] < 0.0 {
            group_engaged[strand][row] = 0u;
            dropped = 1u;
        }
    }
    group_dropped[strand] = dropped;
}

/// The turn the round's forces give `hinge` (`rises`).
fn rise_turn(strand: u32, hinge: u32, compliances: HingeCompliances) -> vec3<f32> {
    var turn = vec3<f32>(0.0);
    for (var pushed = hinge + 1u; pushed < BRISTLE_VERTICES; pushed++) {
        let force = group_forces[strand][pushed - 1u];
        if force != 0.0 {
            let arm_m = (group_chain[strand][pushed] - group_chain[strand][hinge]) * METRES_PER_PX;
            turn += compliances[hinge] * force * cross(arm_m, group_normals[strand][pushed]);
        }
    }
    return turn;
}

/// Engages `row` if the round's forces leave its vertex under its floor (`landed`).
fn land_row(strand: u32, row: u32) {
    let vertex = row + 1u;
    var moved = vec3<f32>(0.0);
    for (var hinge = 0u; hinge < vertex; hinge++) {
        moved += cross(group_rise_turns[strand][hinge], group_chain[strand][vertex] - group_chain[strand][hinge]);
    }
    let lifted = dot(group_normals[strand][vertex], moved);
    let under = group_gaps[strand][row] - lifted > CONTACT_TOLERANCE_PX;
    let added = group_engaged[strand][row] == 0u && under && group_diagonal[strand][row] > group_ridge[strand];
    if added {
        group_engaged[strand][row] = 1u;
    }
    group_added[strand][row] = select(0u, 1u, added);
}

fn any_added(strand: u32) -> bool {
    var added = false;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        added = added || group_added[strand][row] != 0u;
    }
    return added;
}

/// The live strands of the workgroup still solving their contacts.
fn unsettled_strands(group: u32) -> u32 {
    var count = 0u;
    for (var strand = 0u; strand < BRISTLE_STRANDS_PER_GROUP; strand++) {
        let live = group * BRISTLE_STRANDS_PER_GROUP + strand < dab.bristle_total;
        if live && group_solving[strand] != 0u && group_settled[strand] == 0u {
            count++;
        }
    }
    return count;
}

/// The last forces of `contact_forces`, kept only where engaged and never pulling.
fn close_contacts(strand: u32) {
    if group_settled[strand] == 0u {
        let forces = group_active_forces(strand);
        for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
            group_forces[strand][row] = forces[row];
        }
    }
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        let engaged = group_engaged[strand][row] != 0u;
        group_forces[strand][row] = select(0.0, max(group_forces[strand][row], 0.0), engaged);
    }
}

/// The active-set loop of `contact_forces`, round by round across the workgroup, until
/// every strand has settled or `CONTACT_ROUNDS` ran.
fn settle_contacts(lane: Lane, compliances: HingeCompliances) {
    let s = lane.strand;
    for (var round = 0u; round < CONTACT_ROUNDS; round++) {
        let solving = lane.live && group_solving[s] != 0u && group_settled[s] == 0u;
        if solving && lane.vertex == 0u {
            solve_round(s);
        }
        workgroupBarrier();
        let landing = solving && group_dropped[s] == 0u && lane.vertex < BRISTLE_SEGMENTS;
        if landing {
            group_rise_turns[s][lane.vertex] = rise_turn(s, lane.vertex, compliances);
        }
        workgroupBarrier();
        if landing {
            land_row(s, lane.vertex);
        }
        workgroupBarrier();
        if solving && lane.vertex == 0u {
            group_settled[s] = select(0u, 1u, group_dropped[s] == 0u && !any_added(s));
        }
        workgroupBarrier();
        if lane.strand == 0u && lane.vertex == 0u {
            group_unsettled = unsettled_strands(lane.group);
        }
        if workgroupUniformLoad(&group_unsettled) == 0u {
            break;
        }
    }
    if leads(lane) && group_solving[s] != 0u {
        close_contacts(s);
    }
    workgroupBarrier();
}

/// Hinge `hinge`'s free turn plus what the canvas's forces turn it (`solve_iteration`).
fn pushed_turn(strand: u32, hinge: u32, compliances: HingeCompliances, free: vec3<f32>) -> vec3<f32> {
    var turn = free;
    for (var pushed = hinge + 1u; pushed < BRISTLE_VERTICES; pushed++) {
        let force = group_forces[strand][pushed - 1u];
        let arm_m = (group_chain[strand][pushed] - group_chain[strand][hinge]) * METRES_PER_PX;
        turn += compliances[hinge] * force * cross(arm_m, group_normals[strand][pushed]);
    }
    return turn;
}

/// The uniform scale of `capped`.
fn cap_scale(strand: u32) -> f32 {
    var largest = 0.0;
    for (var hinge = 0u; hinge < BRISTLE_SEGMENTS; hinge++) {
        largest = max(largest, length(group_turns[strand][hinge]));
    }
    return MAX_HINGE_TURN_RAD / max(largest, MAX_HINGE_TURN_RAD);
}

/// Where `turn_chain` puts `vertex`: the quaternion prefix down to it, rebuilt by this
/// lane alone in the serial order.
fn turned_position(strand: u32, vertex: u32, scale: f32, segment_px: f32) -> vec3<f32> {
    var at = group_chain[strand][0];
    var total = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    for (var edge = 0u; edge < vertex; edge++) {
        total = normalize(composed(total, quaternion_of(group_turns[strand][edge] * scale)));
        let direction = normalize(rotated(total, group_edges[strand][edge]));
        at = at + direction * segment_px;
    }
    return at;
}

/// `summed`.
fn summed_group_forces(strand: u32) -> f32 {
    var total = 0.0;
    for (var row = 0u; row < BRISTLE_SEGMENTS; row++) {
        total += group_forces[strand][row];
    }
    return total;
}

/// One `solve_iteration` of every strand in the workgroup, once `place_vertex` has
/// published the carried chain and its floors.
fn solve_step(lane: Lane, shape: StrandShape) -> Stepped {
    let s = lane.strand;
    if owns_hinge(lane) {
        place_edge(lane);
    }
    workgroupBarrier();
    if leads(lane) {
        mend_short_edges(s, shape.clamp);
    }
    workgroupBarrier();
    var free = vec3<f32>(0.0);
    if owns_hinge(lane) {
        free = free_turn(lane, shape);
        group_turns[s][lane.vertex] = free;
    }
    workgroupBarrier();
    if owns_hinge(lane) {
        let vertex = lane.vertex + 1u;
        group_gaps[s][lane.vertex] = gap_of(s, vertex);
        group_diagonal[s][lane.vertex] = diagonal_of(s, vertex, shape.compliances);
    }
    workgroupBarrier();
    if leads(lane) {
        open_contacts(s, shape.compliances);
    }
    workgroupBarrier();
    if owns_hinge(lane) && group_solving[s] != 0u {
        for (var column = 0u; column < lane.vertex; column++) {
            group_matrix[s][lane.vertex][column] =
                group_compliance(s, shape.compliances, lane.vertex + 1u, column + 1u);
        }
    }
    workgroupBarrier();
    settle_contacts(lane, shape.compliances);
    if owns_hinge(lane) {
        group_turns[s][lane.vertex] = pushed_turn(s, lane.vertex, shape.compliances, free);
    }
    workgroupBarrier();
    if leads(lane) {
        group_scale[s] = cap_scale(s);
    }
    workgroupBarrier();
    var stepped = Stepped(vec3<f32>(0.0), 0.0);
    if lane.live {
        stepped.at = turned_position(s, lane.vertex, group_scale[s], shape.segment_px);
    }
    if leads(lane) {
        stepped.force = summed_group_forces(s);
    }
    workgroupBarrier();
    return stepped;
}

/// Solves BRISTLE_STRANDS_PER_GROUP strands along the path `touch` recorded, one lane per
/// vertex: the same strands as `solve_reference`.
@compute @workgroup_size(BRISTLE_SOLVE_WORKGROUP_SIZE)
fn solve(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(local_invocation_index) local: u32,
) {
    if group.x * BRISTLE_STRANDS_PER_GROUP >= dab.bristle_total {
        return;
    }
    let lane = lane_of(group.x, local);
    let start = pose_of(handle.start);
    let end = pose_of(handle.end);
    var at = vec3<f32>(0.0);
    var radius_px = 0.0;
    if lane.live {
        at = held_position(lane, start);
        radius_px = rests[lane.bristle].radius_px;
    }
    var sampled = at;
    let substeps = max(dab.substeps, 1u);
    let iterations = max(substeps, MIN_DAB_ITERATIONS);
    var previous = start;
    var force = 0.0;
    for (var iteration = 0u; iteration < iterations; iteration++) {
        let pose = pose_between(start, end, min(f32(iteration + 1u) / f32(substeps), 1.0));
        var shape: StrandShape;
        if lane.live {
            shape = strand_shape(rests[lane.bristle], pose);
            at = carried(at, previous, pose);
            place_vertex(lane, at, sampled, radius_px);
            sampled = at;
        }
        previous = pose;
        workgroupBarrier();
        let stepped = solve_step(lane, shape);
        at = stepped.at;
        force = stepped.force;
    }
    if lane.live {
        states[position_slot(lane.bristle, lane.vertex)] = vec4<f32>(at, disc_floor_px(at.xy, radius_px));
        if lane.vertex == 0u {
            normal_forces[lane.bristle] = force;
            hair_loads[lane.bristle] = FLAT_HAIR_LOAD;
        }
    }
}
```

How this maps onto `solved_dab` in `bristle_step.wgsl`:
- `sampled` is the carried, pre-solve vertex of the previous iteration (the hold before the first), as `floors_under(sampled, chain, …)` and then `sampled = chain` give.
- `previous` advances to `pose` as `carry` does.
- The final store matches `store_chain`.

If naga rejects a construct, fix it without changing the arithmetic. Two likely cases:
- dynamic indexing of a `let` array: copy it to a `var` first;
- a barrier it judges non-uniform: hoist the condition out of the loop.

- [ ] **Step 6: Run the unit tests and the equivalence test**

Run: `cargo test --release -p cadmium-engine --lib bristle_step && cargo test --release -p cadmium-engine --test bristles parallel_solve`
Expected: PASS.

If the equivalence test fails, the assertion message names the bristle and vertex. Find the phase whose arithmetic differs from the serial function it mirrors; the doc comment of each helper names that function. Do **not** loosen the tolerances to make it pass.

- [ ] **Step 7: Run the whole engine suite**

Run: `cargo test --release -p cadmium-engine`
Expected: PASS, except tests that already failed on the research HEAD. Before this task, check which tests fail on the HEAD (`git stash`, run, `git stash pop`), and report any new failure.

The comment on #62 records `relief::wet_paint_holds_the_hairs_up_only_by_its_support_share` as flaky: it swings by up to 2.7 px on last-bit float changes. If it fails here too, record that rather than "fixing" it.

- [ ] **Step 8: Commit**

```bash
cargo fmt --all
cargo clippy --release -p cadmium-engine --all-targets -- -D warnings
git add crates/cadmium-engine/src/shaders/bristle_strand_groups.wgsl crates/cadmium-engine/src/shaders/bristle_step.wgsl crates/cadmium-engine/src/engine/bristles/state.rs crates/cadmium-engine/src/engine/bristles.rs crates/cadmium-engine/src/engine/shader.rs crates/cadmium-engine/src/engine/pass/bristle_step.rs crates/cadmium-engine/src/engine/pass/bristle_step/tests.rs
git commit -m "perf(engine): solve each strand over a group of lanes, one per vertex"
```

---

### Task 6: Measure step 1 and record it

**Files:**
- Modify: `docs/superpowers/specs/2026-11-19-strand-parallel-solve-design.md`

- [ ] **Step 1: Full gate run**

Run: `CADMIUM_BENCH_BASELINE=docs/superpowers/baselines/2026-11-16-bristle-cost.tsv cargo run --release -p cadmium-engine --example bristle_cost 2>&1 | tee target/bench-step1.txt`
Expected: it finishes with a verdict. Whether it passes or fails the gate, the result is data.

- [ ] **Step 2: Stage run**

Run: `CADMIUM_BENCH_STAGES=1 CADMIUM_BENCH_REPEATS=3 cargo run --release -p cadmium-engine --example bristle_cost 2>&1 | tee target/stages-step1.txt`

Take the median Bristles µs/dab of the three passes for `brush, default size`.

- [ ] **Step 3: Fill the results table**

In the spec's `## Results` table:
- fill the research HEAD row's Bristles µs/dab from Task 2, Step 5;
- add a row for step 1, with gate mean / p95 for the three scenarios from `target/bench-step1.txt`, the Bristles µs/dab, and `kept` = `yes` if the step-1 numbers are lower than the research HEAD's, else `no`.

If step 1 did not measure faster, revert the Task 5 commit (`git revert <sha>`) and keep the row with `kept` = `no`.

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/specs/2026-11-19-strand-parallel-solve-design.md
git commit -m "docs: step 1 of the strand-parallel solve, measured"
```

The commit body lists the six over-budget scenarios from `target/bench-step1.txt`, as `before -> after` mean / p95, plus the Bristles µs/dab before → after.

- [ ] **Step 5: Push and post on #62**

```bash
git push
gh issue comment 62 --body-file target/step1-comment.md
```

Write `target/step1-comment.md` in English, with no attribution lines. Include:
- the step-1 row and the research HEAD row of the results table;
- the Bristles µs/dab before → after;
- whether the gate passes;
- one sentence on what dominates the Bristles stage now. Read it from the stage run: if Bristles fell by less than ~4×, the floor loads still dominate and step 2, the texel cache, is next.
