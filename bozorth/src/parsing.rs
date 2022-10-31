use std::fs;
use std::io;
use std::io::BufRead;
use std::path::Path;

use crate::types::MinutiaKind;

#[derive(Debug, Copy, Clone)]
pub struct RawMinutia {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) t: i32,
    pub(crate) q: i32,
}

pub fn parse_xyt(path: impl AsRef<Path>) -> Result<Vec<RawMinutia>, io::Error> {
    let file = fs::File::open(path)?;
    let reader = io::BufReader::new(file);
    let mut minutiae = vec![];
    for line in reader.lines() {
        let line = line?;
        let mut parts = line.split(' ').map(|it| it.parse::<i32>().unwrap());
        let x = parts.next().unwrap();
        let y = parts.next().unwrap();
        let t = parts.next().unwrap();
        let q = parts.next().unwrap_or(0);

        minutiae.push(RawMinutia { x, y, t, q });
    }

    Ok(minutiae)
}

#[derive(Debug, Copy, Clone)]
pub struct RawMinutiaExtended {
    pub(crate) kind: MinutiaKind,
}

pub fn parse_min(xyt_path: impl AsRef<Path>) -> Result<Vec<RawMinutiaExtended>, io::Error> {
    let file = fs::File::open(xyt_path)?;
    let reader = io::BufReader::new(file);

    let mut minutiae = vec![];
    for line in reader.lines().skip(4) {
        let line = line?;
        let mut columns = line.split(':');
        let _index = columns.next().unwrap();
        let _position = columns.next().unwrap();
        let _feature_id = columns.next().unwrap();
        let _reliability = columns.next().unwrap();
        let kind = columns.next().unwrap();
        let _mode = columns.next().unwrap();
        minutiae.push(RawMinutiaExtended {
            kind: match kind.trim() {
                "RIG" => MinutiaKind::Type0,
                "BIF" => MinutiaKind::Type1,
                _ => unimplemented!(),
            },
        })
    }

    Ok(minutiae)
}

#[derive(Debug, Copy, Clone)]
pub struct RawMinutiaCombined {
    pub x: i32,
    pub y: i32,
    pub t: i32,
    pub q: i32,
    pub kind: MinutiaKind,
}

pub fn parse(xyt_path: impl AsRef<Path>) -> Result<Vec<RawMinutiaCombined>, io::Error> {
    let xyt_path = xyt_path.as_ref();
    let a = parse_xyt(xyt_path)?;
    let mut min: Vec<_> = a
        .into_iter()
        .map(|it| RawMinutiaCombined {
            x: it.x,
            y: it.y,
            t: if it.t > 180 { it.t - 360 } else { it.t },
            q: it.q,
            kind: MinutiaKind::Type0,
        })
        .collect();

    let min_path = xyt_path.with_extension("min");
    if min_path.exists() {
        for (i, m) in parse_min(min_path)?.into_iter().enumerate() {
            min[i].kind = m.kind;
        }
    }

    Ok(min)
}
