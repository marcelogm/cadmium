use super::*;

#[test]
fn the_hull_of_a_square_and_its_centre_is_the_square() {
    let hull = convex_hull(vec![
        [0.0, 0.0],
        [2.0, 0.0],
        [1.0, 1.0],
        [2.0, 2.0],
        [0.0, 2.0],
    ]);
    assert_eq!(hull.len(), 4);
    for corner in [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]] {
        assert!(hull.contains(&corner), "{hull:?}");
    }
}

#[test]
fn a_hull_runs_counter_clockwise() {
    let hull = convex_hull(vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]]);
    let doubled_area: f32 = (0..hull.len())
        .map(|i| {
            let [a, b] = [hull[i], hull[(i + 1) % hull.len()]];
            a[0] * b[1] - b[0] * a[1]
        })
        .sum();
    assert!(doubled_area > 0.0, "{hull:?}");
}
