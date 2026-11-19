# Baselines for the XPBD bristle rewrite (#62)

Every phase of #62 is measured against the engine as it was before the rewrite. The baseline engine is the tag `xpbd-baseline`: the last commit of phase 0, which still has the old bristle step.

## GPU cost

`2026-11-16-bristle-cost.tsv` holds `bristle_cost` results recorded on the baseline as the median of five interleaved runs after a warm-up pass; its first lines name the adapter and the repeat count. A phase closes only if a full run (no `CADMIUM_SCENARIO` filter) passes on the same adapter; compare full runs only, since a filtered run warms the GPU less:

```bash
CADMIUM_BENCH_BASELINE=docs/superpowers/baselines/2026-11-16-bristle-cost.tsv \
  cargo run --release -p cadmium-engine --example bristle_cost
```

The check takes its repeat count from the baseline file (unless `CADMIUM_BENCH_REPEATS` is set) and exits with status 1, listing every scenario whose mean or p95 GPU time per frame is more than 10% (and more than 0.05 ms) over the baseline. It warns when the adapter, the repeat count or a scenario's frame count differs from the baseline's. Three checks of the unchanged baseline code stayed within 7.7% of it (worst scenario: `brush, max size`), so on that scenario the gate leaves about 2% for a real slowdown. On a different adapter, record a new baseline from the tag first, with `CADMIUM_BENCH_OUT` and `CADMIUM_BENCH_REPEATS=5`.

## Example sheets

The sheets are not committed. Rendering is deterministic on a given adapter and driver, so they are regenerated from the tag. The baseline worktree lives under `target/`, inside the repo, so the Docker image's mount of the repo reaches it; run each `cargo` line in the cadmium-dev image from the directory it names:

```bash
git worktree add target/xpbd-baseline xpbd-baseline
(cd target/xpbd-baseline && cargo run --release -p cadmium-studio --example brush_sheets -- render target/brush_sheets/baseline)
cargo run --release -p cadmium-studio --example brush_sheets -- render target/brush_sheets/current
cargo run --release -p cadmium-studio --example brush_sheets -- compare \
  target/xpbd-baseline/target/brush_sheets/baseline target/brush_sheets/current target/brush_sheets/page
```

Open `target/brush_sheets/page/index.html` to see each factory brush before and after.

## Invariant tests

`crates/cadmium-engine/tests/bristles/invariants.rs` holds the invariants of the spec's section 5 that the baseline can run: splay vs pressure, mark width vs pen sampling rate, mark width vs hair density, and the light first touch. The table in section 5 says which phase brings in each of the others. A test the baseline fails is `#[ignore]`d and names the phase that must make it pass.
