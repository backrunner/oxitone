import { ErrorCode, OxitoneError, vst3ConfigurationSchema, type Vst3ControlCommand } from "@oxitone/protocol";
import type { Vst3Runtime } from "../../preview/vst3-client.js";
import type { EvaluatedConfigurationSite, ProjectEvaluation } from "../eval/project-evaluation.js";
import { resolveVst3Instance } from "./vst3-instance-target.js";

/** Audition or capture the exact accepted processing instance. */
export async function controlVst3Instance(
  before: ProjectEvaluation,
  site: EvaluatedConfigurationSite,
  snapshotRevision: string,
  command: Vst3ControlCommand,
  runtime: Vst3Runtime | undefined,
  signal: AbortSignal,
  check: () => void,
) {
  const endpoint = await resolveVst3Instance(before, site, snapshotRevision, runtime, signal, check);
  const result = await endpoint.runtime.control(
    snapshotRevision,
    {
      instanceControlVersion: 1,
      graphGeneration: endpoint.target.graphGeneration,
      instanceId: endpoint.target.instanceId,
      command,
      timeoutMs: 5000,
    },
    signal,
  );
  await endpoint.current();
  if (result.graphGeneration !== endpoint.target.graphGeneration || result.instanceId !== endpoint.target.instanceId)
    throw new OxitoneError(ErrorCode.SourceChanged, "Preview returned a different VST3 instance");
  if (command.kind !== "capture") return;
  const info = result.state.info;
  if (
    !info?.configuration ||
    info.classId.toLowerCase() !== endpoint.registration.source.classId.toLowerCase() ||
    info.sha256 !== endpoint.registration.metadata.sha256
  )
    throw new OxitoneError(ErrorCode.PluginManifestMismatch, "VST3 capture changed instance identity");
  const configuration = vst3ConfigurationSchema.parse(info.configuration);
  if (
    configuration.classId.toLowerCase() !== endpoint.registration.source.classId.toLowerCase() ||
    configuration.sha256 !== endpoint.registration.metadata.sha256
  )
    throw new OxitoneError(ErrorCode.PluginManifestMismatch, "VST3 captured state changed plugin identity");
  return configuration;
}
