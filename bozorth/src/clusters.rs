use bitarray::BitArray;
use typenum::U256;

use crate::consts::{factor, MAX_NUMBER_OF_PAIRS};
use crate::math::{
    are_angles_equal_with_tolerance, average_angles, calculate_slope_in_degrees, normalize_angle,
    Averager,
};
use crate::set_intersection::intersection_of_sorted;
use crate::{is_strict_mode, Format, Minutia, PairHolder};
use std::collections::{HashSet, VecDeque};

pub(crate) struct ClusterAssigner {
    cluster_by_pair: [u32; MAX_NUMBER_OF_PAIRS],
}

const MARKER_UNASSIGNED: u32 = u32::max_value();

impl ClusterAssigner {
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            cluster_by_pair: [0; MAX_NUMBER_OF_PAIRS],
        }
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        self.cluster_by_pair.iter_mut().for_each(|it| *it = 0);
    }

    /// Gets cluster assigned to given pair of edges.
    #[inline]
    pub(crate) fn get_cluster(&self, pair_index: u32) -> Option<u32> {
        let cluster = self.cluster_by_pair[pair_index as usize];
        if cluster == 0 {
            None
        } else {
            Some((cluster - 1) as u32)
        }
    }

    #[inline]
    pub(crate) fn assign(&mut self, pair_index: u32, cluster: u32) {
        self.cluster_by_pair[pair_index as usize] = cluster + 1;
    }

    #[inline]
    pub(crate) fn unassign(&mut self, pair_index: u32) {
        if is_strict_mode() {
            self.cluster_by_pair[pair_index as usize] = MARKER_UNASSIGNED;
        } else {
            // XXX: clearing makes more sense
            self.cluster_by_pair[pair_index as usize] = 0;
        }
    }
}

/// Structure containing various averages for pairs in a cluster.
pub(crate) struct ClusterAverages {
    /// Average of `delta_theta` angles
    delta_theta: i32,
    /// Average value of `x` coordinates in fingerprint P
    probe_x: i32,
    /// Average value of `y` coordinates in fingerprint P
    probe_y: i32,
    /// Average value of `x` coordinates in fingerprint G
    gallery_x: i32,
    /// Average value of `y` coordinates in fingerprint G
    gallery_y: i32,
}

/// Packed structure that contains all minutiae that are included in the cluster.
/// Optimized for fast comparison between different clusters.
pub(crate) struct ClusterEndpoints {
    /// Minutiae of fingerprint P
    probe: BitArray<u64, U256>,
    /// Minutiae of fingerprint G
    gallery: BitArray<u64, U256>,
}

/// Builds a `ClusterEndpoints` structure for given collection of pairs.
pub(crate) fn encode_selected_endpoints(pairs: &PairHolder, selected: &[u32]) -> ClusterEndpoints {
    let mut probe = BitArray::new();
    let mut gallery = BitArray::new();
    for &idx in selected {
        let pair = pairs.get(idx as usize);
        probe.set(pair.probe_k.as_usize(), true);
        probe.set(pair.probe_j.as_usize(), true);
        gallery.set(pair.gallery_k.as_usize(), true);
        gallery.set(pair.gallery_j.as_usize(), true);
    }
    ClusterEndpoints { probe, gallery }
}

pub(crate) struct ClusterSimilar {
    /// Number of points for this particular cluster.
    pub(crate) points: u32,
    /// Collection of clusters that are compatible - located in similar position on a fingerprint.
    pub(crate) compatible_clusters: Vec<u32>,
    /// Precalculated sum of points for all compatible clusters.
    /// It is not strictly necessary, but helps to avoid some potentially expensive calculations.
    /// See: `combine_clusters`
    pub(crate) points_including_compatible_clusters: u32,
}

pub struct Clusters {
    pub(crate) similar: Vec<ClusterSimilar>,
    averages: Vec<ClusterAverages>,
    endpoints: Vec<ClusterEndpoints>,
    // pub(crate) e2e: Vec<Vec<(Endpoint, Endpoint)>>,
    pub pairs: Vec<Vec<u32>>,
}

