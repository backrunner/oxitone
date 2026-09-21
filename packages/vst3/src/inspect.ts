import { resolve } from "node:path";
import {
  ErrorCode,
  OxitoneError,
  vst3SourceSchema,
  vst3InfoSchema,
  type Vst3Source,
  type Vst3Info,
} from "@oxitone/protocol";
import { runHelper, type Vst3HostOptions } from "./helper.js";
import { parse } from "./validation.js";

/** Load one exact class in a disposable native process; never opens an audio device or editor. */
export async function inspectVst3Plugin(source: Vst3Source, host: Vst3HostOptions = {}): Promise<Vst3Info> {
  const selected = parse(vst3SourceSchema, source);
  const info = parse(
    vst3InfoSchema,
    await runHelper(
      { protocolVersion: 1, operation: "inspect", source: { ...selected, bundlePath: resolve(selected.bundlePath) } },
      host,
    ),
  );
  if (
    info.classId.toLowerCase() !== selected.classId.toLowerCase() ||
    (selected.expectedHash !== undefined && info.sha256 !== selected.expectedHash) ||
    (info.configuration !== null &&
      (info.configuration.classId.toLowerCase() !== info.classId.toLowerCase() ||
        info.configuration.sha256 !== info.sha256))
  )
    throw new OxitoneError(ErrorCode.RealtimeFault, "VST3 helper inspection identity mismatch");
  return info;
}
