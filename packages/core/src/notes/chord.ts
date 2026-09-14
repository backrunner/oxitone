import type { Beat, EntityId, Pitch } from "@oxitone/protocol";
import type { Pattern } from "../patterns/pattern.js";
import { createSource } from "../source/graph.js";
import { patternFromSource } from "../source/pattern-values.js";

export type ChordQuality = "major" | "minor" | "dim" | "aug" | "sus2" | "sus4";
export type ChordVoicing = "close" | "open";

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

/** Immutable generated chord. Degree selectors retain pre-inversion membership. */
export function chord(root: Pitch, quality: ChordQuality, options: ChordOptions = {}): Pattern {
  const { id, name, ...music } = options;
  return patternFromSource(createSource({ kind: "chord", root, quality, options: music }), {
    ...(id === undefined ? {} : { id }),
    ...(name === undefined ? {} : { name }),
  });
}
