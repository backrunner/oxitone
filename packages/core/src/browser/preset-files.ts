import { ErrorCode, OxitoneError } from "@oxitone/protocol";
export type { LoadedPreset } from "../presets/files.js";
function unavailable(): never {
  throw new OxitoneError(ErrorCode.AssetUnavailable, "Local preset files require Node; use in-memory preset descriptors in browsers");
}
export const savePreset: typeof import("../presets/files.js").savePreset = unavailable;
export const loadPreset: typeof import("../presets/files.js").loadPreset = unavailable;
