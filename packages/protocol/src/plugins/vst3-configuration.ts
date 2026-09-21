import { z } from "zod";
import { vst3RenderOptionsSchema } from "./vst3.js";

/** Restore and flush an isolated configuration, without audio output or an editor window. */
export const vst3ConfigurationOptionsSchema = vst3RenderOptionsSchema.pick({
  sampleRate: true,
  blockSize: true,
  configuration: true,
  parameters: true,
});
export type Vst3ConfigurationOptions = z.input<typeof vst3ConfigurationOptionsSchema>;