impl Clusters {
    #[inline]
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Clusters {
            similar: Vec::with_capacity(capacity),
            averages: Vec::with_capacity(capacity),
            endpoints: Vec::with_capacity(capacity),
            pairs: Vec::new(),
        }
    }

    #[inline]
    pub(crate) fn push(
        &mut self,
        cluster: ClusterSimilar,
        averages: ClusterAverages,
        endpoints: ClusterEndpoints,
        selected: Vec<u32>,
    ) {
        self.similar.push(cluster);
        self.averages.push(averages);
        self.endpoints.push(endpoints);
        self.pairs.push(selected);
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.similar.len()
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        self.similar.clear();
        self.averages.clear();
        self.endpoints.clear();
        self.pairs.clear();
    }
}

/// Check if one cluster is compatible to another by comparing their various averages.
fn are_clusters_compatible(
    averages1: &ClusterAverages,
    averages2: &ClusterAverages,
    format: Format,
) -> bool {
    if !are_angles_equal_with_tolerance(averages2.delta_theta, averages1.delta_theta) {
        return false;
    }

    let probe_dx = averages2.probe_x - averages1.probe_x;
    let probe_dy = averages2.probe_y - averages1.probe_y;
    let gallery_dx = averages2.gallery_x - averages1.gallery_x;
    let gallery_dy = averages2.gallery_y - averages1.gallery_y;

    let probe_distance_squared = probe_dx.pow(2) + probe_dy.pow(2);
    let gallery_distance_squared = gallery_dx.pow(2) + gallery_dy.pow(2);

    let a = 2.0 * factor() * (probe_distance_squared + gallery_distance_squared) as f32;
    let b = ((probe_distance_squared - gallery_distance_squared) as f32).abs();
    if b > a {
        return false;
    }

    let average = average_angles(averages1.delta_theta, averages2.delta_theta);
    let difference = match format {
        Format::NistInternal => {
            calculate_slope_in_degrees(probe_dx, probe_dy)
                - calculate_slope_in_degrees(gallery_dx, gallery_dy)
        }
        Format::Ansi => {
            calculate_slope_in_degrees(probe_dx, -probe_dy)
                - calculate_slope_in_degrees(gallery_dx, -gallery_dy)
        }
    };

    are_angles_equal_with_tolerance(average, normalize_angle(difference))
}

/// Check whether clusters include common minutiae.
fn have_common_endpoints(first: &ClusterEndpoints, second: &ClusterEndpoints) -> bool {
    first
        .probe
        .blocks()
        .zip(second.probe.blocks())
        .any(|(a, b)| a & b != 0)
        || first
            .gallery
            .blocks()
            .zip(second.gallery.blocks())
            .any(|(a, b)| a & b != 0)
}

/// Go through all the clusters and try to find ones that do not have common minutiae
/// and are compatible.
pub(crate) fn find_compatible_disjoint_clusters_and_accumulate_points(
    clusters: &mut Clusters,
    format: Format,
) {
    for cluster in 0..clusters.similar.len() {
        let mut points_from_others = 0;
        let mut compatible_clusters = vec![];

        for other_cluster in cluster + 1..clusters.similar.len() {
            if have_common_endpoints(
                &clusters.endpoints[cluster],
                &clusters.endpoints[other_cluster],
            ) {
                continue;
            }

            if !are_clusters_compatible(
                &clusters.averages[cluster],
                &clusters.averages[other_cluster],
                format,
            ) {
                continue;
            }

            points_from_others += clusters.similar[other_cluster].points;
            compatible_clusters.push(other_cluster as u32);
        }

        clusters.similar[cluster].points_including_compatible_clusters =
            clusters.similar[cluster].points + points_from_others;
        clusters.similar[cluster].compatible_clusters = compatible_clusters;
    }
}

