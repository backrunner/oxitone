import { ErrorCode, OxitoneError, projectAudioImportSchema, type ProjectAudioImport } from "@oxitone/protocol";
import { Sample } from "../arrangement/sample.js";
import { parseAuthoring } from "../authoring-validation.js";
import type { Project } from "./project.js";

/** Declarative frozen audio placement. No I/O, decoder, device or VST3 dependency. */
export function importAudio(project: Project, input: ProjectAudioImport): void {
  const edit = parseAuthoring(projectAudioImportSchema, input, "project.importAudio");
  const position = project.beatsToBarBeat(edit.startBeat);
  Sample.fromSpec({ ...edit.sample, id: "smp_import_preflight" });
  if (project.revisionBigInt > 0xffff_ffff_ffff_ffffn - 3n)
    throw new OxitoneError(ErrorCode.InvalidProject, "project revision exhausted");
  const sample = project.importSampleRef({ ...edit.sample, id: "smp_import_preflight" });
  const track = project.addTrack(edit.name);
  project.createSampleClip(track, sample, position, { tempoSync: "off" });
}
