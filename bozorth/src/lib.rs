#![allow(incomplete_features)]
#![feature(trait_alias)]
#![feature(const_float_bits_conv)]
// #![feature(const_int_pow)]

pub use bozorth::{match_score, BozorthState};
pub use find_edges::find_edges;
pub use match_edges::match_edges_into_pairs;
pub use pair_holder::PairHolder;
pub use parsing::parse;
pub use prof::timeit;
use std::sync::atomic::{AtomicBool, Ordering};
pub use types::{BetaOrder, Edge, Format, Minutia, Pair};
pub use utils::{limit_edges, prune};

static STRICT_MODE: AtomicBool = AtomicBool::new(true);

#[inline(always)]
pub fn is_strict_mode() -> bool {
    STRICT_MODE.load(Ordering::Relaxed)
}

pub fn set_mode(strict: bool) {
    STRICT_MODE.store(strict, Ordering::SeqCst);
}

mod associations;
mod bozorth;
mod clusters;
pub mod consts;
mod find_edges;
mod groups;
mod match_edges;
mod math;
mod pair_holder;
pub mod parsing;
mod prof;
mod set_intersection;
pub mod types;
mod utils;
mod weird_sort;
