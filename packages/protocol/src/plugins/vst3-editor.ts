import { z } from "zod";
import { vst3InfoSchema } from "./vst3.js";
import { vst3ConfigurationOptionsSchema } from "./vst3-configuration.js";

/** Opens an isolated native configuration editor. It never opens an audio device. */
export const vst3EditorOptionsSchema = vst3ConfigurationOptionsSchema;
export type Vst3EditorOptions = z.input<typeof vst3EditorOptionsSchema>;
export const vst3EditorResultSchema = z.discriminatedUnion("accepted", [
  z.strictObject({ protocolVersion: z.literal(1), accepted: z.literal(false) }),
  z.strictObject({ protocolVersion: z.literal(1), accepted: z.literal(true), info: vst3InfoSchema }),
]);
