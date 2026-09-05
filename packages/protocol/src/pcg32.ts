import { createHash } from "node:crypto";

/**
 * pcg32-v1: the versioned deterministic PRNG used by `chance` automation and
 * dither (07-automation-spec.md §3). The parameter set is the classic PCG32
 * (O'Neill 2014) with a fixed stream; it is frozen for protocol v1 and any
 * change requires a protocol major bump or a new `randomAlgorithm` ID.
 *
 * - state: u64, multiplier: 6364136223846793005 (0x5851F42D4C957F2D)
 * - increment: 1442695040888963407 (0x14057B7EF767814F), fixed for all seeds
 * - seeding (PCG reference seeding with the fixed increment):
 *   state = 0; advance(); state = (state + seed) mod 2^64; advance()
 * - step: state = state * multiplier + increment (mod 2^64)
 * - output from the pre-step state `s`:
 *   x = u32(((s >> 18) ^ s) >> 27); rot = s >> 59; out = rotr32(x, rot)
 * - [0,1) conversion: out * 2^-32
 *
 * Golden vectors live in `schemas/fixtures/pcg32-v1.json` and must match the
 * Rust implementation in `crates/core` bit for bit.
 */
export const PCG32_MULTIPLIER = 6364136223846793005n;
export const PCG32_INCREMENT = 1442695040888963407n;

const MASK64 = (1n << 64n) - 1n;

export class Pcg32 {
  private state: bigint;

  constructor(seed: bigint | number) {
    this.state = 0n;
    this.nextU32();
    this.state = (this.state + BigInt(seed)) & MASK64;
    this.nextU32();
  }

  /** Advance the state and return a u32. */
  nextU32(): number {
    const old = this.state;
    this.state = (old * PCG32_MULTIPLIER + PCG32_INCREMENT) & MASK64;
    const xorshifted = Number((((old >> 18n) ^ old) >> 27n) & 0xffff_ffffn);
    const rot = Number(old >> 59n);
    return ((xorshifted >>> rot) | (xorshifted << ((32 - rot) & 31))) >>> 0;
  }

  /** Uniform value in [0, 1): `nextU32() * 2^-32`. */
  nextFloat(): number {
    return this.nextU32() / 4294967296;
  }
}

export type Hash64Part = number | bigint | string;

function encodePart(part: Hash64Part): string {
  if (typeof part === "string") {
    return `s:${part}`;
  }
  const value = typeof part === "bigint" ? part : BigInt(part);
  if (value < 0n || value > 0xffff_ffff_ffff_ffffn) {
    throw new RangeError(`hash64 integer part out of u64 range: ${value}`);
  }
  return `n:${value.toString(10)}`;
}

/**
 * hash64-v1: derive a deterministic 64-bit seed from structured parts, e.g.
 * `hash64(projectSeed, laneSeed, canonicalSourcePath, loopIteration?)`.
 * Encoding: parts map to `n:<u64 decimal>` or `s:<utf8>`, joined by `|`,
 * hashed with SHA-256; the first 8 bytes big-endian are the u64 result.
 * Golden vectors live in `schemas/fixtures/hash64.json`.
 */
export function hash64(...parts: Hash64Part[]): bigint {
  const input = parts.map(encodePart).join("|");
  const digest = createHash("sha256").update(input, "utf8").digest();
  return digest.readBigUInt64BE(0);
}

/** Byte encoding shared with the Rust side; exposed for fixtures/tests. */
export function hash64Input(...parts: Hash64Part[]): string {
  return parts.map(encodePart).join("|");
}
