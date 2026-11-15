use super::*;

#[test]
fn a_dragged_colour_survives_the_trip_as_text() {
    for colour in [
        ColourRef::Paint(0),
        ColourRef::Paint(7),
        ColourRef::Mine(12),
    ] {
        assert_eq!(parse_colour_ref(&colour_ref_text(colour)), Some(colour));
    }
    for junk in ["", "paint", "paint:", "mine:-1", "tube:2", "paint:2:3"] {
        assert_eq!(parse_colour_ref(junk), None, "{junk:?}");
    }
}
