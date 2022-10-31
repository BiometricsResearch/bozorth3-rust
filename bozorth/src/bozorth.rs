use crate::associations::EndpointAssociations;
use crate::clusters::{
    calculate_averages, combine_clusters, encode_selected_endpoints,
    find_compatible_disjoint_clusters_and_accumulate_points, ClusterAssigner, ClusterSimilar,
    Clusters,
};
use crate::consts::{
    max_number_of_clusters, max_number_of_groups, min_number_of_pairs_to_build_cluster,
    score_threshold,
};
use crate::groups::{find_next_not_conflicting_associations, merge_endpoints_into_group, GroupVec};
use crate::math::{are_angles_equal_with_tolerance, Averager};
use crate::types::Endpoint;
use crate::{is_strict_mode, timeit, Format, Minutia, PairHolder};

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
#[repr(u8)]
pub(crate) enum FingerprintKind {
    Probe,
    Gallery,
}

#[inline]
fn calculate_average_delta_theta_for_pairs(selected_pairs: &[u32], pairs: &PairHolder) -> i32 {
    let mut averager = Averager::new();
    for &pair in selected_pairs {
        averager.push(pairs.get(pair as usize).delta_theta);
    }
    averager.average()
}

#[inline]
fn filter_selected(selected_pairs: &mut Vec<u32>, pairs: &PairHolder) {
    let average = calculate_average_delta_theta_for_pairs(selected_pairs, pairs);
    selected_pairs.retain(|&pair| {
        are_angles_equal_with_tolerance(pairs.get(pair as usize).delta_theta, average)
    });
}

#[inline]
fn cleanup_selected(cluster_assigner: &mut ClusterAssigner, selected_pairs: &[u32]) {
    for &pair in selected_pairs {
        cluster_assigner.unassign(pair)
    }
}

fn assign_cluster_to_endpoints(
    cluster: u32,
    pair_index: u32,
    probe_endpoint: Endpoint,
    gallery_endpoint: Endpoint,
    state: &mut BozorthState,
    to_visit: &mut Vec<(Endpoint, Endpoint)>,
) {
    // Check relation between given endpoints in current traversal.
    match (
        state.associator.get_associated_by_probe(probe_endpoint),
        state.associator.get_associated_by_gallery(gallery_endpoint),
    ) {
        // Both endpoints are unassociated.
        (None, None) => {
            // check whether this pair was seen previously when creating this cluster.
            // Unless it was, add it to the cluster.
            if state.assigner.get_cluster(pair_index) != Some(cluster) {
                // save pair that the minutia is an endpoint of...
                state.selected_pairs.push(pair_index);
                // and assign cluster to that pair
                state.assigner.assign(pair_index, cluster);
            }

            // Associate endpoints ...
            state.associator.associate(probe_endpoint, gallery_endpoint);
            // ... then save them in order to track last traversed endpoint
            // and clear the associations after the traversal.
            to_visit.push((probe_endpoint, gallery_endpoint));
        }
        // The endpoints are already mutually associated.
        (Some(endpoint), Some(_)) if endpoint == gallery_endpoint => {
            // If it was done when constructing the same cluster, we do not have to do anything.
            if state.assigner.get_cluster(pair_index) == Some(cluster) {
                // pair was already visited in this traversal -- no need to do anything
                return;
            }
            state.selected_pairs.push(pair_index);
            state.assigner.assign(pair_index, cluster);

            if is_strict_mode() {
                // NOTE: this should be `pair_index` instead of `probe_endpoint`,
                // but we are keeping this implementation strictly identical to the original one
                let should_insert = to_visit
                    .iter()
                    .copied()
                    .all(|(endpoint, _)| endpoint != pair_index.into());
                if should_insert {
                    to_visit.push((probe_endpoint, gallery_endpoint));
                }
            }
        }
        (existing_gallery_endpoint, existing_probe_endpoint) => {
            if is_strict_mode() {
                // Limit number of produced groups.
                if state.groups.len() >= max_number_of_groups() {
                    return;
                }
            }

            // there exists an association already so create a new group
            if let Some(endpoint) = existing_gallery_endpoint {
                merge_endpoints_into_group(
                    &mut state.groups,
                    FingerprintKind::Probe,
                    probe_endpoint,
                    endpoint,
                    gallery_endpoint,
                );
            }

            // just like previously...
            if let Some(endpoint) = existing_probe_endpoint {
                merge_endpoints_into_group(
                    &mut state.groups,
                    FingerprintKind::Gallery,
                    gallery_endpoint,
                    endpoint,
                    probe_endpoint,
                );
            }
        }
    }
}

