use std::fmt;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MinutiaKind {
    Type0,
    Type1,
}

/// Represents a single minutia.
#[derive(Debug)]
pub struct Minutia {
    /// X coordinate.
    pub x: i32,
    /// Y coordinate.
    pub y: i32,
    /// Orientation in degrees.
    pub theta: i32,
    /// Type of the minutia.
    pub kind: MinutiaKind,
}

/// Represents a type-safe index of a minutia in the list of minutiae.
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Endpoint(pub(crate) u32);

impl fmt::Debug for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl Endpoint {
    pub(crate) fn as_usize(self) -> usize {
        self.0 as usize
    }
}

impl Into<usize> for Endpoint {
    fn into(self) -> usize {
        self.0 as usize
    }
}

impl Into<Endpoint> for u8 {
    fn into(self) -> Endpoint {
        Endpoint(self as _)
    }
}

impl Into<Endpoint> for u32 {
    fn into(self) -> Endpoint {
        Endpoint(self as _)
    }
}

impl Into<Endpoint> for usize {
    #[inline(never)]
    fn into(self) -> Endpoint {
        if self >= 200 {
            dbg!(self);
        }
        Endpoint(self.min(200) as _)
    }
}

/// Pair of corresponding minutiae on two fingerprints (Probe and Gallery).
#[derive(Debug, Copy, Clone)]
pub struct Pair {
    /// Difference between theta angles on both fingerprints.
    pub delta_theta: i32,
    /// Endpoint K on the Probe fingerprint.
    pub probe_k: Endpoint,
    /// Corresponding endpoint K the Gallery fingerprint.
    pub gallery_k: Endpoint,
    /// Endpoint J on the Probe fingerprint.
    pub probe_j: Endpoint,
    /// Corresponding endpoint J on the Gallery fingerprint.
    pub gallery_j: Endpoint,
    /// Points that should be added to the cluster's score for inclusion of this pair to this cluster.
    pub points: u32,
}

/// Denotes order of minutiae from which `min_beta` and `max_beta` was taken.
#[derive(Eq, PartialEq, Debug, Copy, Clone)]
pub enum BetaOrder {
    /// `min_beta` contains `beta` from minutia K, `max_beta` from minutia J
    KJ,
    /// `min_beta` contains `beta` from minutia J, `max_beta` from minutia K
    JK,
}

/// Represents a pair of minutiae on a single fingerprint.
#[derive(Debug, Copy, Clone)]
pub struct Edge {
    /// Distance between the minutiae squared.
    pub distance_squared: i32,
    /// The smallest of the `beta` angles of the minutia.
    pub min_beta: i32,
    /// The greatest of the `beta` angles of the minutia.
    pub max_beta: i32,
    /// The leftmost endpoint (with smallest `x` coordinate).
    pub endpoint_k: Endpoint,
    /// The rightmost endpoint (with greatest `x` coordinate).
    pub endpoint_j: Endpoint,
    /// The slope (in degrees) of a line passing through both endpoints.
    pub theta_kj: i32,
    /// Order of endpoints the `min_beta` and `max_beta` were taken from.
    pub beta_order: BetaOrder,
}

#[derive(Copy, Clone)]
pub enum Format {
    NistInternal,
    #[allow(unused)]
    Ansi,
}
