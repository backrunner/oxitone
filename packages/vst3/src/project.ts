import { resolve } from "node:path";
import {
  ErrorCode,
  OxitoneError,
  vst3ConfigurationSchema,
  vst3ParametersSchema,
  type RegisterVst3Options,
  type RegisteredVst3,
  type Vst3Source,
  type Vst3Configuration,
  type InstrumentRef,
} from "@oxitone/protocol";
import { executable, type Vst3HostOptions } from "./helper.js";
import { inspectVst3Plugin } from "./inspect.js";

/** Inspect and register one exact class for Project.compile(), playback and project WAV export. */
export async function registerVst3Plugin(
  project: { registerVst3(options: RegisterVst3Options): RegisteredVst3 },
  source: Vst3Source,
  host: Vst3HostOptions = {},
): Promise<RegisteredVst3> {
  const helperPath = await executable(host);
  const metadata = await inspectVst3Plugin(source, { ...host, hostPath: helperPath });
  host.signal?.throwIfAborted();
  return project.registerVst3({
    registrationVersion: 1,
    source: { ...source, bundlePath: resolve(source.bundlePath), expectedHash: metadata.sha256 },
    helperPath,
    metadata: { ...metadata, configuration: null },
  });
}

/** Serializable configuration accepted by a channel instrument or any effect chain. */
export function vst3Config(
  plugin: RegisteredVst3,
  options: { configuration?: Vst3Configuration; parameters?: Record<string, number> } = {},
): InstrumentRef {
  const state = options.configuration === undefined ? undefined : vst3ConfigurationSchema.parse(options.configuration);
  if (state && (state.sha256 !== plugin.sha256 || `vst3.${state.classId.toLowerCase()}` !== plugin.pluginId))
    throw new OxitoneError(ErrorCode.PluginConfigInvalid, "VST3 configuration belongs to a different binary/class");
  const parameters = vst3ParametersSchema.parse(options.parameters ?? {});
  for (const id of Object.keys(parameters))
    if (!plugin.parameters.some((parameter) => parameter.id === id) && !Object.hasOwn(state?.parameters ?? {}, id))
      throw new OxitoneError(ErrorCode.PluginConfigInvalid, `Unknown or read-only VST3 parameter ${id}`);
  return { pluginId: plugin.pluginId, pluginVersion: plugin.pluginVersion, parameters, ...(state ? { state } : {}) };
}
