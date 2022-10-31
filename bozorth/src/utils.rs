use std::cmp::Ord;

use crate::consts::{max_minutia_distance_squared, MAX_FILE_MINUTIAE, MIN_NUMBER_OF_EDGES};
use crate::parsing::RawMinutiaCombined;
use crate::weird_sort::sort_order_decreasing;
use crate::{is_strict_mode, Edge, Minutia};

pub fn prune(minutiae: &[RawMinutiaCombined], max_minutiae: u32) -> Vec<Minutia> {
    let mut minutiae = minutiae.to_vec();

    if is_strict_mode() {
        minutiae = if minutiae.len() > max_minutiae as usize {
            let mut quality = [0; MAX_FILE_MINUTIAE];
            for i in 0..minutiae.len() {
                quality[i] = minutiae[i].q;
            }

            let mut order = [0; MAX_FILE_MINUTIAE];
            sort_order_decreasing(&quality[..minutiae.len()], &mut order[..minutiae.len()]);
            order[..max_minutiae as usize]
                .iter()
                .map(|&index| minutiae[index])
                .collect()
        } else {
            minutiae
        }
    } else {
        if minutiae.len() > max_minutiae as usize {
            minutiae.sort_by_key(|m| -m.q);
            minutiae.truncate(max_minutiae as usize);
        }
    }

    minutiae.sort_by_key(|it| (it.x, it.y));
    minutiae
        .into_iter()
        .map(|it| Minutia {
            x: it.x,
            y: it.y,
            theta: it.t,
            kind: it.kind,
        })
        .collect()
}

pub fn limit_edges(edges: &[Edge]) -> usize {
    let limit = if is_strict_mode() {
        limit_edges_by_length(edges, max_minutia_distance_squared())
    } else {
        match edges.binary_search_by_key(&max_minutia_distance_squared(), |e| e.distance_squared) {
            Ok(pos) | Err(pos) => pos,
        }
    };

    MIN_NUMBER_OF_EDGES.max(limit).min(edges.len())
}

fn limit_edges_by_length(edges: &[Edge], max_distance: i32) -> usize {
    let mut lower = 0;
    let mut upper = edges.len() + 1;
    let mut current = 1;

    while upper - lower > 1 {
        let midpoint = (lower + upper) / 2;
        if edges[midpoint - 1].distance_squared > max_distance {
            upper = midpoint;
        } else {
            lower = midpoint;
            current = midpoint + 1;
        }
    }

    current.min(edges.len())
}
