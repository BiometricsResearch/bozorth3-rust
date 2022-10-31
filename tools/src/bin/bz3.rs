#![feature(trait_alias)]

use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Context;
use structopt::StructOpt;

use bozorth::{
    find_edges, limit_edges, match_edges_into_pairs, match_score, parse, prune, BozorthState, Edge,
    Format, Minutia, PairHolder,
};
use rayon::iter::{ParallelBridge, ParallelIterator};

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
enum MatchMode {
    Any,
    OnlyFirstMatch,
    AllMatches,
}

impl FromStr for MatchMode {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "all" | "any" => Ok(MatchMode::Any),
            "first-match" => Ok(MatchMode::OnlyFirstMatch),
            "all-matches" => Ok(MatchMode::AllMatches),
            _ => Err("invalid mode"),
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct Range {
    first: u32,
    last: u32,
}

#[allow(unused)]
impl Range {
    fn first(&self) -> u32 {
        self.first
    }
    fn last(&self) -> u32 {
        self.last
    }
    fn len(&self) -> u32 {
        self.last - self.first + 1
    }
}

impl FromStr for Range {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (first, last) = s.split_once("-").ok_or("no separator")?;
        let first: u32 = first.parse().map_err(|_| "invalid start of range")?;
        let last: u32 = last.parse().map_err(|_| "invalid end of range")?;

        if first >= 1 && first <= last {
            Ok(Range {
                first: first - 1,
                last: last - 1,
            })
        } else {
            Err("invalid order")
        }
    }
}

/// Bozorth3 matcher tool
#[derive(StructOpt, Debug)]

struct Options {
    /// All *.xyt files use representation according to ANSI INCITS 378-2004
    #[structopt(short = "a", long)]
    use_ansi: bool,

    /// Matching mode; supported modes: all, first-match, all-matches
    #[structopt(short = "m", long, default_value = "all")]
    mode: MatchMode,

    /// Set match score threshold
    #[structopt(short = "t", long, default_value = "40")]
    threshold: u32,

    /// Only print the filenames between which match scores would be computed
    #[structopt(short = "d", long)]
    dry_run: bool,

    /// Set maximum number of minutiae to use from any file; allowed range 0-200
    #[structopt(short = "n", long, default_value = "150")]
    max_minutiae: u32,

    /// Number of threads to use
    #[structopt(short = "T", long, default_value = "1")]
    threads: u32,

    /// Size of a chunk in parallel mode
    #[structopt(short = "T", long, default_value = "1000")]
    chunk_size: u32,

    /// File containing list of pairs to compare, one file in each line
    #[structopt(short = "M", long)]
    pair_file: Option<PathBuf>,

    /// File containing list of probe files or directory
    #[structopt(short = "P", long)]
    probe_files: Option<PathBuf>,

    /// File containing list of gallery files or directory
    #[structopt(short = "G", long)]
    gallery_files: Option<PathBuf>,

    /// Single probe file
    #[structopt(short = "p", long)]
    fixed_probe: Option<PathBuf>,

    /// Single gallery file
    #[structopt(short = "g", long)]
    fixed_gallery: Option<PathBuf>,

    /// Subset of files in the probe list to process
    #[structopt(long)]
    probe_range: Option<Range>,

    /// Subset of files in the gallery file to process
    #[structopt(long)]
    gallery_range: Option<Range>,

    /// Print only scores without filenames (applicable only for -m 'all')
    #[structopt(short = "s", long)]
    only_scores: bool,

    /// Do not preserve order; can run slightly faster
    #[structopt(short = "r", long)]
    relaxed_output_order: bool,

    /// Output file
    #[structopt(short = "o", long)]
    output_file: Option<PathBuf>,

    inputs: Vec<PathBuf>,
}

fn find_items_from_pairs(
    file_name: impl AsRef<Path>,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>), anyhow::Error> {
    let file = std::fs::File::open(file_name).context("cannot load pairs from file")?;
    let buff = std::io::BufReader::new(file);

    let mut probes = vec![];
    let mut galleries = vec![];

    for (i, line) in buff.lines().enumerate() {
        let line = line.context("error while reading line")?;
        if i % 2 == 0 {
            probes.push(line.into());
        } else {
            galleries.push(line.into());
        }
    }

    if probes.len() != galleries.len() {
        // td::cerr << "warning: there are " << probes.size() << " probe files and " << galleries.size()
        //                   << " gallery files (these numbers should be equal), skipping last gallery file \n";
        galleries.pop();
    }

    Ok((probes, galleries))
}

