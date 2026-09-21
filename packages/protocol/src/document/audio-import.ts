import { z } from "zod";
import { sampleRefSchema } from "../authoring/refs.js";

/** A frozen audio resource and a new Playlist track, expressed without execution IDs. */
export const projectAudioImportSchema = z.strictObject({
  name: z.string().min(1).max(256),
  startBeat: z.number().finite().min(0).max(1_000_000_000),
  sample: sampleRefSchema.omit({ id: true }).strict(),
});
export type ProjectAudioImport = z.infer<typeof projectAudioImportSchema>;
