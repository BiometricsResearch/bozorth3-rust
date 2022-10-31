// use crate::consts::ANGLE_LOWER_BOUND;
// use crate::consts::ANGLE_UPPER_BOUND;
// use crate::edge_holder::EdgeHolder;
use crate::math::{are_angles_equal_with_tolerance, normalize_angle};
use crate::pair_holder::PairHolder;
// use crate::simd::F32x8;
// use crate::simd::I32x8;
// use crate::simd::Mx8;
use crate::consts::factor;
use crate::is_strict_mode;
use crate::types::Edge;
use crate::types::Minutia;
use crate::types::Pair;

/*
#[inline(always)]
fn are_angles_not_equal_with_tolerance_2v8(a: I32x8, b: I32x8, c: I32x8, d: I32x8) -> Mx8 {
    let lower = I32x8::splat(ANGLE_LOWER_BOUND);
    let upper = I32x8::splat(ANGLE_UPPER_BOUND);

    let difference1 = I32x8::sub(a, b).abs();
    let difference2 = I32x8::sub(c, d).abs();

    Mx8::or(
        Mx8::and(
            I32x8::gt(difference1, lower),
            I32x8::gt(upper, difference1),
        ),
        Mx8::and(
            I32x8::gt(difference2, lower),
            I32x8::gt(upper, difference2),
        ),
    )
}*/

pub trait CalculatePoints = Fn(
    /*probe_k: */ &Minutia,
    /*probe_j:*/ &Minutia,
    /*gallery_k:*/ &Minutia,
    /*gallery_j:*/ &Minutia,
) -> u32;

#[inline(always)]
pub fn match_edges_into_pairs(
    probe_edges: &[Edge],
    probe_minutiae: &[Minutia],
    gallery_edges: &[Edge],
    // gallery_edges_soa: &EdgeHolder,
    gallery_minutiae: &[Minutia],
    pairs: &mut PairHolder,
    calculate_points: impl CalculatePoints,
) {
    if probe_edges.is_empty() || gallery_edges.is_empty() {
        return;
    }

    // if false  && is_x86_feature_detected!("avx2") && is_x86_feature_detected!("avx") {
    //     unsafe { simd_match_edges_into_pairs(probe_edges, probe_minutiae, gallery_edges_soa, gallery_minutiae, pairs, calculate_points) }
    // } else {
    scalar_match_edges_into_pairs(
        probe_edges,
        probe_minutiae,
        gallery_edges,
        gallery_minutiae,
        pairs,
        calculate_points,
    )
    // }
}

#[allow(unused)]
pub fn scalar_match_edges_into_pairs(
    probe_edges: &[Edge],
    probe_minutiae: &[Minutia],
    gallery_edges: &[Edge],
    gallery_minutiae: &[Minutia],
    pairs: &mut PairHolder,
    calculate_points: impl CalculatePoints,
) {
    debug_assert!(!probe_edges.is_empty());
    debug_assert!(!gallery_edges.is_empty());

    let mut start = 0;

    let probe_edges = if is_strict_mode() {
        &probe_edges[..probe_edges.len() - 1]
    } else {
        &probe_edges[..]
    };

    for probe in probe_edges {
        for (j, gallery) in gallery_edges.iter().enumerate().skip(start) {
            let dz = gallery.distance_squared - probe.distance_squared;
            let fi = 2.0 * factor() * (gallery.distance_squared + probe.distance_squared) as f32;
            if dz.abs() as f32 > fi {
                if dz < 0 {
                    start = j + 1;
                    continue;
                } else {
                    break;
                }
            }

            if !(are_angles_equal_with_tolerance(probe.min_beta, gallery.min_beta)
                && are_angles_equal_with_tolerance(probe.max_beta, gallery.max_beta))
            {
                continue;
            }

            let mut delta_theta = probe.theta_kj - gallery.theta_kj;
            if probe.beta_order != gallery.beta_order {
                delta_theta -= 180;
            }

            pairs.push(Pair {
                delta_theta: normalize_angle(delta_theta),
                probe_k: probe.endpoint_k,
                probe_j: probe.endpoint_j,
                gallery_k: if probe.beta_order == gallery.beta_order {
                    gallery.endpoint_k
                } else {
                    gallery.endpoint_j
                },
                gallery_j: if probe.beta_order == gallery.beta_order {
                    gallery.endpoint_j
                } else {
                    gallery.endpoint_k
                },
                points: calculate_points(
                    &probe_minutiae[probe.endpoint_k.as_usize()],
                    &probe_minutiae[probe.endpoint_j.as_usize()],
                    &gallery_minutiae[gallery.endpoint_k.as_usize()],
                    &gallery_minutiae[gallery.endpoint_j.as_usize()],
                ),
            });
        }
    }
}

