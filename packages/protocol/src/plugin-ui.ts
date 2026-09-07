import { z } from "zod";

const label = z.string().min(1).max(64).regex(/^[^\p{Cc}]+$/u);
const id = z.string().min(1).max(128).regex(/^[^\p{Cc}]+$/u);
const binding = { parameter: id, label: label.optional() };
/** Native, read-only controls. Parameter semantics always come from the DSP descriptor. */
export const pluginUiControlSchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("knob"), ...binding }).strict(),
  z.object({ kind: z.literal("fader"), ...binding }).strict(),
  z.object({ kind: z.literal("toggle"), ...binding }).strict(),
  z.object({ kind: z.literal("readout"), ...binding }).strict(),
  z.object({ kind: z.literal("choice"), ...binding,
    options: z.array(z.object({ value: z.number().finite(), label }).strict()).min(2).max(16) }).strict(),
  z.object({ kind: z.literal("envelope"), label: label.optional(),
    attack: id, decay: id, sustain: id, release: id }).strict(),
]);
export const pluginUiManifestSchema = z.object({
  uiVersion: z.literal("1.0"), pluginId: id, pluginVersion: id,
  title: label,
  size: z.object({ width: z.number().int().min(440).max(1200), height: z.number().int().min(280).max(900) }).strict(),
  pages: z.array(z.object({ id, title: label,
    groups: z.array(z.object({ id, title: label, columns: z.number().int().min(1).max(6),
      controls: z.array(pluginUiControlSchema).min(1).max(32) }).strict()).min(1).max(16),
  }).strict()).min(1).max(8),
}).strict();
export type PluginUiManifest = z.infer<typeof pluginUiManifestSchema>;
export type PluginUiControl = z.infer<typeof pluginUiControlSchema>;
