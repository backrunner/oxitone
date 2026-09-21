import { z } from "zod";
import { pluginManifestSchema } from "./plugin.js";
import { parameterSpecSchema } from "../authoring/parameter.js";
import { vst3ClassIdSchema, vst3InfoSchema, vst3RenderReportSchema } from "./vst3.js";

export const vst3CatalogSchema = z.object({
  bundlePath: z.string().min(1),
  classId: vst3ClassIdSchema,
  inputChannels: z.number().int().min(0).max(2).optional(),
  outputChannels: z.number().int().min(0).max(2).optional(),
  noteInput: z.boolean().optional(),
  parameters: vst3InfoSchema.shape.parameters,
  render: vst3RenderReportSchema.optional(),
  presetPath: z.string().optional(),
});
export type Vst3Catalog = z.infer<typeof vst3CatalogSchema>;

const vst3PackagePluginSchema = z.object({
  pluginId: z.string().min(1).max(256),
  pluginVersion: z.string().min(1).max(128),
  displayName: z.string().min(1).max(256),
  vendor: z.string().max(256),
  license: z.string().max(256).optional(),
  kind: z.enum(["instrument", "effect"]),
  bundle: z.string().min(1).max(4096),
  classId: vst3ClassIdSchema,
  sha256: z
    .string()
    .regex(/^[a-f0-9]{64}$/)
    .optional(),
});
export type Vst3PackagePlugin = z.infer<typeof vst3PackagePluginSchema>;

/** Static npm discovery envelope. Its format version is independent of the plugin ABI. */
export const pluginInstallManifestSchema = z
  .object({
    formatVersion: z.literal(1),
    vst3: z.array(vst3PackagePluginSchema).max(256).optional(),
    plugins: z
      .array(
        z.object({
          displayName: z.string().min(1).max(256),
          vendor: z.string().max(256),
          license: z.string().max(256).optional(),
          manifest: pluginManifestSchema,
          platforms: z
            .record(z.string(), z.object({ library: z.string().min(1), sha256: z.string().regex(/^[a-f0-9]{64}$/) }))
            .refine((value) => Object.keys(value).length <= 32),
        }),
      )
      .max(256)
      .default([]),
  })
  .refine((value) => value.plugins.length > 0 || (value.vst3?.length ?? 0) > 0, "manifest must declare a plugin");
export type PluginInstallManifest = z.infer<typeof pluginInstallManifestSchema>;
export const pluginCatalogEntrySchema = z.object({
  handle: z.string(),
  pluginId: z.string(),
  pluginVersion: z.string(),
  displayName: z.string(),
  vendor: z.string(),
  kind: z.enum(["instrument", "effect", "unknown"]),
  source: z.enum(["builtin", "package", "project", "vst3"]),
  packageName: z.string().optional(),
  packageVersion: z.string().optional(),
  license: z.string().optional(),
  availability: z.enum(["available", "missing", "unsupported", "invalid"]),
  validation: z.enum(["unverified", "verified", "failed"]),
  diagnostic: z.string().optional(),
  libraryPath: z.string().optional(),
  sha256: z.string().optional(),
  parameters: z.array(parameterSpecSchema).max(256),
  vst3: vst3CatalogSchema.optional(),
  usages: z
    .array(
      z.object({
        kind: z.enum(["instrument", "channelInsert", "busInsert"]),
        owner: z.string(),
        instanceId: z.string().optional(),
        index: z.number().int().nonnegative(),
        label: z.string(),
      }),
    )
    .max(100_000),
});
export type PluginCatalogEntry = z.infer<typeof pluginCatalogEntrySchema>;