/*
#[target_feature(enable = "avx2")]
#[target_feature(enable = "avx")]
#[inline(never)]
pub unsafe fn simd_match_edges_into_pairs(
    probe_edges: &[Edge],
    probe_minutiae: &[Minutia],
    gallery_edges: &[Edge],
    gallery_minutiae: &[Minutia],
    pairs: &mut PairHolder,
    calculate_points: impl CalculatePoints,
) {
    debug_assert!(!probe_edges.is_empty());
    debug_assert!(!gallery_edges.is_empty());

    let factor = F32x8::splat(2.0 * FACTOR);

    let mut start = 0;
    'main: for probe in probe_edges.iter().take(probe_edges.len() - 1) {
        let p_distance_squared = I32x8::splat(probe.distance_squared);
        let p_min_beta = I32x8::splat(probe.min_beta);
        let p_max_beta = I32x8::splat(probe.max_beta);
        let p_theta_kj = I32x8::splat(probe.theta_kj);

        let mut j = start;
        while j + 8 < gallery_edges.len() {
            let v_g_distance_squared = I32x8::from_raw(gallery_edges.distance_squared().get_unchecked(j..j + 8));
            let v_g_min_beta = I32x8::from_raw(gallery_edges.min_beta().get_unchecked(j..j + 8));
            let v_g_max_beta = I32x8::from_raw(gallery_edges.max_beta().get_unchecked(j..j + 8));
            let v_g_theta_kj = I32x8::from_raw(gallery_edges.theta_kj().get_unchecked(j..j + 8));

            let v_dz = I32x8::sub(v_g_distance_squared, p_distance_squared);
            let v_fi = F32x8::mul(factor, I32x8::add(v_g_distance_squared, p_distance_squared).to_f32x8());
            let v_cmp = F32x8::gt(v_dz.abs().to_f32x8(), v_fi);

            let zero = I32x8::splat(0);
            let neg = I32x8::gt(zero, v_dz);
            let neg_neg = I32x8::gt(v_dz, zero);

            if Mx8::and(v_cmp, neg).is_all_set() {
                j += 8;
                start = j;
                continue;
            }

            if Mx8::and(v_cmp, neg_neg).v0() {
                continue 'main;
            }

            let not_within_tolerance = are_angles_not_equal_with_tolerance_2v8(p_min_beta, v_g_min_beta, p_max_beta, v_g_max_beta);

            let v_g_beta_order = gallery_edges.beta_order().get_unchecked(j..j + 8);
            let v_g_endpoint_k = gallery_edges.endpoint_k().get_unchecked(j..j + 8);
            let v_g_endpoint_j = gallery_edges.endpoint_j().get_unchecked(j..j + 8);

            let is_valid = Mx8::or(v_cmp, not_within_tolerance);
            let is_valid_b = is_valid.to_bools();
            let dt_i = I32x8::sub(p_theta_kj, v_g_theta_kj).into_i32();

            for i in 0..8 {
                if is_valid_b[i] {
                    continue;
                }

                let mut delta_theta = dt_i[i];
                if probe.beta_order != v_g_beta_order[i] {
                    delta_theta -= 180;
                }

                pairs.push(Pair {
                    delta_theta: normalize_angle(delta_theta),
                    probe_k: probe.endpoint_k,
                    probe_j: probe.endpoint_j,
                    gallery_k: if probe.beta_order == v_g_beta_order[i] { v_g_endpoint_k[i] } else { v_g_endpoint_j[i] },
                    gallery_j: if probe.beta_order == v_g_beta_order[i] { v_g_endpoint_j[i] } else { v_g_endpoint_k[i] },
                    points: calculate_points(
                        &probe_minutiae[probe.endpoint_k.as_usize()],
                        &probe_minutiae[probe.endpoint_j.as_usize()],
                        &gallery_minutiae[v_g_endpoint_k[i].as_usize()],
                        &gallery_minutiae[v_g_endpoint_j[i].as_usize()],
                    ),
                });
            }

            j += 8;
        }

        while j < gallery_edges.len() {
            let gallery = gallery_edges.get_unchecked(j);

            let dz = gallery.distance_squared - probe.distance_squared;
            let fi = 2.0 * FACTOR * (gallery.distance_squared + probe.distance_squared) as f32;
            if dz.abs() as f32 > fi {
                if dz < 0 {
                    start = j + 1;
                    j += 1;
                    continue;
                } else {
                    break;
                }
            }

            if !(are_angles_equal_with_tolerance(probe.min_beta, gallery.min_beta) &&
                are_angles_equal_with_tolerance(probe.max_beta, gallery.max_beta)) {
                j += 1;
                continue;
            }

            let mut delta_theta = probe.theta_kj - gallery.theta_kj;
            if probe.beta_order != gallery.beta_order {
                delta_theta -= 180;
            }

            pairs.push(Pair {
                delta_theta: normalize_angle(delta_theta),
                probe_k: probe.endpoint_k,
                probe_j: probe.endpoint_j,
                gallery_k: if probe.beta_order == gallery.beta_order { gallery.endpoint_k } else { gallery.endpoint_j },
                gallery_j: if probe.beta_order == gallery.beta_order { gallery.endpoint_j } else { gallery.endpoint_k },
                points: calculate_points(
                    &probe_minutiae[probe.endpoint_k.as_usize()],
                    &probe_minutiae[probe.endpoint_j.as_usize()],
                    &gallery_minutiae[gallery.endpoint_k.as_usize()],
                    &gallery_minutiae[gallery.endpoint_j.as_usize()],
                ),
            });

            j += 1;
        }
    }
}
*/
