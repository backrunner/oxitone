import { ErrorCode, OxitoneError } from "@oxitone/protocol";
import type { Vst3Runtime } from "../../preview/vst3-client.js";
import type { EvaluatedConfigurationSite, ProjectEvaluation } from "../eval/project-evaluation.js";
import { checkSourceReads } from "../files/read-set.js";

/** Resolve exactly one accepted source usage to the existing Preview processing instance. */
export async function resolveVst3Instance(
  before: ProjectEvaluation,
  site: EvaluatedConfigurationSite,
  snapshotRevision: string,
  runtime: Vst3Runtime | undefined,
  signal: AbortSignal,
  check: () => void,
) {
  if (!runtime)
    throw new OxitoneError(ErrorCode.PluginHostUnavailable, "This document has no live Preview plugin runtime");
  const config = site.config;
  const usage = site.usages[0];
  if (!usage || site.usages.length !== 1 || !config.pluginId.startsWith("vst3."))
    throw new OxitoneError(ErrorCode.EditScopeConflict, "Choose one isolated VST3 instance source boundary");
  const snapshot = before.frame.snapshot;
  const channel = snapshot.channels.find((channel) => channel.id === usage.owner);
  const reference =
    usage.kind === "instrument"
      ? channel?.instrument
      : (usage.kind === "channelInsert"
          ? channel?.effectChain
          : snapshot.mixerChannels.find((bus) => bus.id === usage.owner)?.inserts
        )?.find((ref) => ref.instanceId === usage.handle);
  if (
    !reference?.instanceId ||
    reference.instanceId !== usage.handle ||
    reference.pluginId !== config.pluginId ||
    reference.pluginVersion !== config.pluginVersion
  )
    throw new OxitoneError(ErrorCode.EditTargetMissing, "VST3 source usage no longer identifies this instance");
  const registration = before.frame.vst3Plugins.find(
    (plugin) =>
      config.pluginId === `vst3.${plugin.source.classId.toLowerCase()}` &&
      config.pluginVersion === `0.0.0+${plugin.metadata.sha256}`,
  );
  if (!registration)
    throw new OxitoneError(ErrorCode.PluginHostUnavailable, "VST3 instance registration is unavailable");
  const current = async () => {
    await checkSourceReads(before.reads);
    check();
    if (signal.aborted) throw new OxitoneError(ErrorCode.SourceChanged, "VST3 source request was superseded");
  };
  await current();
  const inventory = await runtime.inventory(snapshotRevision, signal);
  const instance = inventory.instances.find((entry) => entry.instanceId === reference.instanceId);
  if (inventory.state !== "active")
    throw new OxitoneError(ErrorCode.PluginTaskConflict, "Preview plugin graph is not active yet");
  if (!instance || instance.pluginId !== config.pluginId || instance.pluginVersion !== config.pluginVersion)
    throw new OxitoneError(ErrorCode.SourceChanged, "Preview does not contain the accepted VST3 instance");
  await current();
  return {
    runtime,
    registration,
    current,
    snapshotRevision,
    target: {
      graphGeneration: inventory.graphGeneration,
      instanceId: instance.instanceId,
    },
  };
}