fn get_items_from_file(file_name: impl AsRef<Path>) -> Result<Vec<PathBuf>, anyhow::Error> {
    let file = std::fs::File::open(file_name).context("cannot load pairs from file")?;
    let buff = std::io::BufReader::new(file);

    let mut files = vec![];
    for line in buff.lines() {
        let line = line.context("cannot read line")?;
        files.push(line.into());
    }

    Ok(files)
}

fn get_items_from_directory(directory: impl AsRef<Path>) -> Result<Vec<PathBuf>, anyhow::Error> {
    let mut files = vec![];

    for entry in std::fs::read_dir(directory).context("cannot read directory")? {
        let entry = entry.context("cannot read entry")?;
        let meta = entry.metadata().context("cannot read file metadata")?;
        if !meta.is_file() {
            continue;
        }

        if entry.path().extension().and_then(OsStr::to_str) != Some("xyt") {
            continue;
        }

        files.push(entry.path());
    }
    files.sort();
    Ok(files)
}

fn get_items_from_file_or_directory(path: impl AsRef<Path>) -> Result<Vec<PathBuf>, anyhow::Error> {
    if path.as_ref().is_file() {
        get_items_from_file(path)
    } else if path.as_ref().is_dir() {
        get_items_from_directory(path)
    } else {
        if path.as_ref().exists() {
            Err(anyhow::Error::msg("cannot read path"))
        } else {
            Err(anyhow::Error::msg("path does not exist"))
        }
    }
}

fn get_slice_by_range<T>(slice: &[T], range: Range) -> Option<&'_ [T]> {
    if range.first < slice.len() as u32 && range.last <= slice.len() as u32 {
        Some(&slice[range.first as usize..range.len() as usize])
    } else {
        None
    }
}

#[derive(Debug)]
enum CompareMode {
    OneToOne,
    EveryProbeWithEachGallery,
    OneToMany,
}

