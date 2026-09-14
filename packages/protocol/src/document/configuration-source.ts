import { z } from "zod";
import { effectRefSchema, instrumentRefSchema } from "../authoring/refs.js";

/** Revision-local projection addresses; never serialized as authoring instance bindings. */
export const configurationUsageSchema = z.object({ handle: z.string(), owner: z.string(), label: z.string(),
  kind: z.enum(["instrument", "channelInsert", "busInsert"]), index: z.number().int().nonnegative() });
export type ConfigurationUsage = z.infer<typeof configurationUsageSchema>;
export const configurationSiteSchema = z.object({ handle: z.string(), fileName: z.string(), expression: z.string(), label: z.string(),
  scope: z.enum(["definition", "reference"]), kind: z.enum(["instrument", "effect"]),
  usages: z.array(configurationUsageSchema).max(100_000), config: instrumentRefSchema.merge(effectRefSchema) });
export type ConfigurationSite = z.infer<typeof configurationSiteSchema>;
export const configurationEditSchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("parameters"), values: z.record(z.string().min(1), z.number().finite()) }),
  z.object({ kind: z.literal("host"), values: z.object({ mix: z.number().finite().min(0).max(1).optional(), bypass: z.boolean().optional() }).strict() }),
]);
export type ConfigurationEdit = z.infer<typeof configurationEditSchema>;
export const effectOwnerSiteSchema = z.object({ handle: z.string(), fileName: z.string(), expression: z.string(), label: z.string(),
  scope: z.enum(["definition", "reference"]), owner: z.string() });
export type EffectOwnerSite = z.infer<typeof effectOwnerSiteSchema>;
export const rackUsageSchema = z.object({ owner: z.string(), label: z.string(), kind: z.enum(["channel", "bus"]) });
export type RackUsage = z.infer<typeof rackUsageSchema>;
export const rackSiteSchema = z.object({ handle: z.string(), fileName: z.string(), expression: z.string(), label: z.string(),
  scope: z.enum(["definition", "reference"]), usages: z.array(rackUsageSchema).max(100_000), effects: z.array(effectRefSchema).max(4096) });
export type RackSite = z.infer<typeof rackSiteSchema>;
export const rackMaterializationReviewSchema = z.object({ planId: z.string(), baseRevision: z.number().int().nonnegative(),
  fileName: z.string(), beforeText: z.string(), afterText: z.string(), affectedOwners: z.array(z.string()), effects: z.number().int().nonnegative(),
  retainsOriginalEvaluation: z.boolean(), retainsDependencyImports: z.literal(true), losesGeneratorLink: z.literal(true) });
export type RackMaterializationReview = z.infer<typeof rackMaterializationReviewSchema>;