/// Calculate averages of various properties for a collection of pairs.
pub(crate) fn calculate_averages(
    probe_minutiae: &[Minutia],
    gallery_minutiae: &[Minutia],
    pairs: &PairHolder,
    selected_pairs: &[u32],
) -> ClusterAverages {
    let mut average = ClusterAverages {
        delta_theta: 0,
        probe_x: 0,
        probe_y: 0,
        gallery_x: 0,
        gallery_y: 0,
    };

    let mut averager = Averager::new();

    for &pair_index in selected_pairs {
        let pair = pairs.get(pair_index as usize);
        averager.push(pair.delta_theta);

        let probe_endpoint = pair.probe_k.as_usize();
        average.probe_x += probe_minutiae[probe_endpoint].x;
        average.probe_y += probe_minutiae[probe_endpoint].y;

        let gallery_endpoint = pair.gallery_k.as_usize();
        average.gallery_x += gallery_minutiae[gallery_endpoint].x;
        average.gallery_y += gallery_minutiae[gallery_endpoint].y;
    }

    average.delta_theta = averager.average();
    average.probe_x /= selected_pairs.len() as i32;
    average.probe_y /= selected_pairs.len() as i32;
    average.gallery_x /= selected_pairs.len() as i32;
    average.gallery_y /= selected_pairs.len() as i32;
    average
}

/// Calculates the highest sum of points for compatible clusters.
pub(crate) fn combine_clusters(
    clusters: &Clusters,
    collect_compatible_clusters: bool,
) -> (u32, Vec<u32>) {
    #[derive(Debug)]
    struct Item {
        cluster: u32,
        connected: Vec<u32>,
        index: u32,
    }

    let mut items = vec![];
    let mut best_score = 0;
    let mut minutiae_of_biggest = vec![];

    for (cluster_index, cluster) in clusters.similar.iter().enumerate() {
        // NOTE: it looks like a heuristic, it helps to avoid unnecessary calculations
        if best_score >= cluster.points_including_compatible_clusters {
            continue;
        }

        items.push(Item {
            cluster: cluster_index as u32,
            index: 0,
            connected: cluster.compatible_clusters.clone(),
        });

        while let Some(last) = items.last() {
            if (last.index as usize) < last.connected.len() {
                let next_cluster = last.connected[last.index as usize] as usize;

                // find all possible clusters that should be visited later
                let connected_clusters = intersection_of_sorted(
                    last.connected.iter(),
                    clusters.similar[next_cluster].compatible_clusters.iter(),
                )
                .copied()
                .collect();

                items.push(Item {
                    cluster: next_cluster as u32,
                    connected: connected_clusters,
                    index: 0,
                });
            } else {
                // there is no more clusters connected to the current one
                if last.connected.is_empty() {
                    // we can't go any further from here so we calculate total score
                    let score: u32 = items
                        .iter()
                        .map(|it| clusters.similar[it.cluster as usize].points)
                        .sum();

                    // let path = items.iter().map(|it| it.cluster).collect::<Vec<_>>();
                    // println!("{} {:?}", cluster_index, &path);

                    if score > best_score {
                        best_score = score;
                        if collect_compatible_clusters {
                            minutiae_of_biggest = items
                                .iter()
                                .flat_map(|it| {
                                    clusters.similar[it.cluster as usize]
                                        .compatible_clusters
                                        .iter()
                                })
                                .copied()
                                .collect();
                            minutiae_of_biggest.sort();
                            minutiae_of_biggest.dedup();
                        }
                    }
                }

                // so we can take it from the stack and then traverse another connections
                items.pop().unwrap();
                // Move to next cluster if such exists.
                if let Some(last) = items.last_mut() {
                    last.index += 1;
                }
            }
        }
    }

    (best_score, minutiae_of_biggest)
}

#[allow(unused)]
pub(crate) fn combine_clusters_2(
    clusters: &Clusters,
    collect_compatible_clusters: bool,
) -> (u32, Vec<u32>) {
    assert!(!collect_compatible_clusters);

    let mut best_score = 0;
    let mut stack = VecDeque::new();
    let mut visited = HashSet::new();

    for (cluster_index, cluster) in clusters.similar.iter().enumerate() {
        if best_score >= cluster.points_including_compatible_clusters {
            continue;
        }

        stack.push_back(cluster_index as u32);
        let mut val = 0;
        visited.clear();
        while let Some(n) = stack.pop_back() {
            if visited.insert(n) {
                val += clusters.similar[n as usize].points;
            }
            for node in clusters.similar[n as usize]
                .compatible_clusters
                .iter()
                .copied()
            {
                if !visited.contains(&node) {
                    stack.push_back(node);
                }
            }
        }
        best_score = best_score.max(val);
    }

    (best_score, vec![])
}
