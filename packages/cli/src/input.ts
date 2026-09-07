import { readFile, stat } from "node:fs/promises";
import { basename, dirname, resolve } from "node:path";
import { loadProject, type LoadedProject } from "@oxitone/core";
import { decodeProjectSnapshot, ErrorCode, OxitoneError } from "@oxitone/protocol";

/** CLI inputs share the SDK's portable-project validation and resource rules. */
export async function loadInput(input: string): Promise<LoadedProject> {
  const path = resolve(input);
  let contents: string;
  try {
    if ((await stat(path)).isDirectory()) return await loadProject(path);
    if (basename(path) === "oxitone.project.json") return await loadProject(dirname(path));
    contents = await readFile(path, "utf8");
  } catch (error) {
    if (OxitoneError.isOxitoneError(error)) throw error;
    throw new OxitoneError(ErrorCode.AssetUnavailable, `cannot read project input: ${path}`, {
      cause: error, details: { path },
    });
  }
  try {
    return { snapshot: decodeProjectSnapshot(contents), assetBaseDir: dirname(path) };
  } catch (error) {
    if (OxitoneError.isOxitoneError(error)) throw error;
    throw new OxitoneError(ErrorCode.InvalidProject, `invalid project snapshot: ${path}`, {
      cause: error, details: { path },
    });
  }
}
