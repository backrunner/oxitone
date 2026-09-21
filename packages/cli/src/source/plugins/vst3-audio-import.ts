import { lstat, mkdir, realpath } from "node:fs/promises";
import { join, relative, resolve } from "node:path";
import { cacheSample } from "@oxitone/native";
import { ErrorCode, OxitoneError, type PluginCatalogEntry, type ProjectAudioImport } from "@oxitone/protocol";
import type { ProjectEvaluation } from "../eval/project-evaluation.js";
import { appendProjectEdit, restoreProject } from "../editing/project-edit-writer.js";

/** Freeze a verified render into owned content-addressed assets before a source transaction. */
export async function prepareVst3AudioImport(
  before: ProjectEvaluation,
  files: ReadonlyMap<string, string>,
  root: string,
  entry: PluginCatalogEntry | undefined,
  name: string,
  startBeat: number,
  check: () => void,
) {
  const report = entry?.vst3?.render;
  if (!report)
    throw new OxitoneError(ErrorCode.EditTargetMissing, "Render this VST3 plugin before adding audio to the project");
  const source = await lstat(report.path);
  if (!source.isFile() || source.size > 256 * 1024 * 1024)
    throw new OxitoneError(ErrorCode.BudgetExceeded, "Audio import requires a regular WAV up to 256 MiB");
  let directory = await realpath(root);
  for (const part of ["assets", "vst3"]) {
    directory = join(directory, part);
    try {
      await mkdir(directory);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
    }
    if (!(await lstat(directory)).isDirectory())
      throw new OxitoneError(ErrorCode.AssetUnavailable, "Project VST3 asset directory cannot contain symlinks");
  }
  check();
  const cached = cacheSample(report.path, directory);
  if (
    cached.provenance.sourceSha256 !== report.sha256 ||
    cached.frames !== String(report.frames) ||
    cached.sampleRate !== report.sampleRate ||
    cached.channels !== 2
  )
    throw new OxitoneError(
      ErrorCode.SourceChanged,
      "VST3 output changed after rendering; render again before importing",
    );
  check();
  const edit: ProjectAudioImport = {
    name,
    startBeat,
    sample: {
      assetUri: relative(await realpath(resolve(before.frame.assetBaseDir ?? root)), cached.path).replaceAll("\\", "/"),
      sha256: cached.sha256,
      format: "wav",
      sampleRate: cached.sampleRate,
      channels: cached.channels,
      frames: cached.frames,
      provenance: cached.provenance,
    },
  };
  const project = restoreProject(before);
  project.importAudio(edit);
  return { files: appendProjectEdit(before, files, "importAudio", edit).files, expected: project.snapshot() };
}
