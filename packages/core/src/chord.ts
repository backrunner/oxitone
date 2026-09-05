import {
  ErrorCode,
  OxitoneError,
  type Beat,
  type EntityId,
  type Pitch,
} from "@oxitone/protocol";
import type { NoteInput } from "./note.js";
import { Pattern } from "./pattern.js";

/** Supported chord qualities. */
export type ChordQuality = "major" | "minor" | "dim" | "aug" | "sus2" | "sus4";

/** Voicing layout: stacked thirds (`close`) or drop-2 (`open`). */
export type ChordVoicing = "close" | "open";

/** Options for {@link chord}. */
export interface ChordOptions {
  inversion?: number;
  voicing?: ChordVoicing;
  duration?: Beat;
  velocity?: number;
  start?: Beat;
  lengthBeats?: Beat;
  name?: string;
  id?: EntityId;
}

const CHORD_INTERVALS: Record<ChordQuality, readonly number[]> = {
  major: [0, 4, 7],
  minor: [0, 3, 7],
  dim: [0, 3, 6],
  aug: [0, 4, 8],
  sus2: [0, 2, 7],
  sus4: [0, 5, 7],
};

const MAX_PITCH = 127;

/**
 * Build a chord as a new immutable {@link Pattern}. Pure: inputs are not
 * mutated and every call returns a fresh pattern. All notes share `start`,
 * `duration`, and `velocity`; `voice` hints encode the stacking order.
 */
export function chord(root: Pitch, quality: ChordQuality, options: ChordOptions = {}): Pattern {
  if (!Number.isInteger(root) || root < 0 || root > MAX_PITCH) {
    throw new OxitoneError(ErrorCode.InvalidProject, `chord root must be 0..127, got ${root}`, {
      details: { path: "chord.root" },
    });
  }
  const intervals = CHORD_INTERVALS[quality];
  if (intervals === undefined) {
    throw new OxitoneError(ErrorCode.InvalidProject, `unknown chord quality: ${String(quality)}`, {
      details: { path: "chord.quality" },
    });
  }
  const inversion = options.inversion ?? 0;
  if (!Number.isInteger(inversion) || inversion < 0) {
    throw new OxitoneError(
      ErrorCode.InvalidProject,
      `chord inversion must be an integer >= 0, got ${inversion}`,
      { details: { path: "chord.inversion" } },
    );
  }
  const voicing = options.voicing ?? "close";
  const start = options.start ?? 0;
  const duration = options.duration ?? 1;
  const velocity = options.velocity ?? 1;

  const pitches = intervals.map((interval) => root + interval);
  for (let step = 0; step < inversion; step += 1) {
    const lowest = pitches.shift();
    if (lowest !== undefined) {
      pitches.push(lowest + 12);
    }
  }
  if (voicing === "open" && pitches.length >= 3) {
    const secondFromTop = pitches[pitches.length - 2];
    if (secondFromTop !== undefined) {
      pitches[pitches.length - 2] = secondFromTop - 12;
      pitches.sort((a, b) => a - b);
    }
  }

  const notes: NoteInput[] = pitches.map((pitch, index) => ({
    pitch: Math.min(pitch, MAX_PITCH),
    start,
    duration,
    velocity,
    voice: index,
  }));

  return new Pattern({
    lengthBeats: options.lengthBeats ?? start + duration,
    notes,
    ...(options.id !== undefined ? { id: options.id } : {}),
    ...(options.name !== undefined ? { name: options.name } : {}),
  });
}
