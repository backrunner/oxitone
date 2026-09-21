import { resolve } from "node:path";
import {
  ErrorCode,
  OxitoneError,
  vst3EditorOptionsSchema,
  vst3EditorResultSchema,
  vst3SourceSchema,
  type Vst3EditorOptions,
  type Vst3Info,
  type Vst3Source,
} from "@oxitone/protocol";
import { runHelper, type Vst3HostOptions } from "./helper.js";
import { parse } from "./validation.js";

/** Native editor with Apply/Cancel. Returns captured configuration on Apply, null on Cancel.
 * This is an isolated authoring session; use the result with vst3Config or saveVst3Preset.
 * No audio device is opened. Default timeout is ten minutes, including user interaction.
 */
export async function editVst3Plugin(
  source: Vst3Source,
  options: Vst3EditorOptions = {},
  host: Vst3HostOptions = {},
): Promise<Vst3Info | null> {
  const selected = parse(vst3SourceSchema, source);
  const settings = parse(vst3EditorOptionsSchema, options);
  const reply = parse(
    vst3EditorResultSchema,
    await runHelper(
      {
        protocolVersion: 1,
        operation: "edit",
        source: { ...selected, bundlePath: resolve(selected.bundlePath) },
        options: settings,
      },
      { ...host, timeoutMs: host.timeoutMs ?? 600_000 },
    ),
  );
  if (!reply.accepted) return null;
  const info = reply.info;
  if (
    info.classId.toLowerCase() !== selected.classId.toLowerCase() ||
    (selected.expectedHash !== undefined && info.sha256 !== selected.expectedHash) ||
    !info.configuration ||
    info.configuration.classId.toLowerCase() !== info.classId.toLowerCase() ||
    info.configuration.sha256 !== info.sha256
  )
    throw new OxitoneError(ErrorCode.PluginManifestMismatch, "VST3 editor returned a different class or binary");
  return info;
}
