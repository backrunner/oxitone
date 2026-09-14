import { ErrorCode, OxitoneError } from "@oxitone/protocol";
export type * from "./index.js";
function unavailable(): never {
  throw new OxitoneError(
    ErrorCode.DeviceUnavailable,
    "This API requires the native host; use WasmEngine or WebAudioSession from @oxitone/web in a browser",
  );
}
export function getProtocolVersion(): string {
  return "1.0";
}
export const resolveBeatDuration: typeof import("./index.js").resolveBeatDuration = unavailable;
export const inspectSample: typeof import("./index.js").inspectSample = unavailable;
export const cacheSample: typeof import("./index.js").cacheSample = unavailable;
export const createEngine: typeof import("./index.js").createEngine = unavailable;
export const registerPlugin: typeof import("./index.js").registerPlugin = unavailable;
export const getPluginDiagnostics: typeof import("./index.js").getPluginDiagnostics = unavailable;
export const getPluginInfo: typeof import("./index.js").getPluginInfo = unavailable;
export const compile: typeof import("./index.js").compile = unavailable;
export const dispose: typeof import("./index.js").dispose = unavailable;
export const exportMidi: typeof import("./index.js").exportMidi = unavailable;
export const renderWav: typeof import("./index.js").renderWav = unavailable;
export const enqueueTransport: typeof import("./index.js").enqueueTransport = unavailable;
export const setParameter: typeof import("./index.js").setParameter = unavailable;
export const listOutputDevices: typeof import("./index.js").listOutputDevices = unavailable;
export const getOutputLatency: typeof import("./index.js").getOutputLatency = unavailable;
export const getDiagnostics: typeof import("./index.js").getDiagnostics = unavailable;
