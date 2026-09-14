import { stat } from "node:fs/promises";
import { dirname, join } from "node:path";
import { captureSourceReads, type SourceRead } from "../files/read-set.js";

/** Existing ancestor resolution inputs. Nonstandard TS config extends and arbitrary assets use readPaths. */
export async function captureResolutionReads(paths: readonly string[]): Promise<SourceRead[]> {
  const directories = new Set<string>();
  for (const path of paths) for (let parent = dirname(path); ; parent = dirname(parent)) {
    directories.add(parent); if (parent === dirname(parent)) break;
  }
  const files: string[] = [];
  for (const directory of directories) for (const name of ["package.json", "tsconfig.json", "pnpm-lock.yaml", "package-lock.json", "yarn.lock"]) {
    const path = join(directory, name);
    try { if ((await stat(path)).isFile()) files.push(path); }
    catch (error) { if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error; }
  }
  return captureSourceReads(files);
}
