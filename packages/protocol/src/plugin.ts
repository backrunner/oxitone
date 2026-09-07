import { z } from "zod";
import { parameterSpecSchema } from "./parameter.js";

/** Expected metadata supplied by a plugin's npm manifest. */
export const pluginManifestSchema = z.object({
  pluginId: z.string().min(1),
  pluginVersion: z.string().min(1),
  abiMajor: z.literal(1),
  abiMinor: z.number().int().nonnegative(),
  minHostVersion: z.string().min(1),
  kind: z.enum(["instrument", "effect"]),
  inputLayout: z.enum(["none", "mono", "stereo"]),
  outputLayout: z.enum(["mono", "stereo"]),
  parameters: z.array(parameterSpecSchema).max(256),
  sidechainInput: z.boolean(),
  reportsTail: z.boolean(),
  maxPolyphony: z.number().int().positive().max(65536).optional(),
});
export type PluginManifest = z.infer<typeof pluginManifestSchema>;

export const registerPluginOptionsSchema = z.object({
  libraryPath: z.string().min(1),
  expectedHash: z.string().regex(/^[a-fA-F0-9]{64}$/).optional(),
  manifest: pluginManifestSchema,
});
export type RegisterPluginOptions = z.infer<typeof registerPluginOptionsSchema>;

export const registeredPluginSchema = z.object({
  pluginId: z.string(),
  pluginVersion: z.string(),
  sha256: z.string().regex(/^[a-f0-9]{64}$/),
});
export type RegisteredPlugin = z.infer<typeof registeredPluginSchema>;

export const pluginDiagnosticsSchema = z.object({
  pluginId: z.string(),
  pluginVersion: z.string(),
  faults: z.number().int().nonnegative(),
});
export type PluginDiagnostics = z.infer<typeof pluginDiagnosticsSchema>;

export const pluginInfoSchema = z.object({
  protocolVersion: z.string(), pluginId: z.string(), pluginVersion: z.string(), abiMajor: z.literal(1),
  kind: z.enum(["instrument", "effect"]), parameters: z.array(parameterSpecSchema),
  stateSchema: z.string().nullable(),
});
export type PluginInfo = z.infer<typeof pluginInfoSchema>;
