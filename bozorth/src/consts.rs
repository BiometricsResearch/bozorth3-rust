use std::sync::atomic::{AtomicI32, AtomicU32, AtomicUsize, Ordering};

/*pub(crate)*/
static MAX_MINUTIA_DISTANCE: AtomicI32 = AtomicI32::new(125);
/*pub(crate)*/
static MAX_MINUTIA_DISTANCE_SQUARED: AtomicI32 = AtomicI32::new(75i32.pow(2));
/*pub(crate)*/
static MIN_NUMBER_OF_PAIRS_TO_BUILD_CLUSTER: AtomicUsize = AtomicUsize::new(3);
/*pub(crate)*/
static MAX_NUMBER_OF_CLUSTERS: AtomicUsize = AtomicUsize::new(2000);
/*pub(crate)*/
static SCORE_THRESHOLD: AtomicU32 = AtomicU32::new(8);
/*pub(crate)*/
static ANGLE_LOWER_BOUND: AtomicI32 = AtomicI32::new(11);
/*pub(crate)*/
static ANGLE_UPPER_BOUND: AtomicI32 = AtomicI32::new(360 - 11);
/*pub(crate)*/
static MAX_NUMBER_OF_GROUPS: AtomicUsize = AtomicUsize::new(10);
/*pub(crate)*/
static FACTOR: AtomicU32 = AtomicU32::new(0.05f32.to_bits());

pub(crate) const MAX_FILE_MINUTIAE: usize = 1000;
pub(crate) const MAX_NUMBER_OF_PAIRS: usize = 20000;
pub(crate) const MAX_NUMBER_OF_MINUTIAE: usize = 200;
pub(crate) const MIN_NUMBER_OF_EDGES: usize = 500;
pub(crate) const MAX_NUMBER_OF_EDGES: usize = 20000;

pub fn max_minutia_distance() -> i32 {
    MAX_MINUTIA_DISTANCE.load(Ordering::Relaxed)
}

pub fn set_max_minutia_distance(n: i32) {
    MAX_MINUTIA_DISTANCE.store(n, Ordering::SeqCst)
}

pub fn max_minutia_distance_squared() -> i32 {
    MAX_MINUTIA_DISTANCE_SQUARED.load(Ordering::Relaxed)
}

pub fn min_number_of_pairs_to_build_cluster() -> usize {
    MIN_NUMBER_OF_PAIRS_TO_BUILD_CLUSTER.load(Ordering::Relaxed)
}

pub fn set_min_number_of_pairs_to_build_cluster(n: usize) {
    MIN_NUMBER_OF_PAIRS_TO_BUILD_CLUSTER.store(n, Ordering::SeqCst)
}

pub fn max_number_of_clusters() -> usize {
    MAX_NUMBER_OF_CLUSTERS.load(Ordering::Relaxed)
}

pub fn set_max_number_of_clusters(n: usize) {
    MAX_NUMBER_OF_CLUSTERS.store(n, Ordering::SeqCst);
}

pub fn score_threshold() -> u32 {
    SCORE_THRESHOLD.load(Ordering::Relaxed)
}

pub fn angle_lower_bound() -> i32 {
    ANGLE_LOWER_BOUND.load(Ordering::Relaxed)
}

pub fn angle_upper_bound() -> i32 {
    ANGLE_UPPER_BOUND.load(Ordering::Relaxed)
}

pub fn set_angle_diff(n: i32) {
    ANGLE_LOWER_BOUND.store(n, Ordering::SeqCst);
    ANGLE_UPPER_BOUND.store(360 - n, Ordering::SeqCst);
}

pub fn max_number_of_groups() -> usize {
    MAX_NUMBER_OF_GROUPS.load(Ordering::Relaxed)
}

pub fn set_max_number_of_groups(n: usize) {
    MAX_NUMBER_OF_GROUPS.store(n, Ordering::Relaxed);
}

pub fn factor() -> f32 {
    f32::from_bits(FACTOR.load(Ordering::Relaxed))
}

pub fn set_factor(x: f32) {
    FACTOR.store(x.to_bits(), Ordering::SeqCst)
}
