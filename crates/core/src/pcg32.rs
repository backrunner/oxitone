//! pcg32-v1 and hash64-v1: the versioned deterministic randomness primitives
//! shared with `@oxitone/protocol` (07-automation-spec.md §3). Frozen for
//! protocol v1; golden vectors in `schemas/fixtures/pcg32-v1.json` and
//! `schemas/fixtures/hash64.json` must match the TypeScript implementation
//! bit for bit. See the doc comment in `packages/protocol/src/pcg32.ts` for
//! the normative parameter list.

use sha2::{Digest, Sha256};

pub const PCG32_MULTIPLIER: u64 = 6364136223846793005;
pub const PCG32_INCREMENT: u64 = 1442695040888963407;

/// Classic PCG32 with a fixed stream (increment) and reference seeding.
pub struct Pcg32 {
    state: u64,
}

impl Pcg32 {
    pub fn new(seed: u64) -> Self {
        let mut rng = Self { state: 0 };
        rng.next_u32();
        rng.state = rng.state.wrapping_add(seed);
        rng.next_u32();
        rng
    }

    /// Advance the state and return a u32.
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old
            .wrapping_mul(PCG32_MULTIPLIER)
            .wrapping_add(PCG32_INCREMENT);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Uniform value in [0, 1): `next_u32() * 2^-32`.
    pub fn next_f64(&mut self) -> f64 {
        self.next_u32() as f64 / 4294967296.0
    }
}

/// Typed part of a hash64 input (u64 integers and UTF-8 strings).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hash64Part {
    Int(u64),
    Str(String),
}

impl From<u64> for Hash64Part {
    fn from(value: u64) -> Self {
        Self::Int(value)
    }
}

impl From<&str> for Hash64Part {
    fn from(value: &str) -> Self {
        Self::Str(value.to_owned())
    }
}

impl From<String> for Hash64Part {
    fn from(value: String) -> Self {
        Self::Str(value)
    }
}

/// Canonical byte encoding shared with the TypeScript side: parts map to
/// `n:<u64 decimal>` or `s:<utf8>`, joined by `|`.
pub fn hash64_input(parts: &[Hash64Part]) -> String {
    parts
        .iter()
        .map(|part| match part {
            Hash64Part::Int(value) => format!("n:{value}"),
            Hash64Part::Str(value) => format!("s:{value}"),
        })
        .collect::<Vec<_>>()
        .join("|")
}

/// hash64-v1: SHA-256 over the canonical encoding; the first 8 bytes
/// big-endian are the u64 seed.
pub fn hash64(parts: &[Hash64Part]) -> u64 {
    let digest = Sha256::digest(hash64_input(parts).as_bytes());
    u64::from_be_bytes(digest[..8].try_into().expect("sha256 digest is 32 bytes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_across_instances() {
        let mut a = Pcg32::new(42);
        let mut b = Pcg32::new(42);
        for _ in 0..64 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn floats_stay_in_unit_interval() {
        let mut rng = Pcg32::new(7);
        for _ in 0..1000 {
            let f = rng.next_f64();
            assert!((0.0..1.0).contains(&f));
        }
    }

    #[test]
    fn hash_is_stable() {
        let parts = [
            Hash64Part::Int(99),
            Hash64Part::Int(17),
            Hash64Part::Str("auto_0003".into()),
        ];
        assert_eq!(hash64(&parts), hash64(&parts));
        assert_eq!(hash64_input(&parts), "n:99|n:17|s:auto_0003");
    }
}
