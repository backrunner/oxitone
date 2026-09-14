import { z } from "zod";
import { arrangementEditSchema } from "./arrangement.js";
import { projectEditSchema } from "./project-edit.js";
import { noteEditSchema, noteSelectorSchema, sourceNoteSchema } from "../authoring/pattern-source.js";
import { pluginCatalogEntrySchema } from "../plugins/plugin-catalog.js";
import { automationSourceSchema } from "../authoring/automation-source.js";
import { curveSchema } from "../authoring/curve.js";
import { configurationEditSchema, configurationSiteSchema, effectOwnerSiteSchema, rackSiteSchema, rackMaterializationReviewSchema } from "./configuration-source.js";

/** Document control is versioned separately from the existing engine snapshot protocol. */
export const DOCUMENT_PROTOCOL_VERSION = "2.0";
export const DOCUMENT_LIMITS = { requests: 64, retainedResults: 256, sites: 4096, frameBytes: 64 * 1024 * 1024 } as const;
export const arrangementOrderSchema = z.object({ patterns: z.array(z.string()), tracks: z.array(z.string()), channels: z.array(z.string()), mixerChannels: z.array(z.string()),
  samples: z.array(z.string()), automation: z.array(z.string()), patternClips: z.array(z.string()), sampleClips: z.array(z.string()), automationClips: z.array(z.string()).default([]) });
const revision = z.number().int().min(0).max(Number.MAX_SAFE_INTEGER);
const handle = z.string().min(1).max(256);
export const patternSiteSchema = z.object({
  handle, patternId: handle, fileName: z.string(), start: revision, end: revision,
  expression: z.string(), label: z.string(), scope: z.enum(["definition", "reference"]),
  invocations: z.number().int().positive(), references: z.number().int().nonnegative(),
  placements: z.array(handle).max(100_000),
  outputs: z.array(z.object({ note: sourceNoteSchema, select: noteSelectorSchema })).max(100_000),
});
export type PatternSite = z.infer<typeof patternSiteSchema>;
export const automationSiteSchema = z.object({ handle, fileName: z.string(), expression: z.string(), label: z.string(),
  scope: z.enum(["definition", "reference"]), lanes: z.array(handle).max(100_000), clips: z.array(handle).max(100_000), source: automationSourceSchema });
export type AutomationSite = z.infer<typeof automationSiteSchema>;
export const automationRangeEditSchema = z.object({ start: z.number().finite().nonnegative(), end: z.number().finite().positive(), fadeBeats: z.number().finite().nonnegative().optional(),
  points: z.array(z.object({ beat: z.number().finite().nonnegative(), value: z.number().finite().min(0).max(1), curve: curveSchema.optional() })).min(1).max(4096) });
