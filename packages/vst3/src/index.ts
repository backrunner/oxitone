export { inspectVst3Plugin } from "./inspect.js";
export { editVst3Plugin } from "./editor.js";
export { configureVst3Plugin } from "./configure.js";
export type { Vst3ConfigurationOptions } from "@oxitone/protocol";
export type { Vst3EditorOptions } from "@oxitone/protocol";
export { scanVst3Bundles, listVst3Classes } from "./discovery.js";
export type { Vst3BundleSource, Vst3ClassList } from "@oxitone/protocol";
export { renderVst3Wav } from "./render.js";
export { registerVst3Plugin, vst3Config } from "./project.js";
export { executable as resolveVst3Host } from "./helper.js";
export type { RegisterVst3Options, RegisteredVst3 } from "@oxitone/protocol";
export { parseVst3Preset, loadVst3Preset, saveVst3Preset } from "./preset.js";
export type { Vst3Preset } from "@oxitone/protocol";
export type { Vst3StreamStart, Vst3StreamReady, Vst3StreamSchedule, Vst3StreamManager } from "@oxitone/protocol";
export type { Vst3Transport } from "@oxitone/protocol";
export type { Vst3HostOptions } from "./helper.js";
export type {
  Vst3Source,
  Vst3Info,
  Vst3Configuration,
  Vst3Event,
  Vst3RenderOptions,
  Vst3RenderReport,
} from "@oxitone/protocol";
