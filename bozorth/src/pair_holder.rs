use std::ops::Range;

use crate::consts::MAX_NUMBER_OF_MINUTIAE;
use crate::consts::MAX_NUMBER_OF_PAIRS;
use crate::types::Endpoint;
use crate::{timeit, Pair};

#[derive(Clone)]
struct SmallOptionalRange {
    start: u32,
    end: u32,
}

const MARKER_EMPTY: u32 = u32::max_value();

impl SmallOptionalRange {
    #[inline]
    const fn new(start: u32, end: u32) -> Self {
        SmallOptionalRange { start, end }
    }

    #[inline]
    const fn empty() -> Self {
        SmallOptionalRange {
            start: MARKER_EMPTY,
            end: MARKER_EMPTY,
        }
    }

    #[inline]
    fn as_range(&self) -> Option<Range<usize>> {
        if !(self.start == MARKER_EMPTY && self.end == MARKER_EMPTY) {
            Some(self.start as usize..self.end as usize)
        } else {
            None
        }
    }
}

pub struct PairHolder {
    forward: Vec<Pair>,
    forward_ranges: Vec<SmallOptionalRange>,
    backward: Vec<u32>,
    backward_ranges: Vec<SmallOptionalRange>,
    dirty: bool,
}

impl PairHolder {
    pub fn new() -> Self {
        PairHolder {
            forward: Vec::with_capacity(MAX_NUMBER_OF_PAIRS),
            forward_ranges: vec![
                SmallOptionalRange::empty();
                MAX_NUMBER_OF_MINUTIAE * MAX_NUMBER_OF_MINUTIAE
            ],
            backward: Vec::with_capacity(MAX_NUMBER_OF_PAIRS),
            backward_ranges: vec![
                SmallOptionalRange::empty();
                MAX_NUMBER_OF_MINUTIAE * MAX_NUMBER_OF_MINUTIAE
            ],
            dirty: false,
        }
    }

    #[inline]
    pub(crate) fn iter<'a>(&'a self) -> impl Iterator<Item = &'a Pair> + 'a {
        self.forward.iter()
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.forward.is_empty()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.forward.len()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.forward.clear();
        self.backward.clear();

        self.forward_ranges.iter_mut().for_each(|it| {
            *it = SmallOptionalRange::empty();
        });
        self.backward_ranges.iter_mut().for_each(|it| {
            *it = SmallOptionalRange::empty();
        });

        self.dirty = false;
    }

    #[inline]
    pub(crate) fn push(&mut self, pair: Pair) {
        self.forward.push(pair);
        self.dirty = true;
    }

    pub fn prepare(&mut self) {
        if !self.dirty {
            return;
        }

        timeit(|| {
            self.forward
                .sort_by_key(|pair| (pair.probe_k, pair.gallery_k, pair.probe_j));
        });
        timeit(|| self.backward.clear());
        timeit(|| {
            for index in 0..self.forward.len() {
                self.backward.push(index as u32);
            }
        });
        timeit(|| {
            self.backward.sort_by_key({
                let forward = &self.forward;
                move |&index| {
                    let index = index as usize;
                    let pair = &forward[index];
                    (pair.probe_j, pair.gallery_j)
                }
            });
        });
        timeit(|| {
            make_range_cache(&self.forward, &mut self.forward_ranges, |pair| {
                (pair.probe_k.as_usize() * MAX_NUMBER_OF_MINUTIAE) + pair.gallery_k.as_usize()
            });
        });
        timeit(|| {
            make_range_cache(&self.backward, &mut self.backward_ranges, {
                let forward = &self.forward;
                move |&index| {
                    let pair = &forward[index as usize];
                    (pair.probe_j.as_usize() * MAX_NUMBER_OF_MINUTIAE) + pair.gallery_j.as_usize()
                }
            });
        });
        self.dirty = false;
    }

    pub fn pairs(&self) -> &[Pair] {
        self.forward.as_slice()
    }

    #[inline]
    pub fn find_pairs_by_first_endpoint(
        &self,
        offset: usize,
        probe_endpoint: Endpoint,
        gallery_endpoint: Endpoint,
    ) -> (
        impl Iterator<Item = (usize, Endpoint, Endpoint)> + '_,
        usize,
    ) {
        debug_assert!(!self.dirty);

        let endpoint_offset =
            (probe_endpoint.as_usize() * MAX_NUMBER_OF_MINUTIAE) + gallery_endpoint.as_usize();
        let range = self.forward_ranges[endpoint_offset]
            .as_range()
            .unwrap_or(offset..offset);
        let range = left_trim_range(range, offset);
        let iterator = range
            .clone()
            .zip(self.forward[range.clone()].iter())
            .map(|(index, pair)| (index, pair.probe_j, pair.gallery_j));

        (iterator, range.end)
    }

    #[inline]
    pub fn find_pairs_by_second_endpoint(
        &self,
        offset: usize,
        probe_endpoint: Endpoint,
        gallery_endpoint: Endpoint,
    ) -> (
        impl Iterator<Item = (usize, Endpoint, Endpoint)> + '_,
        usize,
    ) {
        debug_assert!(!self.dirty);

        let range = self.backward_ranges
            [(probe_endpoint.as_usize() * MAX_NUMBER_OF_MINUTIAE) + gallery_endpoint.as_usize()]
        .as_range()
        .unwrap_or(offset..offset);
        let iterator = self.backward[range.clone()]
            .iter()
            .skip_while(move |&it| *it < offset as u32)
            .map(move |&it| {
                let index = it as usize;
                let pair = self.forward[index];
                (index, pair.probe_k, pair.gallery_k)
            });
        (iterator, range.end)
    }

    #[inline]
    pub fn get(&self, index: usize) -> &Pair {
        &self.forward[index]
    }
}

#[inline]
fn make_range_cache<T, F>(slice: &[T], ranges: &mut [SmallOptionalRange], extractor: F)
where
    F: Fn(&T) -> usize,
{
    let mut previous = None;
    let mut range_start = 0;
    for (i, item) in slice.iter().enumerate() {
        let current = extractor(item);
        if let Some(index) = previous {
            if index != current {
                ranges[index] = SmallOptionalRange::new(range_start as u32, i as u32);
                previous = Some(current);
                range_start = i;
            }
        } else {
            previous = Some(current);
        }
    }

    if let Some(index) = previous {
        ranges[index] = SmallOptionalRange::new(range_start as u32, slice.len() as u32);
    }
}

#[inline]
fn left_trim_range(range: Range<usize>, offset: usize) -> Range<usize> {
    if offset >= range.start && offset < range.end {
        Range {
            start: offset,
            end: range.end,
        }
    } else if offset >= range.end {
        Range {
            start: range.end,
            end: range.end,
        }
    } else {
        range
    }
}
