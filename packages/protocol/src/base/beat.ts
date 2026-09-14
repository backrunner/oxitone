import { z } from "zod";
import { ErrorCode, OxitoneError } from "./errors.js";

/** Maximum wire denominator (`u32`). */
export const BEAT_MAX_DENOMINATOR = 0xffff_ffff;
/** Maximum |numerator| kept inside the JSON safe-integer range. */
export const BEAT_MAX_NUMERATOR = BigInt(Number.MAX_SAFE_INTEGER);

/**
 * Canonical wire form of a beat position/length: a reduced non-negative
 * rational. `numerator` is a JSON safe integer (wire `i64` semantics are
 * clamped to the safe range on the TypeScript side), `denominator` a `u32`.
 */
export const beatWireSchema = z
  .object({
    numerator: z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER),
    denominator: z.number().int().min(1).max(BEAT_MAX_DENOMINATOR),
  })
  .refine((b) => gcd(BigInt(b.numerator), BigInt(b.denominator)) === 1n, {
    message: "beat must be reduced (numerator/denominator coprime)",
  });

export type BeatWire = z.infer<typeof beatWireSchema>;

/** Authoring-edge beat input: a finite, non-negative number or wire form. */
export type Beat = number;

function gcd(a: bigint, b: bigint): bigint {
  let x = a < 0n ? -a : a;
  let y = b < 0n ? -b : b;
  while (y !== 0n) {
    const t = x % y;
    x = y;
    y = t;
  }
  return x;
}

/** Decompose a finite positive double into `mantissa * 2^exp2` exactly. */
function decompose(x: number): { mantissa: bigint; exp2: number } {
  const buffer = new DataView(new ArrayBuffer(8));
  buffer.setFloat64(0, x);
  const bits = buffer.getBigUint64(0);
  const rawExp = Number((bits >> 52n) & 0x7ffn);
  const frac = bits & 0xf_ffff_ffff_ffffn;
  if (rawExp === 0) {
    return { mantissa: frac, exp2: -1074 };
  }
  return { mantissa: frac | 0x10_0000_0000_0000n, exp2: rawExp - 1075 };
}

/**
 * Best rational approximation of `x` with `denominator <= maxDen` and
 * `numerator <= maxNum`, computed by exact continued-fraction expansion of
 * the double's binary value. Deterministic across platforms.
 */
export function rationalFromF64(
  x: number,
  maxDen: bigint = BigInt(BEAT_MAX_DENOMINATOR),
  maxNum: bigint = BEAT_MAX_NUMERATOR,
): { numerator: bigint; denominator: bigint } {
  if (!Number.isFinite(x) || x < 0) {
    throw new OxitoneError(ErrorCode.InvalidProject, `beat must be finite and >= 0, got ${x}`);
  }
  if (Number.isInteger(x) && BigInt(x) <= maxNum) {
    return { numerator: BigInt(x), denominator: 1n };
  }
  const { mantissa, exp2 } = decompose(x);
  let num = mantissa;
  let den = 1n;
  if (exp2 >= 0) {
    num <<= BigInt(exp2);
  } else {
    den <<= BigInt(-exp2);
  }

  let pm2 = 0n;
  let pm1 = 1n;
  let qm2 = 1n;
  let qm1 = 0n;
  let n = num;
  let d = den;
  for (;;) {
    const a = n / d;
    const p = a * pm1 + pm2;
    const q = a * qm1 + qm2;
    if (q > maxDen || p > maxNum) {
      const tDen = qm1 === 0n ? maxDen : (maxDen - qm2) / qm1;
      const tNum = pm1 === 0n ? maxNum : (maxNum - pm2) / pm1;
      const t = tDen < tNum ? tDen : tNum;
      if (t >= 1n) {
        const ps = t * pm1 + pm2;
        const qs = t * qm1 + qm2;
        // Prefer the semiconvergent only when it is not worse than the
        // previous convergent (cross-multiplied error comparison).
        const errSemi = (ps * den - qs * num) * (ps * den - qs * num) * qm1 * qm1;
        const errPrev = (pm1 * den - qm1 * num) * (pm1 * den - qm1 * num) * qs * qs;
        if (errSemi <= errPrev) {
          return { numerator: ps, denominator: qs };
        }
      }
      return { numerator: pm1, denominator: qm1 };
    }
    pm2 = pm1;
    pm1 = p;
    qm2 = qm1;
    qm1 = q;
    const r = n - a * d;
    if (r === 0n) {
      return { numerator: pm1, denominator: qm1 };
    }
    n = d;
    d = r;
  }
}

/**
 * Convert an authoring beat (finite, non-negative number) or an existing
 * wire value into the canonical reduced `{ numerator, denominator }` form.
 */
export function beatToWire(beat: Beat | BeatWire): BeatWire {
  if (typeof beat === "object" && beat !== null) {
    const parsed = beatWireSchema.parse(beat);
    return { numerator: parsed.numerator, denominator: parsed.denominator };
  }
  if (!Number.isFinite(beat) || beat < 0) {
    throw new OxitoneError(ErrorCode.InvalidProject, `beat must be finite and >= 0, got ${beat}`);
  }
  const { numerator, denominator } = rationalFromF64(beat);
  return { numerator: Number(numerator), denominator: Number(denominator) };
}

/** Exact decimal value of a wire beat (for display/preview only). */
export function beatFromWire(wire: BeatWire): number {
  const parsed = beatWireSchema.parse(wire);
  return parsed.numerator / parsed.denominator;
}
