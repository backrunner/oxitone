import { z } from "zod";

/** Opaque, globally unique entity identifier (`trk_`, `pat_`, ... prefixes). */
export const entityIdSchema = z
  .string()
  .min(1)
  .max(128)
  .regex(/^[A-Za-z0-9_-]+$/, "entity IDs use [A-Za-z0-9_-] only");
export type EntityId = z.infer<typeof entityIdSchema>;

/** Documented ID prefixes (01-architecture.md). Unknown prefixes remain valid. */
export const ID_PREFIXES = {
  track: "trk_",
  pattern: "pat_",
  channel: "chn_",
  mixerChannel: "mix_",
  sample: "smp_",
  automation: "auto_",
} as const;

/** MIDI pitch, integer 0..127 (Phase 1). */
export const pitchSchema = z.number().int().min(0).max(127);
export type Pitch = z.infer<typeof pitchSchema>;

/**
 * Wire form of a `u64` sample frame / revision: an unsigned decimal string
 * (06-format-and-export.md). JSON numbers cannot hold the full `u64` range.
 */
export const frameWireSchema = z
  .string()
  .regex(/^(0|[1-9]\d{0,19})$/, "frame must be an unsigned decimal string")
  .refine((s) => BigInt(s) <= 0xffff_ffff_ffff_ffffn, {
    message: "frame exceeds u64 range",
  });
export type FrameWire = z.infer<typeof frameWireSchema>;

export function frameToWire(frame: bigint | number): FrameWire {
  const value = typeof frame === "bigint" ? frame : BigInt(frame);
  if (value < 0n || value > 0xffff_ffff_ffff_ffffn) {
    throw new RangeError(`frame out of u64 range: ${value}`);
  }
  return value.toString(10);
}

export function frameFromWire(wire: FrameWire): bigint {
  return BigInt(frameWireSchema.parse(wire));
}

export const timecodeSchema = z.union([
  z.strictObject({ seconds: z.number().finite().nonnegative() }),
  z.strictObject({ frames: frameWireSchema }),
]);
export type Timecode = z.infer<typeof timecodeSchema>;
