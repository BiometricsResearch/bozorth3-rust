use std::collections::HashMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rayon::iter::ParallelIterator;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator};

use bozorth::{
    find_edges, limit_edges, match_edges_into_pairs, match_score, parse, prune, set_mode, timeit,
    BozorthState, Edge, Format, Minutia, PairHolder,
};

struct Fingerprint {
    minutiae: Box<[Minutia]>,
    edges: Box<[Edge]>,
}

fn extract_edges(file: impl AsRef<Path>) -> Fingerprint {
    let minutiae = prune(&parse(file).unwrap(), 150);
    let mut edges = vec![];
    find_edges(&minutiae, &mut edges, Format::NistInternal);
    let limit = limit_edges(&edges);

    edges.truncate(limit);
    Fingerprint {
        minutiae: minutiae.into_boxed_slice(),
        edges: edges.into_boxed_slice(),
    }
}

fn iter_lines<P>(path: P) -> impl Iterator<Item = String>
where
    P: AsRef<Path>,
{
    let f = std::fs::File::open(path).unwrap();
    let r = std::io::BufReader::new(f);
    r.lines().flat_map(|it| it.ok())
}

struct MatchResult {
    first: u32,
    second: u32,
    expected: u32,
    actual: u32,
}

fn main() {
    set_mode(true);

    let no_check = std::env::args().any(|arg| arg == "no_check");
    let no_parallel = std::env::args().any(|arg| arg == "no-parallel");

    let (expected_path, xyt_path) = if cfg!(target_os = "windows") {
        (r"C:\Users\Host\Documents/all", r"E:/xxxx/backup/xyt")
    } else {
        (
            r"/mnt/c/Projects/grbg/cmake-build-release/all",
            r"/mnt/e/xxxx/backup/xyt",
        )
    };

    let paths: Arc<[PathBuf]> = std::fs::read_dir(xyt_path)
        .unwrap()
        .map(|it| it.unwrap().path())
        .filter_map(|it| {
            let ext = it.extension()?;
            if ext == "xyt" {
                Some(it)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .into();

    let cache: HashMap<_, Fingerprint> = paths
        .par_iter()
        .map(|path| {
            let fp = extract_edges(&path);
            (path.to_owned(), fp)
        })
        .collect();

    let expected: Vec<u32> = if no_check {
        Vec::new()
    } else {
        iter_lines(expected_path)
            .map(|line| parse_line(&line).expect("invalid line"))
            .collect()
    };

    let (tx, rx) = crossbeam::channel::unbounded::<MatchResult>();

    let paths1 = paths.clone();
    let handle = std::thread::spawn(move || {
        let start = std::time::Instant::now();

        let mut x = 0;
        for item in rx {
            x += 1;

            if item.expected != item.actual {
                println!(
                    "❎ {} {} -> ACTUAL: {} EXPECTED: {}",
                    display(&paths1[item.first as usize]).unwrap(),
                    display(&paths1[item.second as usize]).unwrap(),
                    item.actual,
                    item.expected
                );
            }

            if x % 10000 == 0 {
                println!("{} {:?}", x, start.elapsed());
            }
        }
    });

    let start = std::time::Instant::now();
    let executor = |i: usize| {
        let mut pair_cacher = PairHolder::new();
        let mut state = BozorthState::new();

        (0..paths.len()).into_iter().for_each(|j| {
            let probe_fp = cache.get(&paths[i]).unwrap();
            let gallery_fp = cache.get(&paths[j]).unwrap();

            timeit(|| pair_cacher.clear());
            timeit(|| {
                match_edges_into_pairs(
                    &probe_fp.edges,
                    &probe_fp.minutiae,
                    &gallery_fp.edges,
                    &gallery_fp.minutiae,
                    &mut pair_cacher,
                    |_pk: &Minutia, _pj: &Minutia, _gk: &Minutia, _gj: &Minutia| 1,
                )
            });
            timeit(|| pair_cacher.prepare());

            let actual = timeit(|| {
                match_score(
                    &pair_cacher,
                    &probe_fp.minutiae,
                    &gallery_fp.minutiae,
                    Format::NistInternal,
                    &mut state,
                )
                .unwrap_or_default()
                .0 as u32
            });

            let expected = if no_check {
                actual
            } else {
                expected[i * paths.len() + j]
            };
            if expected != actual {
                println!(
                    "❎ {} {} -> ACTUAL: {} EXPECTED: {}",
                    display(&paths[i]).unwrap(),
                    display(&paths[j]).unwrap(),
                    actual,
                    expected
                );
            }

            tx.send(MatchResult {
                first: i as u32,
                second: j as u32,
                expected,
                actual,
            })
            .unwrap();
        });
    };

    if no_parallel {
        (0..paths.len()).for_each(executor);
    } else {
        (0..paths.len()).into_par_iter().for_each(executor);
    }

    print!("elapsed: {:?}", start.elapsed());
    handle.join().unwrap();
}

fn parse_line(line: &str) -> Result<u32, ()> {
    let idx = line.rfind(' ').ok_or(())?;
    line[idx + 1..].parse().map_err(|_| ())
}

fn display<P>(path: P) -> Option<String>
where
    P: AsRef<Path>,
{
    Some(path.as_ref().file_name()?.to_str()?.to_owned())
}
