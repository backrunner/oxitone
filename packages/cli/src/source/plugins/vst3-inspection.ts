import { ErrorCode, OxitoneError, type Vst3Info } from "@oxitone/protocol";
import type { DiscoveredPlugin } from "./plugin-discovery.js";
import { vst3ParameterSpecs, vst3PluginKind } from "./vst3-discovery.js";
import type { Vst3Configurations } from "./vst3-configurations.js";

/** Publish one bounded, verified configuration projection without exposing opaque state to GPUI. */
export function publishVst3Inspection(
  plugin: DiscoveredPlugin,
  info: Vst3Info,
  states: Vst3Configurations,
  projectionBytes: Map<string, number>,
  origin: "inspection" | "editor" = "inspection",
): void {
  const projectionSize = Buffer.byteLength(JSON.stringify({ ...info, configuration: undefined }));
  if (
    [...projectionBytes].reduce((sum, [handle, size]) => sum + (handle === plugin.entry.handle ? 0 : size), 0) +
      projectionSize >
    8 * 1024 * 1024
  )
    throw new OxitoneError(ErrorCode.BudgetExceeded, "VST3 inspection projection exceeds 8 MiB");
  info = states.capture(plugin.entry.handle, info, origin);
  plugin.vst3Info = info;
  projectionBytes.set(plugin.entry.handle, projectionSize);
  Object.assign(plugin.entry, {
    validation: "verified",
    sha256: info.sha256,
    displayName: info.name,
    vendor: info.vendor,
    pluginId: `vst3.${info.classId.toLowerCase()}`,
    pluginVersion: `0.0.0+${info.sha256}`,
    parameters: vst3ParameterSpecs(info),
    kind: vst3PluginKind(info),
    vst3: {
      bundlePath: plugin.vst3Source!.bundlePath,
      classId: info.classId,
      inputChannels: info.inputChannels,
      outputChannels: info.outputChannels,
      noteInput: info.noteInput,
      parameters: info.parameters,
      ...(states.path(plugin.entry.handle) ? { presetPath: states.path(plugin.entry.handle) } : {}),
    },
  });
  delete plugin.entry.diagnostic;
}
