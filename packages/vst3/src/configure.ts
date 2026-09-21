import { resolve } from "node:path";
import {
  ErrorCode,
  OxitoneError,
  vst3ConfigurationOptionsSchema,
  vst3InfoSchema,
  vst3SourceSchema,
  type Vst3ConfigurationOptions,
  type Vst3Info,
  type Vst3Source,
} from "@oxitone/protocol";
import { runHelper, type Vst3HostOptions } from "./helper.js";
import { parse } from "./validation.js";

/** Restore state, flush controller edits and inspect the resulting capabilities in a silent helper. */
export async function configureVst3Plugin(
  source: Vst3Source,
  options: Vst3ConfigurationOptions = {},
  host: Vst3HostOptions = {},
): Promise<Vst3Info> {
  const selected = parse(vst3SourceSchema, source);
  const settings = parse(vst3ConfigurationOptionsSchema, options);
  if (
    settings.configuration &&
    (settings.configuration.classId.toLowerCase() !== selected.classId.toLowerCase() ||
      (selected.expectedHash !== undefined && settings.configuration.sha256 !== selected.expectedHash))
  )
    throw new OxitoneError(
      ErrorCode.PluginManifestMismatch,
      "VST3 configuration belongs to a different class or binary",
    );
  const info = parse(
    vst3InfoSchema,
    await runHelper(
      {
        protocolVersion: 1,
        operation: "configure",
        source: { ...selected, bundlePath: resolve(selected.bundlePath) },
        options: settings,
      },
      host,
    ),
  );
  if (
    info.classId.toLowerCase() !== selected.classId.toLowerCase() ||
    (selected.expectedHash !== undefined && info.sha256 !== selected.expectedHash) ||
    (settings.configuration !== undefined && info.sha256 !== settings.configuration.sha256) ||
    !info.configuration ||
    info.configuration.classId.toLowerCase() !== info.classId.toLowerCase() ||
    info.configuration.sha256 !== info.sha256
  )
    throw new OxitoneError(ErrorCode.PluginManifestMismatch, "VST3 configuration returned a different class or binary");
  return info;
}
