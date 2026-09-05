import { z } from "zod";
import {
  automationLaneSpecSchema,
  channelSpecSchema,
  mixerChannelSpecSchema,
  patternClipSpecSchema,
  patternSpecSchema,
  sampleClipSpecSchema,
} from "./authoring.js";
import { canonicalEncode } from "./canonical.js";
import { entityIdSchema, frameWireSchema } from "./primitives.js";
import { sampleRefSchema, trackSpecSchema } from "./refs.js";
import { markerSpecSchema, tempoSegmentSchema, timeSignatureSegmentSchema } from "./timeline.js";
import { checkProtocolVersion } from "./version.js";

/** Immutable, fully validated project state exchanged with the Rust engine. */
export const projectSnapshotSchema = z.object({
  protocolVersion: z.string(),
  revision: frameWireSchema,
  id: entityIdSchema,
  name: z.string().optional(),
  sampleRate: z.number().int().positive(),
  blockSize: z.number().int().positive(),
  seed: z.number().int().nonnegative(),
  tempoMap: z.array(tempoSegmentSchema).min(1),
  timeSignatureMap: z.array(timeSignatureSegmentSchema).min(1),
  markers: z.array(markerSpecSchema),
  tracks: z.array(trackSpecSchema),
  patterns: z.array(patternSpecSchema),
  patternClips: z.array(patternClipSpecSchema),
  sampleClips: z.array(sampleClipSpecSchema),
  samples: z.array(sampleRefSchema),
  channels: z.array(channelSpecSchema),
  mixerChannels: z.array(mixerChannelSpecSchema),
  automation: z.array(automationLaneSpecSchema),
});
export type ProjectSnapshot = z.infer<typeof projectSnapshotSchema>;

/**
 * Decode a project JSON document. Unknown fields are ignored; an unknown
 * major version (or newer minor) throws `ProtocolVersionUnsupported`.
 */
export function decodeProjectSnapshot(json: string): ProjectSnapshot {
  const raw: unknown = JSON.parse(json);
  if (typeof raw !== "object" || raw === null || !("protocolVersion" in raw)) {
    throw new SyntaxError("project snapshot is missing protocolVersion");
  }
  checkProtocolVersion(String((raw as { protocolVersion: unknown }).protocolVersion));
  return projectSnapshotSchema.parse(raw);
}

/** Validate a snapshot and serialize it as canonical JSON (with trailing LF). */
export function encodeProjectSnapshot(snapshot: unknown): string {
  return canonicalEncode(projectSnapshotSchema.parse(snapshot));
}
