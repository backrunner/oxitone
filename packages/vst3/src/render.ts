import { link, mkdir, mkdtemp, rm } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import {
  ErrorCode,
  OxitoneError,
  vst3RenderOptionsSchema,
  vst3RenderReportSchema,
  vst3SourceSchema,
  type Vst3Source,
  type Vst3RenderOptions,
  type Vst3RenderReport,
} from "@oxitone/protocol";
import { runHelper, type Vst3HostOptions } from "./helper.js";
import { parse } from "./validation.js";

/** Offline native rendering to a new float32 WAV. Existing files are never replaced. */
export async function renderVst3Wav(
  source: Vst3Source,
  options: Vst3RenderOptions,
  host: Vst3HostOptions = {},
): Promise<Vst3RenderReport> {
  const selected = parse(vst3SourceSchema, source);
  const settings = parse(vst3RenderOptionsSchema, options);
  const path = resolve(settings.path);
  if (settings.frames + settings.tailFrames > 536_000_000)
    throw new OxitoneError(ErrorCode.WavTooLarge, "VST3 render exceeds the stereo RIFF budget");
  if (settings.events.some((event) => event.frame >= settings.frames))
    throw new OxitoneError(ErrorCode.InvalidProject, "VST3 events must precede content end");
  host.signal?.throwIfAborted();
  let temporary: string | undefined;
  try {
    await mkdir(dirname(path), { recursive: true });
    temporary = await mkdtemp(join(dirname(path), ".oxitone-vst3-"));
    const staged = join(temporary, "render.wav");
    const response = await runHelper(
      {
        protocolVersion: 1,
        operation: "render",
        source: { ...selected, bundlePath: resolve(selected.bundlePath) },
        options: {
          ...settings,
          path: staged,
          ...(settings.inputPath ? { inputPath: resolve(settings.inputPath) } : {}),
        },
      },
      host,
    );
    const report = parse(vst3RenderReportSchema, response);
    if (
      report.path !== staged ||
      report.frames !== settings.frames + settings.tailFrames ||
      report.sampleRate !== settings.sampleRate ||
      (selected.expectedHash !== undefined && report.pluginSha256 !== selected.expectedHash) ||
      (settings.configuration !== undefined && report.pluginSha256 !== settings.configuration.sha256)
    )
      throw new OxitoneError(ErrorCode.RealtimeFault, "VST3 helper render identity mismatch");
    host.signal?.throwIfAborted();
    await link(staged, path); // Atomic create-only publication, including races and symlink destinations.
    return { ...report, path };
  } catch (cause) {
    if (cause instanceof OxitoneError || (cause instanceof Error && cause.name === "AbortError")) throw cause;
    throw new OxitoneError(ErrorCode.AssetUnavailable, "Cannot publish VST3 WAV (destination must not exist)", {
      cause,
    });
  } finally {
    if (temporary) await rm(temporary, { recursive: true, force: true });
  }
}
