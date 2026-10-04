import { ErrorCode, OxitoneError, type ArrangementSingleEdit } from "@oxitone/protocol";
import type { Project } from "./project.js";
import { editPreflight } from "./edit-preflight.js";

function ids(project: Project, kind: ArrangementSingleEdit["kind"]): string[] {
  return (
    kind === "pattern"
      ? project.tracks.flatMap((track) => track.clips)
      : kind === "sample"
        ? project.sampleClips
        : project.automationClips
  ).map((clip) => clip.id);
}

/** All addresses refer to the state before the batch, even after removal or cross-track moves. */
export function arrangeBatch(
  project: Project,
  edits: ArrangementSingleEdit[],
  apply: (p: Project, e: ArrangementSingleEdit) => void,
): void {
  const planned = edits.map((edit) => {
    if (!("clip" in edit)) return { edit };
    const id = ids(project, edit.kind)[edit.clip];
    if (!id) throw new OxitoneError(ErrorCode.EditTargetMissing, "Batch clip no longer exists");
    return { edit, id };
  });
  const run = (target: Project) => {
    for (const { edit, id } of planned) {
      if (id === undefined) apply(target, edit);
      else {
        const clip = ids(target, edit.kind).indexOf(id);
        if (clip < 0) throw new OxitoneError(ErrorCode.EditTargetMissing, "Batch targets a removed clip");
        apply(target, { ...edit, clip } as ArrangementSingleEdit);
      }
    }
  };
  run(editPreflight(project));
  run(project);
}
