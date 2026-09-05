import {
  ErrorCode,
  OxitoneError,
  Pcg32,
  type Beat,
  type EntityId,
  type Pitch,
} from "@oxitone/protocol";
import type { NoteInput } from "./note.js";
import { Pattern } from "./pattern.js";

/** Arpeggio playback order. */
export type ArpOrder = "up" | "down" | "upDown" | "random";

/** Linear velocity ramp applied across the generated sequence. */
export interface ArpVelocityCurve {
  from: number;
  to: number;
}

/** Options for {@link arp}. */
export interface ArpOptions {
  /** Seed for `order: "random"` (pcg32-v1); defaults to 0 for determinism. */
  seed?: number | bigint;
  /** Note duration as a fraction of `rate`, in (0, 1]. Default 0.9. */
  gate?: number;
  /** Octave repetitions stacked upward. Default 1. */
  octaves?: number;
  /** Base velocity in 0..1; scales the curve when both are given. Default 1. */
  velocity?: number;
  /** Linear ramp from `from` to `to` across the sequence. */
  velocityCurve?: ArpVelocityCurve;
  lengthBeats?: Beat;
  name?: string;
  id?: EntityId;
}

const MAX_PITCH = 127;

function shuffleDeterministic(pitches: readonly number[], seed: number | bigint): number[] {
  const rng = new Pcg32(seed);
  const result = [...pitches];
  for (let i = result.length - 1; i > 0; i -= 1) {
    const j = Math.floor(rng.nextFloat() * (i + 1));
    const a = result[i];
    const b = result[j];
    if (a !== undefined && b !== undefined) {
      result[i] = b;
      result[j] = a;
    }
  }
  return result;
}

function orderPitches(pitches: readonly number[], order: ArpOrder, seed: number | bigint): number[] {
  switch (order) {
    case "up":
      return [...pitches].sort((a, b) => a - b);
    case "down":
      return [...pitches].sort((a, b) => b - a);
    case "upDown": {
      const ascending = [...pitches].sort((a, b) => a - b);
      return ascending.concat(ascending.slice(1, -1).reverse());
    }
    case "random":
      return shuffleDeterministic(pitches, seed);
    default:
      throw new OxitoneError(ErrorCode.InvalidProject, `unknown arp order: ${String(order)}`, {
        details: { path: "arp.order" },
      });
  }
}

/**
 * Build an arpeggio as a new immutable {@link Pattern}. Pure: the input note
 * array is never mutated. `rate` is the per-note step in beats; `random`
 * order is deterministic for a given `seed` via pcg32-v1.
 */
export function arp(
  notes: readonly (Pitch | NoteInput)[],
  order: ArpOrder,
  rate: Beat,
  options: ArpOptions = {},
): Pattern {
  if (notes.length === 0) {
    throw new OxitoneError(ErrorCode.InvalidProject, "arp requires at least one note", {
      details: { path: "arp.notes" },
    });
  }
  if (!Number.isFinite(rate) || rate <= 0) {
    throw new OxitoneError(ErrorCode.InvalidProject, `arp rate must be > 0, got ${rate}`, {
      details: { path: "arp.rate" },
    });
  }
  const gate = options.gate ?? 0.9;
  if (!Number.isFinite(gate) || gate <= 0 || gate > 1) {
    throw new OxitoneError(ErrorCode.InvalidProject, `arp gate must be in (0, 1], got ${gate}`, {
      details: { path: "arp.gate" },
    });
  }
  const octaves = options.octaves ?? 1;
  if (!Number.isInteger(octaves) || octaves < 1) {
    throw new OxitoneError(
      ErrorCode.InvalidProject,
      `arp octaves must be an integer >= 1, got ${octaves}`,
      { details: { path: "arp.octaves" } },
    );
  }
  const velocity = options.velocity ?? 1;
  const curve = options.velocityCurve;
  for (const value of curve !== undefined ? [velocity, curve.from, curve.to] : [velocity]) {
    if (!Number.isFinite(value) || value < 0 || value > 1) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        `arp velocity values must be in 0..1, got ${value}`,
        { details: { path: "arp.velocity" } },
      );
    }
  }

  const base = notes.map((note) => (typeof note === "number" ? note : note.pitch));
  for (const pitch of base) {
    if (!Number.isInteger(pitch) || pitch < 0 || pitch > MAX_PITCH) {
      throw new OxitoneError(ErrorCode.InvalidProject, `arp pitch must be 0..127, got ${pitch}`, {
        details: { path: "arp.notes.pitch" },
      });
    }
  }

  const cycle = orderPitches(base, order, options.seed ?? 0);
  const sequence: number[] = [];
  for (let octave = 0; octave < octaves; octave += 1) {
    for (const pitch of cycle) {
      sequence.push(Math.min(pitch + 12 * octave, MAX_PITCH));
    }
  }

  const result: NoteInput[] = sequence.map((pitch, index) => {
    const t = sequence.length > 1 ? index / (sequence.length - 1) : 0;
    const shaped = curve !== undefined ? curve.from + (curve.to - curve.from) * t : 1;
    return {
      pitch,
      start: index * rate,
      duration: rate * gate,
      velocity: velocity * shaped,
      voice: index,
    };
  });

  return new Pattern({
    lengthBeats: options.lengthBeats ?? sequence.length * rate,
    notes: result,
    ...(options.id !== undefined ? { id: options.id } : {}),
    ...(options.name !== undefined ? { name: options.name } : {}),
  });
}
