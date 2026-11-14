use super::{covering, moved_to};
use crate::canvas::{TileCoord, tiles_in_rect};
use crate::stroke::Dab;

const CANVAS_PX: [u32; 2] = [512, 256];
const DAB_BOUNDS: [f32; 4] = [100.0, 40.0, 191.0, 130.0];
const BOUNDS_AT_THE_DAB_BEFORE: [f32; 4] = [121.0, 64.0, 212.0, 154.0];
const TILE_ONLY_THE_SWEEP_REACHES: TileCoord = TileCoord { x: 3, y: 0 };

fn tiles(bounds: [f32; 4]) -> Vec<TileCoord> {
    tiles_in_rect(CANVAS_PX[0], CANVAS_PX[1], bounds)
}

#[test]
fn a_dab_swept_diagonally_covers_the_tile_neither_of_its_ends_reaches() {
    for end in [DAB_BOUNDS, BOUNDS_AT_THE_DAB_BEFORE] {
        assert!(!tiles(end).contains(&TILE_ONLY_THE_SWEEP_REACHES));
    }
    let swept = covering(Some(DAB_BOUNDS), Some(BOUNDS_AT_THE_DAB_BEFORE)).unwrap();
    assert!(tiles(swept).contains(&TILE_ONLY_THE_SWEEP_REACHES));
    assert_eq!(swept, [100.0, 40.0, 212.0, 154.0]);
}

#[test]
fn a_dab_with_one_end_off_the_canvas_covers_the_end_on_it() {
    assert_eq!(covering(Some(DAB_BOUNDS), None), Some(DAB_BOUNDS));
    assert_eq!(covering(None, Some(DAB_BOUNDS)), Some(DAB_BOUNDS));
    assert_eq!(covering(None, None), None);
}

#[test]
fn a_dab_moved_to_the_dab_before_it_keeps_its_own_shape() {
    let dab = Dab {
        x: 10.0,
        y: 20.0,
        radius: 7.0,
        pressure: 0.6,
        altitude: 1.1,
        azimuth: 0.3,
        direction: 2.0,
        speed: 1.5,
        time_ms: 48.0,
    };
    let before = Dab {
        x: 4.0,
        y: 26.0,
        radius: 3.0,
        ..dab
    };
    assert_eq!(
        moved_to(&dab, &before),
        Dab {
            x: 4.0,
            y: 26.0,
            ..dab
        }
    );
}
