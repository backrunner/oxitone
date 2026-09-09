import { z } from "zod";
import { projectSnapshotSchema } from "./snapshot.js";
import { registerPluginOptionsSchema } from "./plugin.js";
import { transportCommandSchema } from "./commands.js";
import { frameWireSchema } from "./primitives.js";
import { documentMessageSchema, documentRequestSchema } from "./source-daw.js";

const version = { protocolVersion: z.literal("1.0") };
export const PREVIEW_MAX_FRAME_BYTES = 64 * 1024 * 1024;
export const previewFrameSchema = z.discriminatedUnion("type", [
  z.object({ ...version, type: z.literal("document"), message: documentMessageSchema }),
  z.object({ ...version, type: z.literal("snapshot"), snapshot: projectSnapshotSchema,
    assetBaseDir: z.string().min(1), plugins: z.array(registerPluginOptionsSchema).default([]),
    // Optional UI metadata is validated locally by the viewer; it cannot reject valid music.
    pluginUis: z.unknown().optional(),
    allowPlugins: z.enum(["any", "signed-only"]).optional(), hash: z.string().regex(/^[a-f0-9]{64}$/) }),
  z.object({ ...version, type: z.literal("diagnostic"), code: z.string(), message: z.string(), path: z.string().optional() }),
  z.object({ ...version, type: z.literal("status"), state: z.enum(["building", "watching"]) }),
  z.object({ ...version, type: z.literal("transport"), command: transportCommandSchema }),
  z.object({ ...version, type: z.literal("query") }),
  z.object({ ...version, type: z.literal("shutdown") }),
]);
export type PreviewFrame = z.infer<typeof previewFrameSchema>;
export type PreviewSnapshotFrame = Extract<PreviewFrame, { type: "snapshot" }>;

export const previewResponseSchema = z.discriminatedUnion("type", [
  z.object({ ...version, type: z.literal("state"), revision: frameWireSchema.nullable(), seenRevision: frameWireSchema.nullable(),
    cursor: frameWireSchema, audibleFrame: frameWireSchema, playing: z.boolean(),
    xruns: z.number().int().nonnegative(), pluginFaults: z.number().int().nonnegative(),
    tracks: z.number().int().nonnegative(), patterns: z.number().int().nonnegative(), documentProtocolVersion: z.literal("2.0").optional(),
    documentRequests: z.array(documentRequestSchema).max(64).optional() }),
  z.object({ ...version, type: z.literal("rejected"), code: z.string(), message: z.string(), path: z.string().nullable().optional(),
    documentRequests: z.array(documentRequestSchema).max(64).optional() }),
]);
export type PreviewResponse = z.infer<typeof previewResponseSchema>;
