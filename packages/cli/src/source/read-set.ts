import { lstat, realpath } from "node:fs/promises";
import { createReadStream } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { ErrorCode, OxitoneError } from "@oxitone/protocol";
import { createHash } from "node:crypto";
import { sourceSpan } from "./source-timing.js";

export interface SourceRead { readonly path: string; readonly realPath: string; readonly sha256: string | null }

/** Capture a missing enrolled module; later appearance invalidates the evaluation. */
export async function captureMissingSource(path: string): Promise<SourceRead> {
  const realPath = join(await realpath(dirname(path)), basename(path));
  try { await lstat(path); }
  catch (error) { if ((error as NodeJS.ErrnoException).code === "ENOENT") return { path, realPath, sha256: null }; throw error; }
  throw new OxitoneError(ErrorCode.SourceChanged, "missing source appeared during evaluation");
}

async function digest(path: string): Promise<string> {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest("hex");
}

async function captureSource(path: string): Promise<SourceRead> {
  return { path, realPath: await realpath(path), sha256: await digest(path) };
}

/** The evaluator must list every module, lockfile and asset used to build its candidate. */
export async function captureSourceReads(paths: readonly string[]): Promise<SourceRead[]> {
  if (paths.length > 4096) throw new OxitoneError(ErrorCode.BudgetExceeded, "evaluation read set exceeds 4096 files");
  const unique = [...new Set(paths.map((path) => resolve(path)))];
  const results: SourceRead[] = [];
  for (let start = 0; start < unique.length; start += 16) {
    const batch = await Promise.allSettled(unique.slice(start, start + 16).map(captureSource));
    for (const result of batch) {
      if (result.status === "rejected") throw result.reason;
      results.push(result.value);
    }
  }
  return results;
}

export async function checkSourceReads(reads: readonly SourceRead[]): Promise<void> {
  const done = sourceSpan("read-validation");
  try {
    if (reads.length > 4096) throw new OxitoneError(ErrorCode.BudgetExceeded, "evaluation read set exceeds 4096 files");
    for (let start = 0; start < reads.length; start += 16) {
      // Drain a bounded batch even on failure; report the first failure in read-set order.
      const batch = await Promise.allSettled(reads.slice(start, start + 16).map(checkSource));
      for (const result of batch) if (result.status === "rejected") throw result.reason;
    }
  } finally { done(); }
}

async function checkSource(previous: SourceRead): Promise<void> {
  let current: SourceRead;
  try { current = await (previous.sha256 === null ? captureMissingSource(previous.path) : captureSource(previous.path)); }
  catch (cause) { throw new OxitoneError(ErrorCode.SourceChanged, "evaluation dependency is no longer readable", { details: { path: previous.path }, cause }); }
  if (current.realPath !== previous.realPath || current.sha256 !== previous.sha256) {
    throw new OxitoneError(ErrorCode.SourceChanged, "evaluation dependency changed", { details: { path: previous.path } });
  }
}
