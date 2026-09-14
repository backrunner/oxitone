import { ErrorCode, OxitoneError } from "@oxitone/protocol";
export type { SaveProjectOptions, LoadedProject } from "../project/files.js";
function unavailable(): never {
  throw new OxitoneError(ErrorCode.AssetUnavailable, "Local project directories require Node; serialize project.snapshot() and retain asset bytes in browsers");
}
export const saveProject: typeof import("../project/files.js").saveProject = unavailable;
export const loadProject: typeof import("../project/files.js").loadProject = unavailable;
