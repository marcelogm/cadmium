use super::*;

#[test]
fn a_snapshot_reads_each_strands_anchors_and_plastic_shifts() {
    let mut floats = vec![0.0; 2 * STATE_FLOATS];
    let second = STATE_FLOATS;
    let anchor = second + (ANCHOR_VECTORS_FIRST + 3) * VECTOR_FLOATS;
    floats[anchor..anchor + 4].copy_from_slice(&[1.0, 2.0, 3.0, -0.5]);
    let shift = second + (PLASTIC_VECTORS_FIRST + 2) * VECTOR_FLOATS;
    floats[shift..shift + 3].copy_from_slice(&[0.01, 0.02, 0.0]);
    let snapshot = BristleSnapshot::of(&floats, &[0.0, 0.0], &[1.0, 1.0], &[0, 0]);
    assert_eq!(snapshot.anchors[1][3], [1.0, 2.0, 3.0, -0.5]);
    assert_eq!(snapshot.anchors[0][3], [0.0; 4]);
    assert_eq!(snapshot.plastic[1][2], [0.01, 0.02, 0.0]);
    assert_eq!(snapshot.rest_bends[1], [-0.02, 0.01]);
    assert_eq!(snapshot.rest_bends[0], [0.0, 0.0]);
}