fn traverse_edges(
    pairs: &PairHolder,
    start_pair: u32,
    cluster_index: u32,
    state: &mut BozorthState,
) {
    // queue of endpoints to visit
    let mut to_visit = vec![];

    let start = pairs.get(start_pair as usize);
    let (iterator, next_not_connected) =
        pairs.find_pairs_by_first_endpoint(start_pair as usize, start.probe_k, start.gallery_k);

    for (index, probe_j, gallery_j) in iterator {
        assign_cluster_to_endpoints(
            cluster_index,
            index as u32,
            probe_j,
            gallery_j,
            state,
            &mut to_visit,
        );
    }

    let mut cursor = 0;
    while cursor < to_visit.len() {
        let (probe_endpoint, gallery_endpoint) = to_visit[cursor];
        cursor += 1;

        let (iterator, _) = pairs.find_pairs_by_second_endpoint(
            next_not_connected,
            probe_endpoint,
            gallery_endpoint,
        );

        for (index, probe_k, gallery_k) in iterator {
            if probe_k != start.probe_k && gallery_k != start.gallery_k {
                assign_cluster_to_endpoints(
                    cluster_index,
                    index as u32,
                    probe_k,
                    gallery_k,
                    state,
                    &mut to_visit,
                );
            }
        }

        let (iterator, _) = pairs.find_pairs_by_first_endpoint(
            next_not_connected,
            probe_endpoint,
            gallery_endpoint,
        );

        for (index, probe_j, gallery_j) in iterator {
            assign_cluster_to_endpoints(
                cluster_index,
                index as u32,
                probe_j,
                gallery_j,
                state,
                &mut to_visit,
            );
        }
    }

    // Restore previous state
    for (probe_endpoint, _) in to_visit.iter().copied() {
        state.associator.clear_by_probe(probe_endpoint);
    }
}

pub struct BozorthState {
    pub clusters: Clusters,
    associator: EndpointAssociations,
    assigner: ClusterAssigner,
    /// When there is an endpoint that has more than one potentially compatible endpoint
    /// from another fingerprint, a group is created that holds these endpoints.
    /// Later, a brute force checking is performed that looks for a combinations of associations
    /// for which there are no conflicts among all the groups.
    groups: GroupVec,
    selected_pairs: Vec<u32>,
}

impl BozorthState {
    pub fn new() -> Self {
        BozorthState {
            clusters: Clusters::with_capacity(max_number_of_clusters()),
            associator: EndpointAssociations::new(),
            assigner: ClusterAssigner::new(),
            groups: GroupVec::new(),
            selected_pairs: vec![],
        }
    }

    pub fn len(&self) -> usize {
        self.groups.len()
    }

    pub fn clear(&mut self) {
        self.clusters.clear();
        self.associator.clear();
        self.assigner.clear();
        self.groups.clear();
        self.selected_pairs.clear();
    }
}

const MINIMAL_NUMBER_OF_MINUTIA: usize = 10;

fn calculate_points(pairs: &PairHolder, selected_pairs: &[u32]) -> u32 {
    selected_pairs
        .iter()
        .map(|it| pairs.get(*it as usize).points)
        .sum()
}

