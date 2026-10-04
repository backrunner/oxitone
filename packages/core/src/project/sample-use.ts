import { ErrorCode, OxitoneError, sampleUseSchema, type SampleUse, type SampleDestination } from "@oxitone/protocol";
import { Sample } from "../arrangement/sample.js";
import { parseAuthoring } from "../authoring-validation.js";
import type { Project } from "./project.js";
import { editPreflight } from "./edit-preflight.js";

function required<T>(value: T | undefined): T {
  if (value === undefined) throw new OxitoneError(ErrorCode.EditTargetMissing, "Sample destination no longer exists");
  return value;
}
function pluginTarget(project: Project, target: Extract<SampleDestination, { kind: "plugin" }>) {
  const owner = required(
    target.owner === "channel" ? project.channels[target.index] : project.mixerChannels[target.index],
  );
  const instance = required(
    target.slot === undefined
      ? "instrumentInstance" in owner
        ? owner.instrumentInstance
        : undefined
      : owner.effectInstances[target.slot],
  );
  const config = instance.config.toSpec();
  const slot = target.resource;
  if (config.pluginId.startsWith("vst3."))
    throw new OxitoneError(ErrorCode.EditNotRepresentable, "This VST3 does not expose host sample resources");
  const supported =
    config.pluginId === "oxitone.sampler"
      ? slot === "sample"
      : config.pluginId === "oxitone.slicer"
        ? slot === "state.sampleId"
        : config.pluginId === "oxitone.convolver"
          ? slot === "impulse"
          : Object.hasOwn(config.resources ?? {}, slot);
  if (!supported) throw new OxitoneError(ErrorCode.EditNotRepresentable, "Plugin does not expose this sample slot");
  return { owner, instance, config };
}

/** Attach an immutable resource without replacing the plugin instance or its automation. */
export function useSample(project: Project, input: SampleUse): void {
  const edit = parseAuthoring(sampleUseSchema, input, "project.useSample");
  applySample(editPreflight(project), edit);
  applySample(project, edit);
}

function applySample(project: Project, edit: SampleUse): void {
  const target = edit.destination;
  const plugin = target.kind === "plugin" ? pluginTarget(project, target) : undefined;
  const track =
    target.kind === "arrangement" && target.track !== undefined ? required(project.tracks[target.track]) : undefined;
  if (track?.tempo !== undefined)
    throw new OxitoneError(ErrorCode.EditNotRepresentable, "Sample placement requires a Track following project tempo");
  if (typeof edit.sample !== "number") Sample.fromSpec({ ...edit.sample, id: "smp_preflight" });
  else required(project.samples[edit.sample]);
  if (project.revisionBigInt > 0xffff_ffff_ffff_ffffn - 4n)
    throw new OxitoneError(ErrorCode.InvalidProject, "project revision exhausted");
  const sample =
    typeof edit.sample === "number"
      ? required(project.samples[edit.sample])
      : project.importSampleRef({ ...edit.sample, id: "smp_import" });
  if (target.kind === "arrangement") {
    project.createSampleClip(
      track ?? project.addTrack(target.name ?? "Audio"),
      sample,
      project.beatsToBarBeat(target.startBeat),
      { tempoSync: "off" },
    );
  } else {
    const { owner, instance, config } = plugin!;
    if (target.resource === "state.sampleId" && config.pluginId === "oxitone.slicer") {
      const state = config.state as Record<string, unknown>;
      if (!state || Array.isArray(state.slices))
        throw new OxitoneError(
          ErrorCode.EditNotRepresentable,
          "Replacing this sample requires updating its explicit slice markers in source",
        );
      config.state = { ...state, sampleId: sample.id };
    } else config.resources = { ...config.resources, [target.resource]: sample.id };
    owner.updateInstance(instance, config);
  }
}
