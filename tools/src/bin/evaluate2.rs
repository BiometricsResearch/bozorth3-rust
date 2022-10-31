use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use argh::FromArgs;

use bozorth::{
    find_edges, limit_edges, match_edges_into_pairs, match_score, parse, prune, set_mode,
    BozorthState, Edge, Format, Minutia, PairHolder,
};

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
    /// use strict mode
    #[argh(switch, short = 'j')]
    strict: bool,

    /// path to xyt files
    #[argh(option)]
    xyt_path: PathBuf,

    /// points for incompatible minutia types
    #[argh(option, short = '0')]
    points0: u32,

    /// points for one compatible minutiae type
    #[argh(option, short = '1')]
    points1: u32,

    /// points for compatible minutia types
    #[argh(option, short = '2')]
    points2: u32,

    /// max threshold
    #[argh(option)]
    max_threshold: u32,

    /// output file name
    #[argh(option)]
    name: String,

    /// used worker threads
    #[argh(option)]
    threads: u32,

    /// output directory
    #[argh(option)]
    output: PathBuf,
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
    println!("{:#?}", &opts);

    let mut output_file_txt = opts.output.clone();
    output_file_txt.push(&format!("{}.txt", opts.name));

    let mut output_file_csv = opts.output.clone();
    output_file_csv.push(&format!("{}.csv", opts.name));

    if output_file_csv.exists() || output_file_txt.exists() {
        println!("Files already exist.");
        return Ok(());
    }

    let mut files_by_finger: HashMap<_, Vec<_>> = HashMap::new();
    let mut cache = HashMap::new();

    for path in std::fs::read_dir(&opts.xyt_path)? {
        let raw_path = path?.path();
        let name = raw_path
            .file_name()
            .context("no file name")?
            .to_str()
            .context("not utf8")?;
        if !name.ends_with(".jpg.xyt") {
            continue;
        }

        let (finger, _) = name.rsplit_once('_').unwrap();
        files_by_finger
            .entry(finger.to_owned())
            .or_default()
            .push(raw_path.clone());
        let fingerprint = parse_fingerprint(&raw_path);
        cache.insert(raw_path, fingerprint);
    }

    dbg!(cache.len());

    println!("Loaded data into the cache!");

    let results = crossbeam::scope(|s| {
        let (tx_pairs, rx_pairs) = crossbeam::channel::bounded::<(&PathBuf, &PathBuf, bool)>(1000);
        let (tx_scores, rx_scores) = crossbeam::channel::bounded(1000);

        let files_by_finger = &files_by_finger;
        s.spawn(move |_| {
            for (first, first_finger) in files_by_finger
                .iter()
                .flat_map(|(finger, files)| files.iter().map(move |it| (finger, it)))
            {
                for (second, second_finger) in files_by_finger
                    .iter()
                    .flat_map(|(finger, files)| files.iter().map(move |it| (finger, it)))
                {
                    let first_kind = first_finger.file_name().unwrap().to_str().unwrap()
                        [first.len()..]
                        .split_once('.')
                        .unwrap()
                        .0;
                    let second_kind = second_finger.file_name().unwrap().to_str().unwrap()
                        [second.len()..]
                        .split_once('.')
                        .unwrap()
                        .0;

                    if first_kind == "_n" && second_kind != "_n" {
                        tx_pairs
                            .send((first_finger, second_finger, first == second))
                            .unwrap();
                    }
                }
            }
        });

        for _ in 0..opts.threads {
            let rx_pairs = rx_pairs.clone();
            let tx_scores = tx_scores.clone();
            let cache = &cache;
            let opts = &opts;
            s.spawn(move |_| {
                let mut state = BozorthState::new();
                let mut cacher = PairHolder::new();

                for (first_finger, second_finger, should_match) in rx_pairs {
                    let score = match_files(
                        &cache[first_finger],
                        &cache[second_finger],
                        opts,
                        &mut state,
                        &mut cacher,
                    );
                    // match score {
                    //     Ok(score) => {
                    tx_scores.send((score, should_match)).unwrap();
                    // },
                    // Err(e) => println!(
                    //     "error while matching: {} vs {} with {:?}",
                    //     first_finger.display(),
                    //     second_finger.display(),
                    //     e
                    // ),
                    // }
                }
            });
        }

        // Drop channels that we've cloned into the workers since we don't need them any more
        // and they are blocking the last thread
        drop(rx_pairs);
        drop(tx_scores);

        let opts = &opts;
        let total = files_by_finger
            .values()
            .map(|it| it.len() as u32)
            .sum::<u32>()
            .pow(2);

        let results = s
            .spawn(move |_| {
                let threshold = opts.max_threshold as usize;
                let mut results = Results {
                    true_positive: vec![0; threshold + 1],
                    false_positive: vec![0; threshold + 1],
                    true_negative: vec![0; threshold + 1],
                    false_negative: vec![0; threshold + 1],
                };

                let start = std::time::Instant::now();
                let mut done = 0;
                for (score, can_match) in rx_scores {
                    for threshold in 0..=threshold {
                        let matches = score as usize >= threshold;
                        match (can_match, matches) {
                            (true, true) => results.true_positive[threshold] += 1,
                            (false, true) => results.false_positive[threshold] += 1,
                            (false, false) => results.true_negative[threshold] += 1,
                            (true, false) => results.false_negative[threshold] += 1,
                        }
                    }
                    done += 1;

                    if done % 10000 == 0 {
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
    writeln!(f, "{:#?}", &opts).unwrap();

    Ok(())
}

// C:\Users\Host\Downloads\NISTSpecialDatabase4GrayScaleImagesofFIGS\sd04\png_txt\all
