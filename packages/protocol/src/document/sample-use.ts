import { z } from "zod";
import { sampleRefSchema } from "../authoring/refs.js";

const index = z.number().int().nonnegative();
export const sampleDestinationSchema = z.discriminatedUnion("kind", [
  z.strictObject({
    kind: z.literal("arrangement"),
    track: index.optional(),
    name: z.string().min(1).max(256).optional(),
    startBeat: z.number().finite().min(0).max(1_000_000_000),
  }),
  z.strictObject({
    kind: z.literal("plugin"),
    owner: z.enum(["channel", "bus"]),
    index,
    slot: index.optional(),
    resource: z.string().min(1).max(128),
  }),
]);
/** Builder-order addresses; no execution IDs are written into authored source. */
export const sampleUseSchema = z.strictObject({
  sample: z.union([index, sampleRefSchema.omit({ id: true }).strict()]),
  destination: sampleDestinationSchema,
});
/** The document owner decodes and copies local audio before constructing SampleUse. */
export const sampleDropSchema = z.strictObject({
  source: z.discriminatedUnion("kind", [
    z.strictObject({ kind: z.literal("file"), path: z.string().min(1).max(4096) }),
    z.strictObject({ kind: z.literal("project"), index }),
  ]),
  destination: sampleDestinationSchema,
});
export type SampleDestination = z.infer<typeof sampleDestinationSchema>;
export type SampleUse = z.infer<typeof sampleUseSchema>;
export type SampleDrop = z.infer<typeof sampleDropSchema>;
