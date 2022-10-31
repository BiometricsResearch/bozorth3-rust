#![feature(try_blocks)]
use std::io::Write;
use std::path::Path;

use bozorth::consts::{set_angle_diff, set_factor, set_max_number_of_groups};
use bozorth::parsing::RawMinutiaCombined;
use bozorth::types::MinutiaKind;
use bozorth::{
    find_edges, limit_edges, match_edges_into_pairs, match_score, prune, set_mode, BozorthState,
    Edge, Format, Minutia, PairHolder,
};
use isoparser::{load_iso, MinutiaType, ParseError};

struct Fingerprint {
    minutiae: Box<[Minutia]>,
    edges: Box<[Edge]>,
}

fn load_my_format(path: impl AsRef<Path>) -> Result<Vec<RawMinutiaCombined>, ParseError> {
    let rec = load_iso(path)?;

    let mut minutia = vec![];
    for m in &rec.views[0].minutiae {
        minutia.push(RawMinutiaCombined {
            x: m.x as _,
            y: m.y as _,
            t: m.ty as _,
            q: m.quality as _,
            kind: match m.ty {
                MinutiaType::Other => unimplemented!(),
                MinutiaType::RidgeEnding => MinutiaKind::Type0,
                MinutiaType::RidgeBifurcation => MinutiaKind::Type1,
            },
        });
    }
    Ok(minutia)
}

fn extract_edges(file: impl AsRef<Path>) -> Result<Fingerprint, ParseError> {
    let minutiae = prune(&load_my_format(file)?, 150);

    let mut edges = vec![];
    find_edges(&minutiae, &mut edges, Format::NistInternal);
    let limit = limit_edges(&edges);

    edges.truncate(limit);
    Ok(Fingerprint {
        minutiae: minutiae.into_boxed_slice(),
        edges: edges.into_boxed_slice(),
    })
}

fn simple_match(probe_fp: &Fingerprint, gallery_fp: &Fingerprint) -> Result<u32, ()> {
    let mut pair_cacher = PairHolder::new();
    let mut state = BozorthState::new();

    pair_cacher.clear();
    match_edges_into_pairs(
        &probe_fp.edges,
        &probe_fp.minutiae,
        &gallery_fp.edges,
        &gallery_fp.minutiae,
        &mut pair_cacher,
        |pk: &Minutia, pj: &Minutia, gk: &Minutia, gj: &Minutia| match (
            pk.kind == gk.kind,
            pj.kind == gj.kind,
        ) {
            (true, true) => 4,
            (true, false) | (false, true) => 3,
            (false, false) => 2,
        },
    );
    if pair_cacher.pairs().is_empty() {
        return Err(());
    }

    pair_cacher.prepare();
    let actual = match_score(
        &pair_cacher,
        &probe_fp.minutiae,
        &gallery_fp.minutiae,
        Format::NistInternal,
        &mut state,
    )?
    .0 as u32;

    Ok(actual)
}

#[repr(i32)]
enum ErrorCode {
    Success = 0,
    SyntaxError = 1,
    CannotOpenOutputFile = 2,
    CannotOpenTemplateFile = 3,
    CannotUpdateOutputFile = 4,
    #[allow(unused)]
    InitError = 100,
    SetupError = 101,
}

fn run() -> ErrorCode {
    set_mode(true);
    set_max_number_of_groups(0);
    set_factor(0.075);
    set_angle_diff(13);

    let args: Vec<_> = std::env::args().skip(1).collect();
    let (in1, in2, out) = if let [in1, in2, out] = args.as_slice() {
        (in1, in2, out)
    } else {
        print!("\nSyntax error.\nUse: Match <templatefile1> <templatefile2> <outputfile>\n");
        return ErrorCode::SyntaxError;
    };

    let result = std::panic::catch_unwind(|| -> Result<Option<f32>, ErrorCode> {
        let probe_fp = match extract_edges(in1) {
            Ok(fp) => fp,
            Err(ParseError::InvalidFormat) | Err(ParseError::InvalidLength) => {
                return Err(ErrorCode::SetupError)
            }
            Err(ParseError::Io(_)) => return Err(ErrorCode::CannotOpenTemplateFile),
        };

        let gallery_fp = match extract_edges(in2) {
            Ok(fp) => fp,
            Err(ParseError::InvalidFormat) | Err(ParseError::InvalidLength) => {
                return Err(ErrorCode::SetupError)
            }
            Err(ParseError::Io(_)) => return Err(ErrorCode::CannotOpenTemplateFile),
        };

        let score: Option<f32> = try {
            let probe_max = simple_match(&probe_fp, &probe_fp).ok()?;
            let gallery_max = simple_match(&gallery_fp, &gallery_fp).ok()?;
            let score = simple_match(&probe_fp, &gallery_fp).ok()?;
            let max_score = std::cmp::min(probe_max, gallery_max);
            let normalized = (score as f32) / (max_score as f32);
            normalized.clamp(0.0, 1.0)
        };

        Ok(score)
    });
    let score = match result {
        Ok(Ok(score)) => score,
        _ => None,
    };

    let mut file = match std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(out)
    {
        Ok(f) => f,
        Err(_) => return ErrorCode::CannotOpenOutputFile,
    };

    match write!(
        &mut file,
        "{:>15} {:>15} {:>4} {:.6}",
        in1,
        in2,
        if score.is_some() { "OK" } else { "FAIL" },
        score.unwrap_or(0.0)
    ) {
        Ok(_) => ErrorCode::Success,
        Err(_) => ErrorCode::CannotUpdateOutputFile,
    }
}

fn main() {
    std::process::exit(run() as i32);
}
