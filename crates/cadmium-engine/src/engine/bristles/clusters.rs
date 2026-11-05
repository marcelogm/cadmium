use super::layout::{BRISTLES_PER_CLUSTER, MAX_BRISTLES};

pub(crate) const MAX_CLUSTERS: usize = MAX_BRISTLES / BRISTLES_PER_CLUSTER;
pub(crate) const CLUSTER_SPAN_WORDS: usize = 2;
pub(super) const CLUSTER_INDEX_WORDS: usize = MAX_CLUSTERS * CLUSTER_SPAN_WORDS + MAX_BRISTLES;
const MEMBERS_START: usize = MAX_CLUSTERS * CLUSTER_SPAN_WORDS;

pub(super) fn cluster_index(clusters: &[u32]) -> Vec<u32> {
    let mut index = vec![0u32; CLUSTER_INDEX_WORDS];
    let mut filled = 0;
    for cluster in 0..MAX_CLUSTERS {
        let start = filled;
        let members = (0..clusters.len()).filter(|&bristle| clusters[bristle] as usize == cluster);
        for bristle in members {
            index[MEMBERS_START + filled] = bristle as u32;
            filled += 1;
        }
        index[cluster * CLUSTER_SPAN_WORDS] = start as u32;
        index[cluster * CLUSTER_SPAN_WORDS + 1] = (filled - start) as u32;
    }
    index
}

#[cfg(test)]
mod tests;
