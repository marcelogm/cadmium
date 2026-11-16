# Baselines for the XPBD bristle rewrite (#62)

Every phase of #62 is measured against the engine as it was before the rewrite. The baseline engine is the tag `xpbd-baseline`: the last commit of phase 0, which still has the old bristle step.

## GPU cost

`2026-11-16-bristle-cost.tsv` holds `bristle_cost` results recorded on the baseline as the median of five interleaved runs after a warm-up pass; its first lines name the adapter and the repeat count. A phase closes only if a full run (no `CADMIUM_SCENARIO` filter) passes on the same adapter; compare full runs only, since a filtered run warms the GPU less:

```bash
CADMIUM_BENCH_BASELINE=docs/superpowers/baselines/2026-11-16-bristle-cost.tsv \
  cargo run --release -p cadmium-engine --example bristle_cost
```

The check takes its repeat count from the baseline file (unless `CADMIUM_BENCH_REPEATS` is set) and exits with status 1, listing every scenario whose mean or p95 GPU time per frame is more than 10% (and more than 0.05 ms) over the baseline. On a different adapter, record a new baseline from the tag first, with `CADMIUM_BENCH_OUT` and `CADMIUM_BENCH_REPEATS=5`.

## Example sheets

The sheets are not committed. Rendering is deterministic, so they are regenerated from the tag:

```bash
git worktree add ../cadmium-baseline xpbd-baseline
(cd ../cadmium-baseline && cargo run --release -p cadmium-studio --example brush_sheets -- render "$PWD/target/brush_sheets/baseline")
cargo run --release -p cadmium-studio --example brush_sheets -- render target/brush_sheets/current
cargo run --release -p cadmium-studio --example brush_sheets -- compare \
  ../cadmium-baseline/target/brush_sheets/baseline target/brush_sheets/current target/brush_sheets/page
```

Open `target/brush_sheets/page/index.html` to see each factory brush before and after. (cargo runs inside the cadmium-dev Docker image, as everywhere in this repo.)

## Invariant tests

`crates/cadmium-engine/tests/bristles/invariants.rs` holds the behaviours of the spec's section 5. A test the baseline fails is `#[ignore]`d and names the phase that must make it pass.
