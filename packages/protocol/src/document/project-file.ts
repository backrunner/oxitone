import { z } from "zod";
import { projectSnapshotSchema } from "../engine/snapshot.js";
import { entityIdSchema } from "../base/primitives.js";

/** Portable on-disk format version, independent from the native protocol version. */
export const PROJECT_FORMAT_VERSION = "1.0";
export const projectFileSchema = projectSnapshotSchema
  .safeExtend({
    formatVersion: z.literal(PROJECT_FORMAT_VERSION),
    projectId: entityIdSchema,
  })
  .refine((document) => document.projectId === document.id, {
    message: "projectId must equal snapshot id",
    path: ["projectId"],
  });
export type ProjectFile = z.infer<typeof projectFileSchema>;
