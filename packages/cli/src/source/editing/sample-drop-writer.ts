import { lstat, mkdir, realpath } from "node:fs/promises";
import { basename, join, relative, resolve } from "node:path";
import { cacheSample } from "@oxitone/native";
import { ErrorCode, OxitoneError, sampleDropSchema, type SampleDrop, type SampleUse } from "@oxitone/protocol";
import type { ProjectEvaluation } from "../eval/project-evaluation.js";
import { appendProjectEdit, restoreProject } from "./project-edit-writer.js";

/** Decode with Rust, freeze an owned asset, then stage one reversible source edit. */
export async function prepareSampleDrop(
  before: ProjectEvaluation,
  files: ReadonlyMap<string, string>,
  root: string,
  input: SampleDrop,
  check: () => void,
) {
  const drop = sampleDropSchema.parse(input);
  let sample: SampleUse["sample"];
  if (drop.source.kind === "project") sample = drop.source.index;
  else {
    const path = drop.source.path;
    const stat = await lstat(path);
    if (!stat.isFile() || stat.size > 256 * 1024 * 1024)
      throw new OxitoneError(ErrorCode.AssetUnavailable, "Choose a regular audio file up to 256 MiB");
    let directory = await realpath(root);
    for (const part of ["assets", "samples"]) {
      directory = join(directory, part);
      await mkdir(directory).catch((error: NodeJS.ErrnoException) => {
        if (error.code !== "EEXIST") throw error;
      });
      if (!(await lstat(directory)).isDirectory())
        throw new OxitoneError(ErrorCode.AssetUnavailable, "Sample asset directories cannot contain symlinks");
    }
    check();
    const cached = cacheSample(path, directory);
    const after = await lstat(path);
    if (stat.ino !== after.ino || stat.size !== after.size || stat.mtimeMs !== after.mtimeMs)
      throw new OxitoneError(ErrorCode.SourceChanged, "Sample changed during import; try again");
    sample = {
      assetUri: relative(await realpath(resolve(before.frame.assetBaseDir ?? root)), cached.path).replaceAll("\\", "/"),
      sha256: cached.sha256,
      format: "wav",
      sampleRate: cached.sampleRate,
      channels: cached.channels,
      frames: cached.frames,
      provenance: cached.provenance,
    };
    if (drop.destination.kind === "arrangement" && drop.destination.track === undefined)
      drop.destination.name ??= basename(path);
  }
  check();
  const edit: SampleUse = { sample, destination: drop.destination };
  const project = restoreProject(before);
  project.useSample(edit);
  return { files: appendProjectEdit(before, files, "useSample", edit).files, expected: project.snapshot() };
}
