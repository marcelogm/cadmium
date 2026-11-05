use super::{CELL_GRID_BINS, CellFrame, cell_grids};
use crate::brush::HeadSpec;
use crate::engine::bristles::layout::{LayoutRequest, layout};

fn laid(head: HeadSpec) -> super::super::layout::BristleLayout {
    layout(&LayoutRequest {
        head: HeadSpec {
            length: 2.5,
            ..head
        },
        ferrule_radius_px: 20.0,
        seed: 7,
    })
}

#[test]
fn every_grid_bin_points_into_its_own_bristles_cell() {
    for head in [HeadSpec::ROUND, HeadSpec::FLAT, HeadSpec::FAN] {
        let laid = laid(head);
        let (grids, _) = laid.cell_grids.as_chunks::<CELL_GRID_BINS>();
        for (bristle, grid) in grids.iter().enumerate() {
            let owned = laid.owners.contains(&(bristle as u32));
            assert!(
                !owned
                    || grid
                        .iter()
                        .all(|&slot| laid.owners[slot as usize] == bristle as u32),
                "{head:?} bristle {bristle}"
            );
        }
    }
}

#[test]
fn a_cells_grid_reaches_most_of_its_texels() {
    let laid = laid(HeadSpec::ROUND);
    let (grids, _) = laid.cell_grids.as_chunks::<CELL_GRID_BINS>();
    for (bristle, grid) in grids.iter().enumerate() {
        let owned = laid
            .owners
            .iter()
            .filter(|&&owner| owner == bristle as u32)
            .count();
        let mut reached: Vec<u32> = grid.to_vec();
        reached.sort_unstable();
        reached.dedup();
        assert!(
            2 * reached.len() >= owned.min(CELL_GRID_BINS),
            "bristle {bristle} reaches {} of {owned} texels",
            reached.len()
        );
    }
}

#[test]
fn a_cell_without_texels_falls_back_to_its_given_slot() {
    let owners = vec![0u32; 4];
    let frames = [
        CellFrame {
            direction: [1.0, 0.0],
            fallback_slot: 9,
        },
        CellFrame {
            direction: [1.0, 0.0],
            fallback_slot: 42,
        },
    ];
    let grids = cell_grids(&owners, &frames);
    assert!(grids[CELL_GRID_BINS..].iter().all(|&slot| slot == 42));
    assert!(grids[..CELL_GRID_BINS].iter().all(|&slot| slot < 4));
}
