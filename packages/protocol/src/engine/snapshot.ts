import { z } from "zod";
import {
  automationLaneSpecSchema,
  automationClipSpecSchema,
  channelSpecSchema,
  mixerChannelSpecSchema,
  patternClipSpecSchema,
  patternSpecSchema,
  sampleClipSpecSchema,
} from "../authoring/specs.js";
import { canonicalEncode } from "../base/canonical.js";
import { entityIdSchema, frameWireSchema } from "../base/primitives.js";
import { sampleRefSchema, trackSpecSchema } from "../authoring/refs.js";
import { markerSpecSchema, tempoSegmentSchema, timeSignatureSegmentSchema } from "../authoring/timeline.js";
import { checkProtocolVersion } from "../base/version.js";
import { ErrorCode, OxitoneError } from "../base/errors.js";

/** Immutable, fully validated project state exchanged with the Rust engine. */
const snapshotShape = z.object({
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
  automationClips: z.array(automationClipSpecSchema).optional(),
});
export type ProjectSnapshot = z.infer<typeof snapshotShape>;

function checkSnapshotVersion(snapshot: ProjectSnapshot): void {
  checkProtocolVersion(snapshot.protocolVersion);
  if (Number(snapshot.protocolVersion.split(".")[1]) < 2 &&
    (snapshot.patterns.some(pattern => pattern.parts !== undefined) || snapshot.tracks.some(track => track.mute !== undefined || track.solo !== undefined) || snapshot.automationClips !== undefined || snapshot.automation.some(lane => lane.playback !== undefined))) {
    throw new OxitoneError(ErrorCode.ProtocolVersionUnsupported, "Pattern parts, Track mute/solo and Playlist automation require protocol 1.2");
  }
  const hasInstances = snapshot.channels.some(channel => channel.instrument.instanceId !== undefined || channel.effectChain.some(ref => ref.instanceId !== undefined)) ||
    snapshot.mixerChannels.some(bus => bus.inserts.some(ref => ref.instanceId !== undefined));
  if (Number(snapshot.protocolVersion.split(".")[1]) < 1 && (hasInstances || snapshot.automation.some(lane => lane.target.scope !== undefined))) {
    throw new OxitoneError(ErrorCode.ProtocolVersionUnsupported, "plugin instances and scoped automation require engine protocol 1.1", { details: { path: "$.protocolVersion" } });
  }
}
export const projectSnapshotSchema = snapshotShape.superRefine((snapshot, context) => {
  try { checkSnapshotVersion(snapshot); }
  catch (error) { context.addIssue({ code: "custom", path: ["protocolVersion"], message: (error as Error).message }); }
});

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
  const snapshot = snapshotShape.parse(raw);
  checkSnapshotVersion(snapshot);
  return snapshot;
}

/** Validate a snapshot and serialize it as canonical JSON (with trailing LF). */
export function encodeProjectSnapshot(snapshot: unknown): string {
  const parsed = snapshotShape.parse(snapshot);
  checkSnapshotVersion(parsed);
  return canonicalEncode(parsed);
}
