//! Rational beat values (`num: i64`, `den: u32`), always reduced and
//! non-negative. Wire form is `{ "numerator": n, "denominator": d }`
//! (04-api-contracts.md). `from_f64` exists for display/preview only; the
//! authoritative float→rational conversion lives in TypeScript
//! (`packages/protocol/src/beat.ts`).

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::{codes, OxitoneError};

/// Reduced non-negative rational beat position/length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Beat {
    numerator: i64,
    denominator: u32,
}

#[derive(Deserialize)]
struct BeatWire {
    numerator: i64,
    denominator: u32,
}

impl<'de> Deserialize<'de> for Beat {
    /// Decoding reduces and validates so unreduced wire values are rejected,
    /// matching the zod schema on the TypeScript side.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = BeatWire::deserialize(deserializer)?;
        Beat::new(wire.numerator, wire.denominator).map_err(serde::de::Error::custom)
    }
}

fn gcd_i128(mut a: i128, mut b: i128) -> i128 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a.abs()
}

impl Beat {
    pub const ZERO: Beat = Beat {
        numerator: 0,
        denominator: 1,
    };

    /// Construct a reduced beat; rejects negative values and `denominator == 0`.
    pub fn new(numerator: i64, denominator: u32) -> Result<Self, OxitoneError> {
        if numerator < 0 || denominator == 0 {
            return Err(OxitoneError::new(
                codes::INVALID_PROJECT,
                format!(
                    "beat must be non-negative with denominator > 0, got {numerator}/{denominator}"
                ),
            ));
        }
        let g = gcd_i128(numerator as i128, denominator as i128);
        Ok(Self {
            numerator: (numerator as i128 / g) as i64,
            denominator: (denominator as i128 / g) as u32,
        })
    }

    pub fn numerator(&self) -> i64 {
        self.numerator
    }

    pub fn denominator(&self) -> u32 {
        self.denominator
    }

    fn checked(num: i128, den: i128) -> Result<Self, OxitoneError> {
        let g = gcd_i128(num, den);
        let num = num / g;
        let den = den / g;
        if num < 0 || num > i64::MAX as i128 || den > u32::MAX as i128 {
            return Err(OxitoneError::new(
                codes::INVALID_PROJECT,
                "beat arithmetic overflowed the i64/u32 wire range",
            ));
        }
        Ok(Self {
            numerator: num as i64,
            denominator: den as u32,
        })
    }

    pub fn checked_add(self, other: Beat) -> Result<Beat, OxitoneError> {
        let (a, b) = (self.as_pair(), other.as_pair());
        Beat::checked(a.0 * b.1 + b.0 * a.1, a.1 * b.1)
    }

    pub fn checked_sub(self, other: Beat) -> Result<Beat, OxitoneError> {
        let (a, b) = (self.as_pair(), other.as_pair());
        Beat::checked(a.0 * b.1 - b.0 * a.1, a.1 * b.1)
    }

    pub fn checked_mul(self, other: Beat) -> Result<Beat, OxitoneError> {
        let (a, b) = (self.as_pair(), other.as_pair());
        Beat::checked(a.0 * b.0, a.1 * b.1)
    }

    fn as_pair(&self) -> (i128, i128) {
        (self.numerator as i128, self.denominator as i128)
    }

    /// Display/preview value; scheduling always uses the rational form.
    pub fn to_f64(self) -> f64 {
        self.numerator as f64 / self.denominator as f64
    }

