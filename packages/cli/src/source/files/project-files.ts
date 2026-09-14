import { open, readdir, realpath, stat } from "node:fs/promises";
import { join, resolve } from "node:path";
import { ErrorCode, OxitoneError } from "@oxitone/protocol";
import { SourceOwnership } from "./ownership.js";

const excluded = new Set([
  "node_modules",
  ".pnpm",
  ".yarn",
  ".git",
  "dist",
  "target",
  "build",
  "native-generated",
  ".oxitone-source-save",
]);
export const SOURCE_FILE_LIMIT = 8 * 1024 * 1024;
export const SOURCE_PROJECT_LIMIT = 32 * 1024 * 1024;

/** Read UTF-8 source with a byte cap before allocating an unbounded string. */
export async function readSourceText(path: string, limit = SOURCE_FILE_LIMIT): Promise<string> {
  const metadata = await stat(path);
  if (!metadata.isFile() || metadata.size > limit)
    throw new OxitoneError(ErrorCode.BudgetExceeded, "source file exceeds 8 MiB");
  const handle = await open(path, "r");
  try {
    const chunks: Buffer[] = [];
    let total = 0;
    const chunkSize = 64 * 1024;
    while (total <= limit) {
      const chunk = Buffer.allocUnsafe(Math.min(chunkSize, limit + 1 - total));
      const result = await handle.read(chunk, 0, chunk.byteLength, total);
      if (result.bytesRead === 0) break;
      total += result.bytesRead;
      chunks.push(result.bytesRead === chunk.byteLength ? chunk : chunk.subarray(0, result.bytesRead));
      if (total > limit) throw new OxitoneError(ErrorCode.BudgetExceeded, "source file exceeds 8 MiB");
    }
    const bytes = Buffer.concat(chunks, total),
      text = bytes.toString("utf8");
    if (!Buffer.from(text).equals(bytes)) throw new OxitoneError(ErrorCode.DraftInvalid, "source requires valid UTF-8");
    return text;
  } finally {
    await handle.close();
  }
}

export async function projectSourceFiles(
  roots: readonly string[],
): Promise<{ ownership: SourceOwnership; files: Map<string, string> }> {
  const candidates: string[] = [];
  const visit = async (directory: string, root: string): Promise<void> => {
    const entries = await readdir(directory, { withFileTypes: true });
    if (directory !== root && entries.some((entry) => entry.name === "package.json")) return;
    for (const entry of entries) {
      if (excluded.has(entry.name)) continue;
      const path = join(directory, entry.name);
      if (entry.isDirectory()) await visit(path, root);
      else if ((entry.isFile() || entry.isSymbolicLink()) && /(?<!\.d)\.(?:ts|mts)$/.test(entry.name))
        candidates.push(path);
      if (candidates.length > 4096)
        throw new OxitoneError(ErrorCode.BudgetExceeded, "project exceeds 4096 source files");
    }
  };
  for (const input of roots) {
    const root = resolve(input);
    await visit(root, root);
  }
  const paths: string[] = [],
    realPaths = new Set<string>();
  for (const path of new Set(candidates)) {
    try {
      await SourceOwnership.open(roots, [path]);
      const actual = await realpath(path);
      if (!realPaths.has(actual)) {
        paths.push(path);
        realPaths.add(actual);
      }
    } catch (error) {
      if (!OxitoneError.isOxitoneError(error) || error.code !== ErrorCode.EditNotRepresentable) throw error;
    }
  }
  const ownership = await SourceOwnership.open(roots, paths);
  const files = new Map<string, string>();
  let total = 0;
  for (const path of paths) {
    const text = await readSourceText(path);
    total += Buffer.byteLength(text);
    if (total > SOURCE_PROJECT_LIMIT)
      throw new OxitoneError(ErrorCode.BudgetExceeded, "project source text exceeds 32 MiB");
    files.set(path, text);
  }
  return { ownership, files };
}
