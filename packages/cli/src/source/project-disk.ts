import { ErrorCode, OxitoneError, type DocumentView } from "@oxitone/protocol";
import { sourceHash } from "./syntax.js";
import { readSourceText, SOURCE_PROJECT_LIMIT } from "./project-files.js";
import { imageHash, type SourceSaveFile } from "./save-journal.js";

export function projectModified(files: ReadonlyMap<string, string>, disk: ReadonlyMap<string, string>): boolean {
  return files.size !== disk.size || [...files].some(([path, text]) => disk.get(path) !== text);
}

export function projectSaveFiles(files: ReadonlyMap<string, string>, disk: ReadonlyMap<string, string>): SourceSaveFile[] {
  return [...new Set([...files.keys(), ...disk.keys()])].filter(path => files.get(path) !== disk.get(path))
    .map(path => ({ path, text: files.get(path) ?? null, baselineHash: imageHash(disk.get(path) ?? null) }));
}

export function checkProjectText(files: ReadonlyMap<string, string>): void {
  const sizes = [...files.values()].map(text => Buffer.byteLength(text));
  if (files.size > 4096 || sizes.some(size => size > 8 * 1024 * 1024) || sizes.reduce((sum, size) => sum + size, 0) > 32 * 1024 * 1024) {
    throw new OxitoneError(ErrorCode.BudgetExceeded, "project source text budget exceeded");
  }
}

/** Stage all external files before changing any baseline. Conflicting drafts remain intact. */
export async function readProjectDisk(files: ReadonlyMap<string, string>, disk: ReadonlyMap<string, string>, check: () => void) {
  const nextFiles = new Map(files), nextDisk = new Map(disk);
  const conflicts: DocumentView["conflicts"] = [];
  let changed = false;
  let total = 0;
  for (const path of new Set([...disk.keys(), ...files.keys()])) {
    const baseline = disk.get(path);
    let text: string;
    try { text = await readSourceText(path); }
    catch (error) { if (baseline === undefined && (error as NodeJS.ErrnoException).code === "ENOENT") { check(); continue; } throw error; }
    total += Buffer.byteLength(text); if (total > SOURCE_PROJECT_LIMIT) throw new OxitoneError(ErrorCode.BudgetExceeded, "project disk text exceeds 32 MiB"); check();
    if (text === baseline) continue;
    if (files.get(path) !== baseline && files.get(path) !== text) conflicts.push({ path, baseline: baseline ?? "", disk: text, diskHash: sourceHash(text) });
    else { nextFiles.set(path, text); nextDisk.set(path, text); changed = true; }
  }
  checkProjectText(nextFiles);
  checkProjectText(new Map(conflicts.map(item => [item.path, item.disk])));
  return { files: nextFiles, disk: nextDisk, conflicts, changed };
}

/** Resolution is bound to the exact disk version displayed by the client. */
export async function checkConflict(conflict: DocumentView["conflicts"][number] | undefined, diskHash: string): Promise<void> {
  if (!conflict || sourceHash(conflict.disk) !== diskHash || await readSourceText(conflict.path) !== conflict.disk) {
    throw new OxitoneError(ErrorCode.SourceChanged, "disk conflict changed; refresh before resolving it");
  }
}
