use cadmium_engine::canvas::*;

#[test]
fn dirty_rect_marks_every_touched_tile_once() {
    let mut d = DirtyTiles::new(256, 128);
    d.mark_rect(60.0, 10.0, 70.0, 70.0);
    assert_eq!(
        d.take(),
        vec![
            TileCoord { x: 0, y: 0 },
            TileCoord { x: 0, y: 1 },
            TileCoord { x: 1, y: 0 },
            TileCoord { x: 1, y: 1 }
        ]
    );
    assert!(d.is_empty());
}

#[test]
fn dirty_rect_is_clamped_to_the_canvas() {
    let mut d = DirtyTiles::new(100, 100);
    d.mark_rect(-50.0, 90.0, 500.0, 900.0);
    assert_eq!(
        d.tiles(),
        vec![TileCoord { x: 0, y: 1 }, TileCoord { x: 1, y: 1 }]
    );
    d.mark_rect(-20.0, -20.0, -10.0, -10.0);
    assert_eq!(d.tiles().len(), 2);
}

#[test]
fn rect_ending_on_a_tile_boundary_marks_the_next_tile() {
    assert_eq!(
        tiles_in_rect(256, 128, [0.0, 0.0, 64.0, 64.0]),
        vec![
            TileCoord { x: 0, y: 0 },
            TileCoord { x: 0, y: 1 },
            TileCoord { x: 1, y: 0 },
            TileCoord { x: 1, y: 1 }
        ]
    );
}

#[test]
fn rect_ending_inside_the_last_pixel_of_a_tile_stays_in_that_tile() {
    assert_eq!(
        tiles_in_rect(256, 128, [0.0, 0.0, 63.9, 63.9]),
        vec![TileCoord { x: 0, y: 0 }]
    );
}

#[test]
fn rect_starting_past_the_last_pixel_marks_nothing() {
    assert!(tiles_in_rect(100, 100, [100.0, 0.0, 120.0, 10.0]).is_empty());
    assert_eq!(
        tiles_in_rect(100, 100, [99.5, 0.0, 120.0, 10.0]),
        vec![TileCoord { x: 1, y: 0 }]
    );
}

#[test]
fn point_rect_marks_its_tile() {
    assert_eq!(
        tiles_in_rect(256, 128, [100.0, 100.0, 100.0, 100.0]),
        vec![TileCoord { x: 1, y: 1 }]
    );
}

#[test]
fn mark_all_covers_partial_edge_tiles() {
    let mut d = DirtyTiles::new(130, 65);
    d.mark_all();
    assert_eq!(d.tiles().len(), 3 * 2);
    d.clear();
    assert!(d.is_empty());
}