fn main() -> anyhow::Result<()> {
    let opt: Options = Options::from_args();
    println!("{:?}", opt);

    let mut errors = vec![];
    if opt.max_minutiae > 200 {
        errors.push("invalid number of computable minutaie");
    }

    if opt.pair_file.is_some() && opt.probe_files.is_some() {
        errors.push(r#"flags "-M" and "-P" are incompatible"#)
    }

    if opt.pair_file.is_some() && opt.gallery_files.is_some() {
        errors.push(r#"flags "-M" and "-G" are incompatible"#);
    }

    if opt.pair_file.is_some() && opt.fixed_probe.is_some() {
        errors.push(r#"flags "-M" and "-p" are incompatible"#);
    }

    if opt.pair_file.is_some() && opt.fixed_gallery.is_some() {
        errors.push(r#"flags "-M" and "-g" are incompatible"#);
    }

    if opt.probe_files.is_some() && opt.fixed_probe.is_some() {
        errors.push(r#"flags "-P" and "-p" are incompatible"#);
    }

    if opt.gallery_files.is_some() && opt.fixed_gallery.is_some() {
        errors.push(r#"flags "-G" and "-g" are incompatible"#);
    }

    if opt.mode != MatchMode::Any && opt.pair_file.is_some() {
        errors.push(r#"flag "-M" is not compatible with modes other than "all"#);
    }

    if !errors.is_empty() {
        eprintln!("Parsing errors:");
        for error in errors {
            eprintln!(" - {}", error);
        }
        exit(-1);
    }

    let mode = match opt.mode {
        MatchMode::Any => CompareMode::EveryProbeWithEachGallery,
        _ => CompareMode::OneToMany,
    };

    let (probes, galleries, mode) = if let Some(pair_file) = &opt.pair_file {
        let (probes, galleries) = find_items_from_pairs(pair_file)?;
        (probes, galleries, CompareMode::OneToMany)
    } else if opt.fixed_probe.is_some() && opt.fixed_gallery.is_some() {
        (
            vec![opt.fixed_probe.clone().unwrap()],
            vec![opt.fixed_gallery.clone().unwrap()],
            mode,
        )
    } else if let Some(fixed_probe) = &opt.fixed_probe {
        let probes = vec![fixed_probe.clone()];
        let galleries = if let Some(gallery_files) = &opt.gallery_files {
            get_items_from_directory(gallery_files)?
        } else if !opt.inputs.is_empty() {
            opt.inputs
        } else {
            eprintln!("missing gallery files");
            exit(-1);
        };
        (probes, galleries, mode)
    } else if let Some(fixed_gallery) = &opt.fixed_gallery {
        let galleries = vec![fixed_gallery.clone()];
        let probes = if let Some(probe_files) = &opt.probe_files {
            get_items_from_directory(probe_files)?
        } else if !opt.inputs.is_empty() {
            opt.inputs
        } else {
            eprintln!("missing probe files");
            exit(-1);
        };
        (probes, galleries, mode)
    } else if opt.probe_files.is_some() && opt.gallery_files.is_some() {
        let probes = get_items_from_file_or_directory(opt.probe_files.as_ref().unwrap())?;
        let galleries = get_items_from_file_or_directory(opt.gallery_files.as_ref().unwrap())?;
        (probes, galleries, mode)
    } else if opt.probe_files.is_some() && !opt.inputs.is_empty() {
        let probes = get_items_from_file_or_directory(opt.probe_files.as_ref().unwrap())?;
        let galleries = opt.inputs;
        (probes, galleries, mode)
    } else if opt.gallery_files.is_some() && !opt.inputs.is_empty() {
        let probes = opt.inputs;
        let galleries = get_items_from_file_or_directory(opt.gallery_files.as_ref().unwrap())?;
        (probes, galleries, mode)
    } else if !opt.inputs.is_empty() {
        if opt.inputs.len() % 2 == 1 {
            eprintln!("Number of files to compare is odd");
            exit(-1);
        }

        let mut probes = Vec::with_capacity(opt.inputs.len() / 2);
        let mut galleries = Vec::with_capacity(opt.inputs.len() / 2);

        for (i, path) in opt.inputs.iter().cloned().enumerate() {
            if i % 2 == 0 {
                probes.push(path);
            } else {
                galleries.push(path);
            }
        }
        (probes, galleries, CompareMode::OneToOne)
    } else {
        eprintln!("missing input data");
        exit(-1);
    };

    let probe_range = match opt.probe_range {
        Some(r) => get_slice_by_range(&probes, r).context("out of bounds")?,
        None => &probes,
    };

    let gallery_range = match opt.gallery_range {
        Some(r) => get_slice_by_range(&galleries, r).context("out of bounds")?,
        None => &galleries,
    };

    if opt.dry_run {
        dry_run(probe_range, gallery_range, mode);
    } else {
        let s = std::time::Instant::now();
        run(
            probe_range,
            gallery_range,
            mode,
            &Options {
                inputs: vec![],
                ..opt
            },
        );

        dbg!(s.elapsed());
    }

    Ok(())
}

fn dry_run(probes: &[PathBuf], galleries: &[PathBuf], mode: CompareMode) {
    match mode {
        CompareMode::OneToOne => {
            assert_eq!(probes.len(), galleries.len());
            for (probe, gallery) in probes.iter().zip(galleries.iter()) {
                println!("{} {}", probe.display(), gallery.display());
            }
        }
        CompareMode::EveryProbeWithEachGallery | CompareMode::OneToMany => {
            for probe in probes {
                for gallery in galleries {
                    println!("{} {}", probe.display(), gallery.display());
                }
            }
        }
    }
}

type CallbackResult = bool;

struct MatchResult<'data> {
    probe: &'data PathBuf,
    gallery: &'data PathBuf,
    score: Option<u32>,
}

fn run(probes: &[PathBuf], galleries: &[PathBuf], compare_mode: CompareMode, options: &Options) {
    crossbeam::scope(move |scope| {
        let (tx_match_done, rx_match_done) = crossbeam::channel::unbounded::<MatchResult>();
        let output_file = options.output_file.clone();

        scope.spawn(move |_| {
            let score_callback = |score: Option<u32>| -> CallbackResult {
                if options.mode == MatchMode::Any {
                    true
                } else {
                    score >= Some(options.threshold)
                }
            };

            let format = if options.use_ansi {
                Format::Ansi
            } else {
                Format::NistInternal
            };
            if options.threads > 1 {
                execute_parallel(
                    compare_mode,
                    &ExecuteOptions {
                        match_mode: options.mode,
                        probes,
                        galleries,
                        score_callback,
                        match_done: tx_match_done,
                        max_minutiae: options.max_minutiae,
                        format,
                        threads: options.threads,
                        chunk_size: options.chunk_size,
                        relaxed_order: options.relaxed_output_order,
                    },
                )
            } else {
                execute_sequential(
                    compare_mode,
                    options.mode,
                    probes,
                    galleries,
                    score_callback,
                    tx_match_done,
                    options.max_minutiae,
                    format,
                );
            }
        });

        scope.spawn(move |_| {
            fn print_into_stream(
                output: &mut impl Write,
                rx: crossbeam::Receiver<MatchResult>,
                mode: MatchMode,
                only_scores: bool,
            ) {
                for MatchResult {
                    probe,
                    gallery,
                    score,
                } in rx
                {
                    let score = score.map(|s| s as i32).unwrap_or(-1);
                    if mode == MatchMode::Any && only_scores {
                        writeln!(output, "{}", score).unwrap();
                    } else {
                        writeln!(
                            output,
                            "{} {} {}",
                            probe.display(),
                            gallery.display(),
                            score
                        )
                        .unwrap();
                    }
                }
            }

            if let Some(file) = output_file.as_ref() {
                let file = std::fs::File::create(file).expect("cannot open file for creation");
                let mut buff = std::io::BufWriter::new(file);
                print_into_stream(&mut buff, rx_match_done, options.mode, options.only_scores);
            } else {
                let stdout = std::io::stdout();
                let stdout = stdout.lock();
                let mut buff = std::io::BufWriter::new(stdout);
                print_into_stream(&mut buff, rx_match_done, options.mode, options.only_scores);
            }
        });
    })
    .expect("cannot spawn tasks");
}

struct Fingerprint {
    minutiae: Box<[Minutia]>,
    edges: Box<[Edge]>,
}

fn extract_edges(
    file: impl AsRef<Path>,
    max_minutiae: u32,
    format: Format,
) -> anyhow::Result<Fingerprint> {
    let minutiae = prune(&parse(file).context("cannot parse file")?, max_minutiae);
    let mut edges = vec![];
    find_edges(&minutiae, &mut edges, format);
    let limit = limit_edges(&edges);
    edges.truncate(limit);
    Ok(Fingerprint {
        minutiae: minutiae.into_boxed_slice(),
        edges: edges.into_boxed_slice(),
    })
}

struct Cache {
    cache: HashMap<PathBuf, Arc<Fingerprint>>,
}

impl Cache {
    fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    fn get_or_load(
        &mut self,
        file_name: impl AsRef<Path>,
        max_minutiae: u32,
        format: Format,
    ) -> anyhow::Result<Arc<Fingerprint>> {
        if let Some(fp) = self.cache.get(file_name.as_ref()) {
            return Ok(fp.clone());
        }

        let fp = extract_edges(&file_name, max_minutiae, format)?;
        let fp = Arc::new(fp);
        self.cache.insert(file_name.as_ref().to_owned(), fp.clone());
        Ok(fp)
    }

    #[allow(unused)]
    fn get(&self, file_name: impl AsRef<Path>) -> anyhow::Result<Arc<Fingerprint>> {
        Ok(self.cache.get(file_name.as_ref()).unwrap().clone())
    }
}

trait ScoreCallback = Fn(Option<u32>) -> bool + Sync;

struct ExecuteOptions<'data, SC: ScoreCallback> {
    match_mode: MatchMode,
    probes: &'data [PathBuf],
    galleries: &'data [PathBuf],
    score_callback: SC,
    match_done: crossbeam::channel::Sender<MatchResult<'data>>,
    max_minutiae: u32,
    format: Format,
    threads: u32,
    #[allow(unused)]
    chunk_size: u32,
    relaxed_order: bool,
}

fn single_match(
    probe: &Fingerprint,
    gallery: &Fingerprint,
    pair_cacher: &mut PairHolder,
    state: &mut BozorthState,
) -> Option<u32> {
    pair_cacher.clear();
    state.clear();

    match_edges_into_pairs(
        &probe.edges,
        &probe.minutiae,
        &gallery.edges,
        &gallery.minutiae,
        pair_cacher,
        |_pk: &Minutia, _pj: &Minutia, _gk: &Minutia, _gj: &Minutia| 1,
    );
    pair_cacher.prepare();

    let actual = match_score(
        pair_cacher,
        &probe.minutiae,
        &gallery.minutiae,
        Format::NistInternal,
        state,
    )
    .unwrap_or_default()
    .0 as u32;
    Some(actual)
}

fn execute_parallel<SC: ScoreCallback>(
    compare_mode: CompareMode,
    options: &ExecuteOptions<'_, SC>,
) {
    if !options.relaxed_order {
        todo!();
    }

    let (tx, rx) = crossbeam::channel::bounded::<(&PathBuf, &PathBuf)>(1000);

    let cache: HashMap<&Path, Fingerprint> = options
        .probes
        .iter()
        .chain(options.galleries.iter())
        .par_bridge()
        .map(|it| {
            let fp = extract_edges(it, options.max_minutiae, options.format).unwrap();
            (it.as_path(), fp)
        })
        .collect();

    crossbeam::scope(|s| {
        // start workers
        for _ in 0..options.threads as usize {
            let rx = rx.clone();
            s.spawn(|_| {
                let mut state = BozorthState::new();
                let mut cacher = PairHolder::new();

                for (probe, gallery) in rx {
                    state.clear();
                    cacher.clear();

                    let score = single_match(
                        &cache[probe.as_path()],
                        &cache[gallery.as_path()],
                        &mut cacher,
                        &mut state,
                    );

                    if (options.score_callback)(score) {
                        options
                            .match_done
                            .send(MatchResult {
                                probe,
                                gallery,
                                score,
                            })
                            .unwrap();

                        if options.match_mode == MatchMode::OnlyFirstMatch {
                            return;
                        }
                    }
                }
            });
        }

        // drop unused channel that would be blocking app termination
        drop(rx);

        // start producer
        s.spawn(|_| match compare_mode {
            CompareMode::OneToOne => {
                for (probe, gallery) in options.probes.iter().zip(options.galleries.iter()) {
                    tx.send((probe, gallery)).unwrap();
                }
            }
            CompareMode::EveryProbeWithEachGallery | CompareMode::OneToMany => {
                for probe in options.probes.iter() {
                    for gallery in options.galleries.iter() {
                        tx.send((probe, gallery)).unwrap();
                    }
                }
            }
        });
    })
    .unwrap();
}

fn execute_sequential<'data>(
    compare_mode: CompareMode,
    match_mode: MatchMode,
    probes: &'data [PathBuf],
    galleries: &'data [PathBuf],
    mut score_callback: impl FnMut(Option<u32>) -> bool,
    match_done: crossbeam::channel::Sender<MatchResult<'data>>,
    max_minutiae: u32,
    format: Format,
) {
    let mut cache = Cache::new();
    let mut pair_cacher = PairHolder::new();
    let mut state = BozorthState::new();

    let mut execute = move |probe: &PathBuf, gallery: &PathBuf| -> Option<u32> {
        let gallery_cache = cache.get_or_load(gallery, max_minutiae, format);
        let probe_cache = cache.get_or_load(probe, max_minutiae, format);

        if let (Ok(gallery_fp), Ok(probe_fp)) = (gallery_cache, probe_cache) {
            pair_cacher.clear();
            state.clear();
            match_edges_into_pairs(
                &probe_fp.edges,
                &probe_fp.minutiae,
                &gallery_fp.edges,
                &gallery_fp.minutiae,
                &mut pair_cacher,
                |_pk: &Minutia, _pj: &Minutia, _gk: &Minutia, _gj: &Minutia| 1,
            );
            pair_cacher.prepare();

            let actual = match_score(
                &pair_cacher,
                &probe_fp.minutiae,
                &gallery_fp.minutiae,
                Format::NistInternal,
                &mut state,
            )
            .unwrap_or_default()
            .0 as u32;

            Some(actual)
        } else {
            None
        }
    };

    match compare_mode {
        CompareMode::OneToOne => {
            for (probe, gallery) in probes.iter().zip(galleries.iter()) {
                let score = execute(probe, gallery);
                if score_callback(score) {
                    match_done
                        .send(MatchResult {
                            probe,
                            gallery,
                            score,
                        })
                        .unwrap();
                    if match_mode == MatchMode::OnlyFirstMatch {
                        return;
                    }
                }
            }
        }
        CompareMode::EveryProbeWithEachGallery => {
            for probe in probes {
                for gallery in galleries {
                    let score = execute(probe, gallery);
                    if score_callback(score) {
                        match_done
                            .send(MatchResult {
                                probe,
                                gallery,
                                score,
                            })
                            .unwrap();
                        if match_mode == MatchMode::OnlyFirstMatch {
                            return;
                        }
                    }
                }
            }
        }
        CompareMode::OneToMany => {
            for probe in probes {
                for gallery in galleries {
                    let score = execute(probe, gallery);
                    if score_callback(score) {
                        match_done
                            .send(MatchResult {
                                probe,
                                gallery,
                                score,
                            })
                            .unwrap();
                        if match_mode == MatchMode::OnlyFirstMatch {
                            break;
                        }
                    }
                }
            }
        }
    }
}
