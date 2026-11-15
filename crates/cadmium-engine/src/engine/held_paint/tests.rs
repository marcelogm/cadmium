use super::*;
use crate::engine::TIP_BYTES;

const SMALL_RADIUS_PX: f32 = 20.0;
const WIDEST_UNCLIPPED_RADIUS_PX: f32 = 243.0;
const FIRST_CLIPPED_RADIUS_PX: f32 = 244.0;
const WIDEST_RADIUS_PX: f32 = 250.0;

fn whole_tip() -> TipSquare {
    TipSquare {
        first_texel: 0,
        side_texels: TIP_SIZE,
    }
}

#[test]
fn a_small_tip_is_captured_as_the_square_around_the_centre_it_reaches() {
    assert_eq!(
        TipSquare::around(SMALL_RADIUS_PX),
        TipSquare {
            first_texel: TIP_HALF - 32,
            side_texels: 65,
        }
    );
}

#[test]
fn a_fractional_radius_reaches_as_far_as_the_next_whole_one() {
    assert_eq!(
        TipSquare::around(SMALL_RADIUS_PX - 0.5),
        TipSquare::around(SMALL_RADIUS_PX)
    );
}

#[test]
fn the_widest_tip_that_fits_stops_one_texel_short_of_the_texture_edge() {
    assert_eq!(
        TipSquare::around(WIDEST_UNCLIPPED_RADIUS_PX),
        TipSquare {
            first_texel: 1,
            side_texels: TIP_SIZE - 1,
        }
    );
}

#[test]
fn a_tip_reaching_past_the_texture_is_captured_as_the_whole_texture() {
    assert_eq!(TipSquare::around(FIRST_CLIPPED_RADIUS_PX), whole_tip());
    assert_eq!(TipSquare::around(WIDEST_RADIUS_PX), whole_tip());
}

#[test]
fn the_rows_of_the_widest_tip_fill_the_tip_buffer_and_the_capture_end_to_end() {
    let square = TipSquare::around(WIDEST_RADIUS_PX);
    let rows: Vec<(u64, u64)> = square.rows().collect();
    assert_eq!(rows.first(), Some(&(0, 0)));
    let (last_in_tip, last_in_capture) = *rows.last().unwrap();
    assert_eq!(last_in_tip + square.row_bytes(), TIP_BYTES);
    assert_eq!(last_in_capture + square.row_bytes(), square.bytes());
    assert_eq!(Layout::for_tip(WIDEST_RADIUS_PX).body_offset, TIP_BYTES);
}

#[test]
fn the_rows_of_a_small_tip_start_at_its_first_texel_of_each_row_it_covers() {
    let square = TipSquare::around(SMALL_RADIUS_PX);
    let rows: Vec<(u64, u64)> = square.rows().collect();
    let first_texel = u64::from(square.first_texel);
    let first_in_tip = (first_texel * u64::from(TIP_SIZE) + first_texel) * TIP_TEXEL_BYTES;
    assert_eq!(rows.len(), 65);
    assert_eq!(rows[0], (first_in_tip, 0));
    assert_eq!(
        rows[1],
        (
            first_in_tip + u64::from(TIP_SIZE) * TIP_TEXEL_BYTES,
            square.row_bytes()
        )
    );
}
