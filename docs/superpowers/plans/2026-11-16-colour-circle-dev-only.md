# Colour Circle Dev-Only Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show the colour circle only in dev builds; release builds offer only the colour grid and mixing.

**Architecture:** One `COLOUR_CIRCLE` const (`cfg!(any(debug_assertions, feature = "smoke"))`). Pure functions in each `presentation.rs` take the flag as a bool and pick the views/modes on offer; the GTK builders call them with the const. Spec: `docs/superpowers/specs/2026-11-16-colour-circle-dev-only-design.md`.

**Tech Stack:** Rust 2024, gtk4-rs, libadwaita.

**Running cargo:** always inside the `cadmium-dev` Docker image (the host has no GTK dev libs). From the repo root:

```bash
D="docker run --rm --user $(id -u):$(id -g) --device /dev/dri $(stat -c '--group-add %g' /dev/dri/* | sort -u) --security-opt label=disable -v $PWD:$PWD -w $PWD -v $HOME/.cargo:$HOME/.cargo -e CARGO_HOME=$HOME/.cargo -e HOME=/tmp cadmium-dev"
```

Then `$D xvfb-run -a cargo test ...`. Docker needs the command sandbox disabled.

---

### Task 1: The flag

**Files:**
- Modify: `crates/cadmium-app/src/colour_wheel.rs` (above `struct Wheel`, ~line 82)

- [ ] **Step 1: Add the const**

```rust
/// Whether the app offers the colour circle. It is still buggy, so only dev builds
/// (and the smoke tests) show it; release builds offer the colour grid and mixing alone.
pub(crate) const COLOUR_CIRCLE: bool = cfg!(any(debug_assertions, feature = "smoke"));
```

No commit yet: it is unused until Task 2 (dead-code warning).

### Task 2: Brush panel

**Files:**
- Modify: `crates/cadmium-app/src/brush_panel/presentation.rs` (after `palette_view_named`, ~line 51)
- Modify: `crates/cadmium-app/src/brush_panel/build.rs:4-10, 69-87`
- Modify: `crates/cadmium-app/src/brush_panel/snapshot.rs:7-10, 42`
- Test: `crates/cadmium-app/src/brush_panel/presentation/tests.rs`

- [ ] **Step 1: Write the failing test** (append to `brush_panel/presentation/tests.rs`)

```rust
#[test]
fn without_the_circle_the_palette_offers_and_shows_only_its_colours() {
    assert_eq!(palette_views(true), PALETTE_VIEWS.as_slice());
    assert_eq!(palette_views(false), [(PaletteView::Colours, "Colours")]);
    assert_eq!(shown_view(PaletteView::Circle, true), PaletteView::Circle);
    assert_eq!(shown_view(PaletteView::Circle, false), PaletteView::Colours);
    assert_eq!(shown_view(PaletteView::Colours, false), PaletteView::Colours);
}
```

- [ ] **Step 2: Run it, expect a compile failure** (`palette_views`, `shown_view` not found)

Run: `$D xvfb-run -a cargo test -p cadmium-app without_the_circle`

- [ ] **Step 3: Implement** (in `brush_panel/presentation.rs`, after `palette_view_named`)

```rust
/// The faces the palette offers: without the circle, only its colours.
pub(super) fn palette_views(circle: bool) -> &'static [(PaletteView, &'static str)] {
    if circle { &PALETTE_VIEWS } else { &PALETTE_VIEWS[..1] }
}

/// The face on show: `view`, unless it is the circle and the app has none.
pub(super) fn shown_view(view: PaletteView, circle: bool) -> PaletteView {
    if circle { view } else { PaletteView::Colours }
}
```

- [ ] **Step 4: Use them in `build.rs`**

Import: replace `PALETTE_VIEWS` with `palette_views` in the `super::presentation` use list, and change `use crate::colour_wheel::ColourWheel;` to `use crate::colour_wheel::{COLOUR_CIRCLE, ColourWheel};`.

In `palette_group`, replace the loop

```rust
    for ((view, label), page) in PALETTE_VIEWS.into_iter().zip([colours, circle]) {
        let name = palette_view_name(view);
        view_toggles.add(adw::Toggle::builder().name(name).label(label).build());
        stack.add_named(&page, Some(name));
    }
```

with

```rust
    for ((view, label), page) in palette_views(COLOUR_CIRCLE).iter().zip([colours, circle]) {
        let name = palette_view_name(*view);
        view_toggles.add(adw::Toggle::builder().name(name).label(*label).build());
        stack.add_named(&page, Some(name));
    }
    // A single face needs no switch.
    view_toggles.set_visible(COLOUR_CIRCLE);
```

- [ ] **Step 5: Use `shown_view` in `snapshot.rs`**

Change `use super::presentation::{PLATE_GAP_KEY, RackEntry, rack_entry};` to `use super::presentation::{PLATE_GAP_KEY, RackEntry, rack_entry, shown_view};`, `use crate::colour_wheel::WheelKey;` to `use crate::colour_wheel::{COLOUR_CIRCLE, WheelKey};`, and in `PaletteSnapshot::of`:

```rust
            view: shown_view(state.palette_view, COLOUR_CIRCLE),
```

(This also keeps `circle_shows` false in release, so the wheel is never mixed.)

