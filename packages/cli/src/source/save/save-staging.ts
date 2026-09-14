import { lstat, rm } from "node:fs/promises";
import { dirname, join } from "node:path";
import type { SourceOwnership } from "../files/ownership.js";
import type { SaveImage } from "./save-journal.js";
import { readSourceText } from "../files/project-files.js";
import { saveConflict, syncDirectory } from "./save-files.js";

export const stagingPath = (path: string, id: string): string => join(dirname(path), `.oxitone-save-${id}.tmp`);

/** A kill between link and unlink leaves two names for the same complete inode. */
export async function recoverStaging(image: SaveImage, ownership: SourceOwnership): Promise<void> {
  if (!image.stagingId) return;
  const owned = await ownership.assertCreationParent(image.path);
  if (owned.realPath !== image.realPath) saveConflict("source staging parent was relocated");
  const path = stagingPath(image.realPath, image.stagingId);
  let staged;
  try {
    staged = await lstat(path);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return;
    throw error;
  }
  if (!staged.isFile() || staged.nlink > 2) saveConflict("source staging file has unknown links");
  if (staged.nlink === 2) {
    const target = await lstat(image.realPath);
    if (!target.isFile() || target.dev !== staged.dev || target.ino !== staged.ino)
      saveConflict("source staging alias was changed");
    const text = await readSourceText(path);
    if (text !== image.before && text !== image.after) saveConflict("source staging content was changed");
  }
  // With one link this is an unpublished temporary, possibly killed during write.
  await rm(path);
  await syncDirectory(dirname(path));
}