    /// Best rational within the wire range, via exact continued-fraction
    /// expansion of the double's binary value. Display/preview only; the
    /// authoring-edge conversion is the TypeScript `rationalFromF64`.
    pub fn from_f64(x: f64) -> Result<Beat, OxitoneError> {
        if !x.is_finite() || x < 0.0 {
            return Err(OxitoneError::new(
                codes::INVALID_PROJECT,
                format!("beat must be finite and >= 0, got {x}"),
            ));
        }
        if x >= 9_223_372_036_854_775_808.0 {
            return Err(OxitoneError::new(
                codes::INVALID_PROJECT,
                "beat exceeds i64 range",
            ));
        }
        // Values below half the smallest representable positive rational round to zero.
        if x <= 0.5 / u32::MAX as f64 {
            return Ok(Beat::ZERO);
        }
        if x.fract() == 0.0 {
            return Beat::new(x as i64, 1);
        }
        let bits = x.to_bits();
        let raw_exp = ((bits >> 52) & 0x7ff) as i32;
        let frac = bits & 0xf_ffff_ffff_ffff;
        let (mantissa, exp2) = if raw_exp == 0 {
            (frac, -1074)
        } else {
            (frac | 0x10_0000_0000_0000, raw_exp - 1075)
        };
        // The early small-value bound limits the denominator to at most 2^86.
        // Preserve the binary exponent exactly; clamping it changes time itself.
        let shift = exp2;
        let mut num = mantissa as i128;
        let mut den = 1i128;
        if shift >= 0 {
            num <<= shift;
        } else {
            den <<= -shift;
        }

        let max_den = u32::MAX as i128;
        let max_num = i64::MAX as i128;
        let (mut pm2, mut pm1, mut qm2, mut qm1) = (0i128, 1i128, 1i128, 0i128);
        let (mut n, mut d) = (num, den);
        loop {
            let a = n / d;
            let p = a * pm1 + pm2;
            let q = a * qm1 + qm2;
            if q > max_den || p > max_num {
                let t_den = if qm1 == 0 {
                    max_den
                } else {
                    (max_den - qm2) / qm1
                };
                let t_num = if pm1 == 0 {
                    max_num
                } else {
                    (max_num - pm2) / pm1
                };
                let t = t_den.min(t_num);
                if t >= 1 {
                    let ps = t * pm1 + pm2;
                    let qs = t * qm1 + qm2;
                    let err_semi = (ps * den - qs * num).abs() * qm1;
                    let err_prev = (pm1 * den - qm1 * num).abs() * qs;
                    if err_semi <= err_prev {
                        return Beat::checked(ps, qs);
                    }
                }
                return Beat::checked(pm1, qm1);
            }
            pm2 = pm1;
            pm1 = p;
            qm2 = qm1;
            qm1 = q;
            let r = n - a * d;
            if r == 0 {
                return Beat::checked(pm1, qm1);
            }
            n = d;
            d = r;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reduces_on_construction() {
        let b = Beat::new(2, 4).unwrap();
        assert_eq!((b.numerator(), b.denominator()), (1, 2));
    }

    #[test]
    fn arithmetic_stays_reduced() {
        let half = Beat::new(1, 2).unwrap();
        let third = Beat::new(1, 3).unwrap();
        assert_eq!(half.checked_add(third).unwrap(), Beat::new(5, 6).unwrap());
        assert_eq!(half.checked_sub(third).unwrap(), Beat::new(1, 6).unwrap());
        assert_eq!(half.checked_mul(third).unwrap(), Beat::new(1, 6).unwrap());
    }

    #[test]
    fn f64_round_trip() {
        for (num, den) in [(0, 1), (1, 2), (1, 3), (15, 4), (1, 25)] {
            let b = Beat::new(num, den).unwrap();
            let back = Beat::from_f64(b.to_f64()).unwrap();
            assert_eq!(back, b, "{num}/{den}");
        }
    }

    #[test]
    fn approximates_non_terminating_decimals() {
        assert_eq!(Beat::from_f64(0.1).unwrap(), Beat::new(1, 10).unwrap());
    }

    #[test]
    fn rejects_invalid_values() {
        assert!(Beat::new(-1, 2).is_err());
        assert!(Beat::new(1, 0).is_err());
        assert!(Beat::from_f64(f64::NAN).is_err());
        assert!(Beat::from_f64(-0.5).is_err());
    }

    #[test]
    fn serde_shape_matches_wire() {
        let b = Beat::new(3, 8).unwrap();
        let json = serde_json::to_string(&b).unwrap();
        assert_eq!(json, r#"{"numerator":3,"denominator":8}"#);
        let back: Beat = serde_json::from_str(&json).unwrap();
        assert_eq!(back, b);
    }
}