fn maybe_create_cluster(
    probe_minutiae: &[Minutia],
    gallery_minutiae: &[Minutia],
    pairs: &PairHolder,
    start_pair: u32,
    state: &mut BozorthState,
) {
    let new_cluster_index = state.clusters.len();
    state.selected_pairs.clear();

    traverse_edges(pairs, start_pair, new_cluster_index as u32, state);

    if state.selected_pairs.len() >= min_number_of_pairs_to_build_cluster() {
        filter_selected(&mut state.selected_pairs, pairs);
    }

    if state.selected_pairs.len() < min_number_of_pairs_to_build_cluster() {
        cleanup_selected(&mut state.assigner, &state.selected_pairs);
    } else {
        state.clusters.push(
            ClusterSimilar {
                points: calculate_points(&pairs, &state.selected_pairs),
                points_including_compatible_clusters: 0,
                compatible_clusters: vec![],
            },
            calculate_averages(
                probe_minutiae,
                gallery_minutiae,
                pairs,
                &state.selected_pairs,
            ),
            encode_selected_endpoints(pairs, &state.selected_pairs),
            // {
            //     let mut eps = Vec::new();
            //     for pair in state.selected_pairs.iter() {
            //         let pair = pairs.get(*pair as usize);
            //         eps.push((pair.probe_k, pair.gallery_k));
            //         eps.push((pair.probe_j, pair.gallery_j));
            //     }
            //     eps.sort();
            //     eps.dedup();
            //     eps
            // },
            state.selected_pairs.clone(),
        );
    }
}

pub fn match_score(
    pairs: &PairHolder,
    probe_minutiae: &[Minutia],
    gallery_minutiae: &[Minutia],
    format: Format,
    state: &mut BozorthState,
) -> Result<(u32, Vec<u32>), ()> {
    if probe_minutiae.len() < MINIMAL_NUMBER_OF_MINUTIA
        || gallery_minutiae.len() < MINIMAL_NUMBER_OF_MINUTIA
    {
        return Err(());
    }
    debug_assert!(!pairs.is_empty());

    timeit(|| state.clear());
    for (start_pair_index, start_pair) in pairs
        .iter()
        .take(if is_strict_mode() {
            pairs.len() - 1
        } else {
            pairs.len()
        })
        .enumerate()
    {
        if state
            .assigner
            .get_cluster(start_pair_index as u32)
            .is_some()
        {
            // Was assigned to some cluster already so it was visited - no need to do it again
            continue;
        }
        state
            .associator
            .associate(start_pair.probe_k, start_pair.gallery_k);
        state.groups.clear();

        loop {
            timeit(|| {
                maybe_create_cluster(
                    probe_minutiae,
                    gallery_minutiae,
                    pairs,
                    start_pair_index as u32,
                    state,
                );
            });

            if state.clusters.len() > max_number_of_clusters() - 1 {
                break;
            }

            if !find_next_not_conflicting_associations(
                state.groups.as_mut_slice(),
                &mut state.associator,
            ) {
                // no more clusters can be created
                break;
            }
        }

        if state.clusters.len() > max_number_of_clusters() - 1 {
            break;
        }
        state.associator.clear_by_probe(start_pair.probe_k);
    }

    timeit(|| find_compatible_disjoint_clusters_and_accumulate_points(&mut state.clusters, format));

    // NOTE: some interesting heuristics?
    let (initial_score, clusters) = state
        .clusters
        .similar
        .iter()
        .enumerate()
        .map(|(idx, cluster)| {
            (
                cluster.points_including_compatible_clusters,
                std::iter::once(idx as u32)
                    .chain(cluster.compatible_clusters.iter().copied())
                    .collect(),
            )
        })
        .max_by_key(|item| item.0)
        .unwrap_or((0, vec![]));

    Ok(if initial_score < score_threshold() {
        (initial_score, clusters)
    } else {
        timeit(|| combine_clusters(&mut state.clusters, false))
    })
}