- [ ] **Step 6: Run the brush panel tests**

Run: `$D xvfb-run -a cargo test -p cadmium-app brush_panel`
Expected: all PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/cadmium-app/src/colour_wheel.rs crates/cadmium-app/src/brush_panel
git commit -m "feat: show the brush panel's colour circle only in dev builds"
```

### Task 3: Palette dialog

**Files:**
- Modify: `crates/cadmium-app/src/palette_editor/presentation.rs` (after `mode_named`, ~line 68)
- Modify: `crates/cadmium-app/src/palette_editor/header.rs:6, 19-26`
- Modify: `crates/cadmium-app/src/palette_editor/layout.rs:8-10, 23`
- Modify: `crates/cadmium-app/src/palette_editor.rs:13, 57-66`
- Test: `crates/cadmium-app/src/palette_editor/presentation/tests.rs`

- [ ] **Step 1: Write the failing test** (append to `palette_editor/presentation/tests.rs`)

```rust
#[test]
fn without_the_circle_the_dialog_only_mixes() {
    assert_eq!(modes(true), MODES.as_slice());
    assert_eq!(modes(false), [(PaletteMode::Mixing, "Mixing")]);
    assert_eq!(opening_mode(PaletteMode::Circle, true), PaletteMode::Circle);
    assert_eq!(opening_mode(PaletteMode::Circle, false), PaletteMode::Mixing);
    assert_eq!(opening_mode(PaletteMode::Mixing, false), PaletteMode::Mixing);
}
```

- [ ] **Step 2: Run it, expect a compile failure** (`modes`, `opening_mode` not found)

Run: `$D xvfb-run -a cargo test -p cadmium-app without_the_circle_the_dialog`

- [ ] **Step 3: Implement** (in `palette_editor/presentation.rs`, after `mode_named`)

```rust
/// The modes the dialog offers: without the circle, only mixing.
pub(super) fn modes(circle: bool) -> &'static [(PaletteMode, &'static str)] {
    if circle { &MODES } else { &MODES[1..] }
}

/// The mode the dialog opens in: `asked`, or mixing when the app has no circle.
pub(super) fn opening_mode(asked: PaletteMode, circle: bool) -> PaletteMode {
    if circle { asked } else { PaletteMode::Mixing }
}
```

- [ ] **Step 4: Header** (`palette_editor/header.rs`)

Change `use super::presentation::{MODES, mode_name};` to `use super::presentation::{mode_name, modes};`, add `use crate::colour_wheel::COLOUR_CIRCLE;`, and replace the group and its loop in `header()`, renaming the local to `toggles` so it does not shadow `modes`:

```rust
    let toggles = adw::ToggleGroup::new();
    for (mode, label) in modes(COLOUR_CIRCLE) {
        toggles.add(
            adw::Toggle::builder()
                .name(mode_name(*mode))
                .label(*label)
                .build(),
        );
    }
    // A single mode needs no switch.
    toggles.set_visible(COLOUR_CIRCLE);
```

and further down use `toggles`: `.title_widget(&toggles)` and `Header { bar, modes: toggles }`.

- [ ] **Step 5: Layout** (`palette_editor/layout.rs`)

Add `use crate::colour_wheel::COLOUR_CIRCLE;` and guard the circle page:

```rust
    if COLOUR_CIRCLE {
        stack.add_named(circle, Some(mode_name(PaletteMode::Circle)));
    }
    stack.add_named(mixing, Some(mode_name(PaletteMode::Mixing)));
```

- [ ] **Step 6: Opening mode** (`palette_editor.rs`)

Add `use crate::colour_wheel::COLOUR_CIRCLE;` and `use presentation::opening_mode;` (next to the other `use` of submodules), and in `open_in` change `state.set_palette_mode(mode);` to:

```rust
        state.set_palette_mode(opening_mode(mode, COLOUR_CIRCLE));
```

Update the doc comment of `open`: `/// Shows the dialog arranging the circle (mixing when the app has none); while an editor is open, does nothing.`

- [ ] **Step 7: Run the palette editor tests**

Run: `$D xvfb-run -a cargo test -p cadmium-app palette_editor`
Expected: all PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/cadmium-app/src/palette_editor crates/cadmium-app/src/palette_editor.rs
git commit -m "feat: open the palette dialog in mixing when the circle is off"
```

### Task 4: Verify both profiles

- [ ] **Step 1: Release build, no smoke (the shipped path)**

Run: `$D cargo clippy --release -p cadmium-app -- -D warnings`
Expected: no warnings (no dead code; the circle code is still referenced through `cfg!`).

- [ ] **Step 2: Dev clippy and the CI test set**

Run: `$D cargo clippy --workspace --all-targets --features cadmium-app/smoke -- -D warnings`
Run: `$D xvfb-run -a cargo test --release --workspace --features cadmium-app/smoke`
Expected: all PASS.

- [ ] **Step 3: Format**

Run: `$D cargo fmt --all -- --check`
Expected: no diff.

- [ ] **Step 4: Manual check (optional)** — `cargo run --release -p cadmium-app` shows no Colours|Circle switch in the panel and the Palette dialog opens straight in mixing with no mode switch; `cargo run` shows both as before.
