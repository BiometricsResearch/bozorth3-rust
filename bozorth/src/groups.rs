use crate::associations::{EndpointAssociations, EndpointRelation};
use crate::bozorth::FingerprintKind;
use crate::consts::max_number_of_groups;
use crate::is_strict_mode;
use crate::types::Endpoint;

pub(crate) type GroupVec = Vec<EndpointGroup>;

/// Represents a minutia from one fingerprint and collection of
/// possible corresponding ones from another fingerprint.
pub(crate) struct EndpointGroup {
    /// Minutia from the first fingerprint.
    endpoint: Endpoint,
    /// Kind of fingerprint that contains this minutia.
    endpoint_source: FingerprintKind,
    /// Collection of endpoints from another fingerprint
    /// that may match one from first fingerprint.
    /// These minutiae are located on a fingerprint with opposite kind.
    matching_endpoints: Vec<Endpoint>,

    /// Index of the currently selected endpoint in the list of potential corresponding minutiae.
    /// This is used during search of not conflicting pairs of endpoints among all the groups.
    endpoint_index: usize,
    /// Minutia from the other fingerprint that was recently associated.
    /// It is used to revert to the previous state.
    last_associated_from_probe: Option<Endpoint>,
}

/// Merges given endpoints into a group.
/// If endpoint is already in a group, it takes that one and adds `new_endpoint` into it.
/// Otherwise, it creates a new group.
pub(crate) fn merge_endpoints_into_group(
    groups: &mut Vec<EndpointGroup>,
    endpoint_source: FingerprintKind,
    endpoint: Endpoint,
    existing_endpoint: Endpoint,
    new_endpoint: Endpoint,
) {
    debug_assert_ne!(existing_endpoint, new_endpoint);

    if !is_strict_mode() {
        if groups.len() == max_number_of_groups() {
            return;
        }
    }

    let existing_group = groups
        .iter_mut()
        .find(|g| g.endpoint_source == endpoint_source && g.endpoint == endpoint);

    match existing_group {
        Some(group) => {
            // There is no need to add `existing_endpoint` since it had to be inserted earlier
            // during creation of this group.
            if !group.matching_endpoints.contains(&new_endpoint) {
                group.matching_endpoints.push(new_endpoint);
            }
        }
        None => {
            let last_associated_from_probe = if is_strict_mode() {
                None
            } else {
                // there is an old association that probably should be taken into account
                Some(existing_endpoint)
            };

            groups.push(EndpointGroup {
                endpoint,
                endpoint_source,
                matching_endpoints: vec![existing_endpoint, new_endpoint],
                endpoint_index: 0,
                last_associated_from_probe,
            });
        }
    }
}

#[inline]
pub(crate) fn cleanup_associations(
    groups: &mut [EndpointGroup],
    associator: &mut EndpointAssociations,
) {
    for group in groups.iter_mut() {
        if let Some(probe) = group.last_associated_from_probe.take() {
            associator.clear_by_probe(probe)
        }
    }
}

pub(crate) fn try_associate_current_endpoints(
    groups: &mut [EndpointGroup],
    associator: &mut EndpointAssociations,
) -> bool {
    // NOTE: it's not clear why iteration goes in a reverse order
    for group_index in (0..groups.len()).rev() {
        let group = &mut groups[group_index];
        let (probe_endpoint, gallery_endpoint) = match group.endpoint_source {
            FingerprintKind::Probe => (
                group.endpoint,
                group.matching_endpoints[group.endpoint_index],
            ),
            FingerprintKind::Gallery => (
                group.matching_endpoints[group.endpoint_index],
                group.endpoint,
            ),
        };

        match associator.get_status(probe_endpoint, gallery_endpoint) {
            EndpointRelation::Unassociated => {
                associator.associate(probe_endpoint, gallery_endpoint);
                groups[group_index].last_associated_from_probe = Some(probe_endpoint);
            }
            EndpointRelation::MutuallyAssociated => {
                if is_strict_mode() {
                    // NOTE: probably this should not be here
                    // since in many cases it does not preserve the previous state
                    // and affects following iterations
                    groups[group_index].last_associated_from_probe = Some(probe_endpoint);
                }
            }
            EndpointRelation::AssociatedToOther => {
                return false;
            }
        }
    }
    return true;
}

pub(crate) fn find_next_not_conflicting_associations(
    groups: &mut [EndpointGroup],
    associator: &mut EndpointAssociations,
) -> bool {
    cleanup_associations(groups, associator);

    // NOTE: probably order does not matter here... it should work just fine with forward iteration.
    // scores would be different, though
    let mut it = groups.iter_mut().rev();
    while let Some(group) = it.next() {
        if group.endpoint_index + 1 < group.matching_endpoints.len() {
            group.endpoint_index += 1;

            // Try to associate currently selected endpoint for all the groups.
            // All changes are restored after a failed association.
            if try_associate_current_endpoints(groups, associator) {
                return true;
            }

            // There is a conflict, so clear all made associations
            // and start from the beginning...
            cleanup_associations(groups, associator);
            it = groups.iter_mut().rev();
        } else {
            group.endpoint_index = 0;
        }
    }
    return false;
}