export type AutomationRangeEdit = z.infer<typeof automationRangeEditSchema>;
export const materializationReviewSchema = z.object({
  planId: handle, baseRevision: revision, fileName: z.string(), beforeText: z.string(), afterText: z.string(),
  affectedClips: z.array(handle).max(100_000), beforeNotes: revision, afterNotes: revision,
  losesGeneratorLink: z.literal(true), retainsDependencyImports: z.literal(true),
  retainsOriginalEvaluation: z.boolean(),
});
export type MaterializationReview = z.infer<typeof materializationReviewSchema>;
export const documentViewSchema = z.object({
  projectRoot: z.string().min(1),
  arrangementOrder: arrangementOrderSchema.optional(),
  sessionId: handle, revision, acceptedRevision: z.number().int().min(-1), savedRevision: z.number().int().min(-1),
  status: z.enum(["building", "ready", "invalid", "conflict", "closed"]), modified: z.boolean(), saving: z.boolean(),
  sites: z.array(patternSiteSchema).max(DOCUMENT_LIMITS.sites),
  automationSites: z.array(automationSiteSchema).max(DOCUMENT_LIMITS.sites),
  configurationSites: z.array(configurationSiteSchema).max(DOCUMENT_LIMITS.sites),
  effectOwnerSites: z.array(effectOwnerSiteSchema).max(DOCUMENT_LIMITS.sites).optional(),
  rackSites: z.array(rackSiteSchema).max(DOCUMENT_LIMITS.sites),
  rackMaterialization: rackMaterializationReviewSchema.optional(),
  files: z.array(z.object({ path: z.string(), text: z.string() })).max(4096),
  conflicts: z.array(z.object({ path: z.string(), baseline: z.string(), disk: z.string(), diskHash: z.string().length(64) })).max(4096),
  materialization: materializationReviewSchema.optional(),
  plugins: z.array(pluginCatalogEntrySchema).max(4096),
  diagnostic: z.object({ code: z.string(), message: z.string() }).optional(),
});
export type DocumentView = z.infer<typeof documentViewSchema>;
const metadata = { documentProtocolVersion: z.literal(DOCUMENT_PROTOCOL_VERSION), sessionId: handle, requestId: handle, baseRevision: revision };
export const documentRequestSchema = z.object({ ...metadata, operation: z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("project"), edit: projectEditSchema }),
  z.object({ kind: z.literal("assignPlugin"), plugin: z.string(), owner: z.string(),
    target: z.enum(["instrument", "channelInsert", "busInsert"]), instance: z.string().optional() }),
  z.object({ kind: z.literal("arrangement"), edit: arrangementEditSchema }),
  z.object({ kind: z.literal("notes"), site: handle, placement: handle.optional(), edits: z.array(noteEditSchema).max(100_000) }),
  z.object({ kind: z.literal("automationRange"), site: handle, lane: handle.optional(), clip: handle.optional(), edit: automationRangeEditSchema }),
  z.object({ kind: z.literal("configuration"), site: handle, usage: handle.optional(), edit: configurationEditSchema }),
  z.object({ kind: z.literal("effectOrder"), site: handle, owner: handle, order: z.array(handle).max(4096) }),
  z.object({ kind: z.literal("planMaterializeRack"), site: handle, owner: handle.optional() }),
  z.object({ kind: z.literal("planMaterialize"), site: handle, placement: handle.optional(), edits: z.array(noteEditSchema).max(100_000) }),
  z.object({ kind: z.literal("confirmMaterialize"), planId: handle }),
  z.object({ kind: z.literal("cancelMaterialize"), planId: handle }),
  z.object({ kind: z.literal("code"), fileName: z.string(), text: z.string().max(8 * 1024 * 1024) }),
  z.object({ kind: z.literal("createFile"), fileName: z.string(), text: z.string().max(8 * 1024 * 1024) }),
  z.object({ kind: z.literal("resolveConflict"), fileName: z.string(), diskHash: z.string().length(64), resolution: z.enum(["use-disk", "keep-draft", "merge"]) }),
  z.object({ kind: z.literal("undo") }), z.object({ kind: z.literal("redo") }), z.object({ kind: z.literal("save") }),
  z.object({ kind: z.literal("query") }),
  z.object({ kind: z.literal("refreshPlugins") }), z.object({ kind: z.literal("verifyPlugin"), plugin: handle }),
  z.object({ kind: z.literal("installPlugin"), packageName: z.string().min(1).max(256), version: z.string().min(1).max(128).optional() }),
  z.object({ kind: z.literal("upgradePlugin"), packageName: z.string().min(1).max(256), version: z.string().min(1).max(128).optional() }),
  z.object({ kind: z.literal("uninstallPlugin"), packageName: z.string().min(1).max(256) }),
  z.object({ kind: z.literal("repairPlugin"), packageName: z.string().min(1).max(256) }),
]) });
export type DocumentRequest = z.infer<typeof documentRequestSchema>;
export const documentMessageSchema = z.discriminatedUnion("type", [
  z.object({ documentProtocolVersion: z.literal(DOCUMENT_PROTOCOL_VERSION), type: z.literal("event"), view: documentViewSchema }),
  z.object({ documentProtocolVersion: z.literal(DOCUMENT_PROTOCOL_VERSION), type: z.literal("response"), sessionId: handle, requestId: handle,
    accepted: z.boolean(), revision, error: z.object({ code: z.string(), message: z.string() }).optional() }),
]);
export type DocumentMessage = z.infer<typeof documentMessageSchema>;
