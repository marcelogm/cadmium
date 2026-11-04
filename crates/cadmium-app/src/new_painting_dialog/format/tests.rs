use super::*;
use crate::presets::starting_canvas_spec;

#[test]
fn landscape_format_keeps_the_long_side_of_a_portrait_size() {
    assert_eq!(
        PaintingFormat::Landscape4x3.size_keeping_long_side(2304, 3072),
        Some((3072, 2304))
    );
}

#[test]
fn portrait_format_puts_the_long_side_vertically() {
    assert_eq!(
        PaintingFormat::Portrait3x4.size_keeping_long_side(2048, 1536),
        Some((1536, 2048))
    );
}

#[test]
fn square_and_widescreen_formats_derive_the_short_side() {
    assert_eq!(
        PaintingFormat::Square.size_keeping_long_side(2048, 1536),
        Some((2048, 2048))
    );
    assert_eq!(
        PaintingFormat::Landscape16x9.size_keeping_long_side(1536, 2048),
        Some((2048, 1152))
    );
}

#[test]
fn custom_format_leaves_the_size_alone() {
    assert_eq!(
        PaintingFormat::Custom.size_keeping_long_side(1000, 700),
        None
    );
}

#[test]
fn short_side_never_drops_below_the_minimum() {
    assert_eq!(
        PaintingFormat::Landscape16x9.size_keeping_long_side(64, 64),
        Some((64, 64))
    );
}

#[test]
fn size_with_a_known_aspect_is_recognised_as_that_format() {
    assert_eq!(
        PaintingFormat::matching(2048, 1536),
        PaintingFormat::Landscape4x3
    );
    assert_eq!(
        PaintingFormat::matching(1536, 2048),
        PaintingFormat::Portrait3x4
    );
    assert_eq!(PaintingFormat::matching(1024, 1024), PaintingFormat::Square);
    assert_eq!(PaintingFormat::matching(1000, 700), PaintingFormat::Custom);
}

#[test]
fn starting_painting_size_is_recognised_as_landscape() {
    let spec = starting_canvas_spec();
    assert_eq!(
        PaintingFormat::matching(spec.width, spec.height),
        PaintingFormat::Landscape4x3
    );
}
