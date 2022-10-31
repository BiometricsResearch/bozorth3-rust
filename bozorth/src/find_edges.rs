use crate::consts::{max_minutia_distance, MAX_NUMBER_OF_EDGES};
use crate::math::{are_angles_opposite, atan2_round_degree, normalize_angle};
use crate::{BetaOrder, Edge, Format, Minutia};

pub fn find_edges(minutiae: &[Minutia], edges: &mut Vec<Edge>, format: Format) {
    assert!(!minutiae.is_empty());

    'main: for k in 0..minutiae.len() - 1 {
        for j in k + 1..minutiae.len() {
            if are_angles_opposite(minutiae[k].theta, minutiae[j].theta) {
                continue;
            }

            let dx = minutiae[j].x - minutiae[k].x;
            let dy = minutiae[j].y - minutiae[k].y;
            let distance_squared = dx.pow(2) + dy.pow(2);
            if distance_squared > max_minutia_distance().pow(2) {
                if dx > max_minutia_distance() {
                    break;
                } else {
                    continue;
                }
            }

            let theta_kj = atan2_round_degree(
                dx,
                match format {
                    Format::NistInternal => dy,
                    Format::Ansi => -dy,
                },
            );

            let beta_k = normalize_angle(theta_kj - minutiae[k].theta);
            let beta_j = normalize_angle(theta_kj - minutiae[j].theta + 180);
            let (min_beta, max_beta, beta_order) = if beta_k < beta_j {
                (beta_k, beta_j, BetaOrder::KJ)
            } else {
                (beta_j, beta_k, BetaOrder::JK)
            };

            edges.push(Edge {
                distance_squared,
                min_beta,
                max_beta,
                endpoint_k: k.into(),
                endpoint_j: j.into(),
                theta_kj,
                beta_order,
            });
            if edges.len() == MAX_NUMBER_OF_EDGES - 1 {
                break 'main;
            }
        }
    }

    edges.sort_by_key(|edge| (edge.distance_squared, edge.min_beta, edge.max_beta));
}
