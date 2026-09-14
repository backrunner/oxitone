import { lstat, readFile, rm, stat } from "node:fs/promises";
import { randomUUID } from "node:crypto";
import { dirname, join } from "node:path";
import { ErrorCode, OxitoneError } from "@oxitone/protocol";
import type { SourceOwnership } from "../files/ownership.js";
import { sourceHash } from "../syntax/program.js";
import { readSourceText } from "../files/project-files.js";
import { durableCreate, durableReplace, saveConflict, syncDirectory } from "./save-files.js";
import { recoverStaging, stagingPath } from "./save-staging.js";

export interface SourceSaveFile {
  readonly path: string;
  /** null means absent, distinct from an existing empty UTF-8 file. */
  readonly baselineHash: string | null;
  readonly text: string | null;
}
export interface SaveImage {
  path: string; realPath: string; before: string | null; after: string | null;
  beforeHash: string | null; afterHash: string | null; mode: number;
  stagingId?: string;
}
export interface SaveJournal { version: 2; phase: "prepared" | "committed"; files: SaveImage[] }
export const journalPath = (directory: string): string => join(directory, "journal.json");
const MAX_BYTES = 32 * 1024 * 1024;
export const imageHash = (text: string | null): string | null => text === null ? null : sourceHash(text);

async function readImage(path: string): Promise<string | null> {
  try { return await readSourceText(path); }
  catch (error) { if ((error as NodeJS.ErrnoException).code === "ENOENT") return null; throw error; }
}

export async function prepareImages(files: readonly SourceSaveFile[], ownership: SourceOwnership): Promise<SaveImage[]> {
  if (!files.length || files.length > 64) throw new OxitoneError(ErrorCode.BudgetExceeded, "source save requires 1..64 files");
  const images: SaveImage[] = [];
  for (const file of files) {
    const owned = await ownership.assertWritable(file.path);
    const before = await readImage(owned.realPath);
    if (imageHash(before) !== file.baselineHash) saveConflict("source changed on disk before Save");
    if (before === null && file.text === null) saveConflict("source transaction cannot have two absent images");
    if (images.some(image => image.realPath === owned.realPath)) saveConflict("duplicate source file in save transaction");
    images.push({ ...owned, before, after: file.text, beforeHash: file.baselineHash, afterHash: imageHash(file.text),
      ...(before === null || file.text === null ? { stagingId: randomUUID() } : {}),
      mode: before === null ? 0o600 : (await stat(owned.realPath)).mode & 0o777 });
    checkBudget(images);
  }
  return images;
}
function checkBudget(images: readonly SaveImage[]): void {
  if (images.reduce((bytes, image) => bytes + Buffer.byteLength(image.before ?? "") + Buffer.byteLength(image.after ?? ""), 0) > MAX_BYTES) {
    throw new OxitoneError(ErrorCode.BudgetExceeded, "source transaction preimages and postimages exceed 32 MiB");
  }
}
export async function writeJournal(directory: string, journal: SaveJournal): Promise<void> {
  await durableReplace(journalPath(directory), JSON.stringify(journal));
}
export async function clearJournal(directory: string): Promise<void> {
  await rm(journalPath(directory)); await syncDirectory(directory);
}
export async function readJournal(directory: string): Promise<SaveJournal | undefined> {
  const path = journalPath(directory);
  try {
    const info = await lstat(path);
    if (!info.isFile() || info.nlink !== 1 || info.size > MAX_BYTES * 6 + 65536) saveConflict("invalid source save journal file");
  } catch (error) { if ((error as NodeJS.ErrnoException).code === "ENOENT") return; throw error; }
  const value = JSON.parse(await readFile(path, "utf8")) as SaveJournal;
  if (!value || value.version !== 2 || !["prepared", "committed"].includes(value.phase)
    || !Array.isArray(value.files) || !value.files.length || value.files.length > 64) saveConflict("unsupported source save journal");
  for (const image of value.files) {
    const isText = (text: unknown) => typeof text === "string" || text === null;
    if (!image || typeof image.path !== "string" || typeof image.realPath !== "string"
      || !isText(image.before) || !isText(image.after) || (image.before === null && image.after === null)
      || (image.stagingId !== undefined && (typeof image.stagingId !== "string" || !/^[a-f0-9-]{36}$/.test(image.stagingId)))
      || ((image.before === null || image.after === null) && image.stagingId === undefined)
      || !Number.isInteger(image.mode) || image.mode < 0 || image.mode > 0o777
      || imageHash(image.before) !== image.beforeHash || imageHash(image.after) !== image.afterHash) saveConflict("source recovery image is corrupt");
  }
  if (new Set(value.files.map(image => image.realPath)).size !== value.files.length) saveConflict("duplicate recovery file");
  checkBudget(value.files);
  return value;
}
export async function checkImage(image: SaveImage, ownership: SourceOwnership): Promise<string | null> {
  const owned = image.before === null || image.after === null
    ? await ownership.assertCreationTarget(image.path) : await ownership.assertRecoverySource(image.path);
  if (owned.realPath !== image.realPath) saveConflict("source recovery path was relocated");
  const hash = imageHash(await readImage(owned.realPath));
  if (hash !== image.beforeHash && hash !== image.afterHash) saveConflict("source recovery conflicts with external edits: " + image.path);
  return hash;
}

export async function publishImage(image: SaveImage, text: string | null, currentHash: string | null): Promise<void> {
  if (text === null) { await rm(image.realPath); await syncDirectory(dirname(image.realPath)); }
  else if (currentHash === null) {
    if (!image.stagingId) saveConflict("source creation requires a recoverable staging identity");
    await durableCreate(image.realPath, text, image.mode, stagingPath(image.realPath, image.stagingId));
  }
  else await durableReplace(image.realPath, text, image.mode);
}

/** Prepared restores preimages; committed finishes postimages. Never infer absence from an empty hash. */
export async function recoverJournal(directory: string, ownership: SourceOwnership): Promise<boolean> {
  const journal = await readJournal(directory);
  if (!journal) return false;
  for (const image of journal.files) await recoverStaging(image, ownership);
  for (const image of journal.files) await checkImage(image, ownership);
  for (const image of journal.files) {
    const text = journal.phase === "committed" ? image.after : image.before;
    const current = await checkImage(image, ownership);
    if (current !== imageHash(text)) await publishImage(image, text, current);
  }
  for (const image of journal.files) {
    if (await checkImage(image, ownership) !== (journal.phase === "committed" ? image.afterHash : image.beforeHash)) saveConflict("source changed during recovery");
  }
  await clearJournal(directory);
  return true;
}
