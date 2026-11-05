use super::{CLUSTER_SPAN_WORDS, MAX_CLUSTERS, MEMBERS_START, cluster_index};
use crate::brush::HeadSpec;
use crate::engine::bristles::layout::{LayoutRequest, layout};

fn members_of(index: &[u32], cluster: usize) -> &[u32] {
    let start = index[cluster * CLUSTER_SPAN_WORDS] as usize;
    let size = index[cluster * CLUSTER_SPAN_WORDS + 1] as usize;
    &index[MEMBERS_START + start..MEMBERS_START + start + size]
}

#[test]
fn the_cluster_index_lists_each_bristle_once_under_its_own_cluster() {
    let clusters = [2, 0, 1, 0, 2, 2, 1];
    let index = cluster_index(&clusters);
    assert_eq!(members_of(&index, 0), &[1, 3]);
    assert_eq!(members_of(&index, 1), &[2, 6]);
    assert_eq!(members_of(&index, 2), &[0, 4, 5]);
    assert!((3..MAX_CLUSTERS).all(|cluster| members_of(&index, cluster).is_empty()));
}

#[test]
fn a_laid_out_brush_indexes_every_bristle_in_its_cluster() {
    for head in [HeadSpec::ROUND, HeadSpec::FLAT, HeadSpec::FAN] {
        let laid = layout(&LayoutRequest {
            head: HeadSpec {
                length: 2.0,
                ..head
            },
            ferrule_radius_px: 120.0,
            seed: 3,
        });
        let mut seen = vec![false; laid.bristles.len()];
        let listed = (0..MAX_CLUSTERS).flat_map(|cluster| {
            members_of(&laid.cluster_index, cluster)
                .iter()
                .map(move |&member| (cluster, member))
        });
        for (cluster, member) in listed {
            assert_eq!(laid.bristles[member as usize].cluster as usize, cluster);
            assert!(
                !seen[member as usize],
                "{head:?} bristle {member} listed twice"
            );
            seen[member as usize] = true;
        }
        assert!(seen.into_iter().all(|listed| listed), "{head:?}");
    }
}
