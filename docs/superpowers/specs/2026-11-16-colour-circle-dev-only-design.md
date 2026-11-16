# Colour circle only in dev builds

The colour circle is buggy. Until it is fixed, it ships only in dev (debug) builds; release
builds (Flatpak, AppImage, Windows) hide it. Mixing on the palette sheet stays as is.

## Flag

`pub(crate) const COLOUR_CIRCLE: bool = cfg!(any(debug_assertions, feature = "smoke"));` in
`colour_wheel.rs`. The `smoke` feature counts as dev, as elsewhere in the app: CI runs the smoke
tests in release, and shipped builds never enable it.

`cfg!` rather than `#[cfg]`: the circle code still compiles and is tested in both profiles,
with no dead-code warnings. Only the places that show the circle read the flag.

## Brush panel

- Release: the `Circle` toggle and page are not added; the `ToggleGroup` is hidden, since a
  single option is meaningless. Only the colour grid shows.
- The shown view is always `PaletteView::Colours` in release, whatever the state holds.
- Dev: unchanged.

## Palette dialog

- Release: opens in `PaletteMode::Mixing`; the Circle page and the mode switcher in the
  header are not shown.
- Dev: unchanged (opens in Circle, both modes available).

## Selection

Small pure functions decide what is available, taking the flag as an argument:

- `palette_views(circle: bool) -> &[(PaletteView, &str)]`
- `palette_modes(circle: bool) -> &[(PaletteMode, &str)]`
- the dialog's opening mode: `Circle` when the circle is on, `Mixing` otherwise.

Builders call them with `COLOUR_CIRCLE`.

## Testing

Existing tests run in debug and stay valid. New unit tests cover the selection functions with
both flag values: no Circle entry and Mixing as opening mode when off; today's lists when on.
A `cargo build --release` confirms the release path compiles.
