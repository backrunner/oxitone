import { z } from "zod";
import { pluginManifestSchema } from "./plugin.js";
import { parameterSpecSchema } from "./parameter.js";

/** Static npm discovery envelope. Its format version is independent of the plugin ABI. */
export const pluginInstallManifestSchema = z.object({
  formatVersion: z.literal(1),
  plugins: z.array(z.object({
    displayName: z.string().min(1).max(256), vendor: z.string().max(256), license: z.string().max(256).optional(),
    manifest: pluginManifestSchema,
    platforms: z.record(z.string(), z.object({ library: z.string().min(1), sha256: z.string().regex(/^[a-f0-9]{64}$/) })).refine(value => Object.keys(value).length <= 32),
  })).min(1).max(256),
});
export type PluginInstallManifest = z.infer<typeof pluginInstallManifestSchema>;
export const pluginCatalogEntrySchema = z.object({
  handle: z.string(), pluginId: z.string(), pluginVersion: z.string(), displayName: z.string(), vendor: z.string(),
  kind: z.enum(["instrument", "effect", "unknown"]), source: z.enum(["builtin", "package", "project"]),
  packageName: z.string().optional(), packageVersion: z.string().optional(), license: z.string().optional(),
  availability: z.enum(["available", "missing", "unsupported", "invalid"]),
  validation: z.enum(["unverified", "verified", "failed"]), diagnostic: z.string().optional(),
  libraryPath: z.string().optional(), sha256: z.string().optional(), parameters: z.array(parameterSpecSchema).max(256),
  usages: z.array(z.object({ kind: z.enum(["instrument", "channelInsert", "busInsert"]), owner: z.string(), instanceId: z.string().optional(), index: z.number().int().nonnegative(), label: z.string() })).max(100_000),
});
export type PluginCatalogEntry = z.infer<typeof pluginCatalogEntrySchema>;
