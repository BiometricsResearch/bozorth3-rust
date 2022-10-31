use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use argh::FromArgs;

use bozorth::consts::{
    set_angle_diff, set_factor, set_max_minutia_distance, set_max_number_of_clusters,
    set_max_number_of_groups, set_min_number_of_pairs_to_build_cluster,
};
use bozorth::{
    find_edges, limit_edges, match_edges_into_pairs, match_score, parse, prune, set_mode,
    BozorthState, Edge, Format, Minutia, PairHolder,
};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

fn parse_fingerprint(file: impl AsRef<Path>) -> Fingerprint {
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

struct Fingerprint {
    minutiae: Box<[Minutia]>,
    edges: Box<[Edge]>,
}

fn match_files(
    first: &Fingerprint,
    second: &Fingerprint,
    options: &Options,
    state: &mut BozorthState,
    cacher: &mut PairHolder,
) -> u32 {
    cacher.clear();
    match_edges_into_pairs(
        &first.edges,
        &first.minutiae,
        &second.edges,
        &second.minutiae,
        cacher,
        |pk: &Minutia, pj: &Minutia, gk: &Minutia, gj: &Minutia| match (
            pk.kind == gk.kind,
            pj.kind == gj.kind,
        ) {
            (true, true) => options.points2,
            (true, false) | (false, true) => options.points1,
            (false, false) => options.points0,
        },
    );
    cacher.prepare();

    state.clear();
    match_score(
        &cacher,
        &first.minutiae,
        &second.minutiae,
        Format::Ansi,
        state,
    )
    .unwrap_or_default()
    .0 as u32
}

/// Benchmark specified algorithm version
#[derive(FromArgs, Debug)]
struct Options {
    /// use original version of Bozorth3
    #[argh(switch, short = 's')]
    strict: bool,

    /// path to directory with input .xyt and .min files
    #[argh(option, short = 'i')]
    input: PathBuf,

    /// points for no compatible minutia type
    #[argh(option, short = '0')]
    points0: u32,

    /// points for one compatible minutia type
    #[argh(option, short = '1')]
    points1: u32,

    /// points for two compatible minutiae types
    #[argh(option, short = '2')]
    points2: u32,

    /// max threshold
    #[argh(option, short = 't')]
    max_threshold: u32,

    /// name of output files
    #[argh(option, short = 'n')]
    name: String,

    /// directory to save output files
    #[argh(option, short = 'o')]
    output: PathBuf,

    /// number of worker threads
    #[argh(option, short = 'm')]
    threads: u32,

    /// normalize score
    #[argh(switch)]
    normalize: bool,

    /// max score when normalization is enabled (default: 1)
    #[argh(option, default = "1")]
    max_score: u32,

    /// max number of clusters to create (default: 2000)
    #[argh(option, default = "2000")]
    max_clusters: u32,

    /// min cluster size (default: 3)
    #[argh(option, default = "3")]
    min_cluster_size: u32,

    /// max number of groups (default: 10)
    #[argh(option, default = "10")]
    max_groups: u32,

    /// angle comparison tolerance (default: 11)
    #[argh(option, default = "11")]
    angle_tolerance: u32,

    /// max distance (default: 125)
    #[argh(option, default = "125")]
    max_distance: u32,

    /// factor (default: 0.05)
    #[argh(option, default = "0.05")]
    factor: f32,
}

struct Results {
    true_positive: Vec<usize>,
    false_positive: Vec<usize>,
    true_negative: Vec<usize>,
    false_negative: Vec<usize>,
}

fn main() -> Result<(), anyhow::Error> {
    let opts: Options = argh::from_env();
    set_mode(opts.strict);
    set_max_number_of_clusters(opts.max_clusters as usize);
    set_max_number_of_groups(opts.max_groups as usize);
    set_angle_diff(opts.angle_tolerance as i32);
    set_max_minutia_distance(opts.max_distance as i32);
    set_factor(opts.factor);
    set_min_number_of_pairs_to_build_cluster(opts.min_cluster_size as usize);
    println!("{:#?}", &opts);

    if !opts.output.exists() {
        std::fs::create_dir_all(&opts.output).unwrap();
        println!("Created directory {}", opts.output.display());
    }

    let mut output_file_txt = opts.output.clone();
    output_file_txt.push(&format!("{}.txt", opts.name));

    let mut output_file_csv = opts.output.clone();
    output_file_csv.push(&format!("{}.csv", opts.name));

    if output_file_csv.exists() || output_file_txt.exists() {
        println!("Files already exist.");
        return Ok(());
    }

    let mut files_first = vec![];
    let mut files_second = vec![];
    let mut cache = HashMap::new();

    for path in std::fs::read_dir(&opts.input)? {
        let raw_path = path?.path();
        let name = raw_path
            .file_name()
            .context("no file name")?
            .to_str()
            .context("not utf8")?;
        if !name.ends_with(".png.xyt") {
            continue;
        }

        if name.starts_with("f") {
            files_first.push(raw_path.clone());
        } else if name.starts_with("s") {
            files_second.push(raw_path.clone());
        }

        let fingerprint = parse_fingerprint(&raw_path);
        cache.insert(raw_path, fingerprint);
    }

    println!("Loaded data into the cache!");

    let max_scores: HashMap<&Path, u32> = if opts.normalize {
        let scores = cache
            .par_iter()
            .map(|(path, fp)| {
                let mut state = BozorthState::new();
                let mut cacher = PairHolder::new();
                let score = match_files(fp, &fp, &opts, &mut state, &mut cacher);
                (path.as_path(), score)
            })
            .collect();
        println!("Calculated max scores!");
        scores
    } else {
        HashMap::new()
    };

    let start = std::time::Instant::now();
    let results = crossbeam::scope(|s| {
        let (tx_pairs, rx_pairs) = crossbeam::channel::bounded::<(&PathBuf, &PathBuf)>(1000);
        let (tx_scores, rx_scores) = crossbeam::channel::bounded(1000);

        let files_first = &files_first[..];
        let files_second = &files_second[..];

        s.spawn(move |_| {
            for first_finger in files_first.iter() {
                for second_finger in files_second {
                    tx_pairs.send((first_finger, second_finger)).unwrap();
                }
            }
        });

        for _ in 0..opts.threads {
            let rx_pairs = rx_pairs.clone();
            let tx_scores = tx_scores.clone();
            let cache = &cache;
            let max_points = &max_scores;
            let opts = &opts;
            s.spawn(move |_| {
                let mut state = BozorthState::new();
                let mut cacher = PairHolder::new();

                for (first_finger, second_finger) in rx_pairs {
                    let should_match = first_finger.file_name().unwrap().to_str().unwrap()[1..]
                        == second_finger.file_name().unwrap().to_str().unwrap()[1..];

                    let score = match_files(
                        &cache[first_finger],
                        &cache[second_finger],
                        opts,
                        &mut state,
                        &mut cacher,
                    );

                    let score = if opts.normalize {
                        let total_score = std::cmp::min(
                            max_points[first_finger.as_path()],
                            max_points[second_finger.as_path()],
                        );

                        let normalized_score = (score as f32) / (total_score as f32);
                        (normalized_score * opts.max_score as f32).round() as u32
                    } else {
                        score
                    };

                    tx_scores.send((score, should_match)).unwrap();
                }
            });
        }

        // Drop channels that we've cloned into the workers since we don't need them any more
        // and they are blocking the last thread
        drop(rx_pairs);
        drop(tx_scores);

        let opts = &opts;
        let results = s
            .spawn(move |_| {
                let threshold = opts.max_threshold as usize;
                let mut results = Results {
                    true_positive: vec![0; threshold + 1],
                    false_positive: vec![0; threshold + 1],
                    true_negative: vec![0; threshold + 1],
                    false_negative: vec![0; threshold + 1],
                };

                let mut done = 0;
                for (score, should_match) in rx_scores {
                    for threshold in 0..=threshold {
                        let matches = score as usize >= threshold;
                        match (should_match, matches) {
                            (true, true) => results.true_positive[threshold] += 1,
                            (false, true) => results.false_positive[threshold] += 1,
                            (false, false) => results.true_negative[threshold] += 1,
                            (true, false) => results.false_negative[threshold] += 1,
                        }
                    }
                    done += 1;

                    if done % 10000 == 0 {
                        let total = files_first.len() * files_second.len();
                        eprintln!(
                            "{}/{} -- {:.02}% in {:.03}s",
                            done,
                            total,
                            (done as f32 / total as f32 * 100.0),
                            start.elapsed().as_secs_f64()
                        );
                    }
                }
                eprintln!("Done in {:?}", start.elapsed());
                results
            })
            .join()
            .unwrap();

        results
    })
    .unwrap();

    let mut f = std::fs::File::create(&output_file_csv).unwrap();
    writeln!(f, "thres\ttp\tfn\ttn\tfp").unwrap();
    for i in 0..=opts.max_threshold as usize {
        writeln!(
            f,
            "{}\t{}\t{}\t{}\t{}",
            i,
            results.true_positive[i],
            results.false_negative[i],
            results.true_negative[i],
            results.false_positive[i],
        )
        .unwrap();
    }

    let mut f = std::fs::File::create(&output_file_txt).unwrap();
    writeln!(f, "{:#?}\n", &opts).unwrap();
    writeln!(f, "time: {:?}", start.elapsed()).unwrap();

    Ok(())
}
