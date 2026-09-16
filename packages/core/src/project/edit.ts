import { projectEditSchema, ErrorCode, OxitoneError, type ProjectEdit } from "@oxitone/protocol";
import type { Project } from "./project.js";
import { orderEffects } from "../channels/effect-order.js";
import { parseAuthoring } from "../authoring-validation.js";

/** Apply a validated control-side configuration without writing source or contacting audio devices. */
export function configure(project: Project, input: ProjectEdit): void {
  const edit = parseAuthoring(projectEditSchema, input, "project.configure");
  project.assertMutable();
  const require = <T>(value: T | undefined): T => {
    if (value === undefined)
      throw new OxitoneError(ErrorCode.EditTargetMissing, "Project edit target no longer exists");
    return value;
  };
  if (edit.kind === "tempo") {
    if (
      project.automationLanes.some((lane) => lane.target.entityId === project.id && lane.target.parameterId === "tempo")
    ) {
      throw new OxitoneError(ErrorCode.EditScopeConflict, "Tempo is automated; edit its automation instead");
    }
    if (project.tempoMap.length !== 1)
      throw new OxitoneError(ErrorCode.EditScopeConflict, "Edit the tempo map to preserve its tempo changes");
    project.setTempo(edit.bpm);
    return;
  }
  if (edit.kind === "track") {
    const track = require(project.tracks[edit.index]);
    if (edit.enabled !== undefined) track.enabled = edit.enabled;
    if (edit.mute !== undefined) track.mute = edit.mute;
    if (edit.solo !== undefined) track.solo = edit.solo;
    return;
  }
  if (edit.kind === "effectOrder") {
    orderEffects(
      require(edit.owner === "channel" ? project.channels[edit.index] : project.mixerChannels[edit.index]),
      edit.order,
    );
    return;
  }
  if (edit.kind === "instrument") {
    const channel = require(project.channels[edit.index]);
    channel.instrument = edit.config;
    // Validation clones config data; source capture still needs the caller's original object identity.
    if (input.kind === "instrument") channel.configurationSources.instrument = input.config;
    return;
  }
  if (edit.kind === "effect") {
    const owner = require(edit.owner === "channel" ? project.channels[edit.index] : project.mixerChannels[edit.index]);
    const sources = [...owner.configurationSources.chain];
    const sourceConfig = input.kind === "effect" ? input.config : undefined;
    if (edit.slot === undefined) {
      if (!edit.config)
        throw new OxitoneError(ErrorCode.EditTargetMissing, "Adding an effect requires its configuration");
      owner.addEffect(edit.config);
      owner.configurationSources.chain = [...sources, sourceConfig!];
      return;
    }
    const instance = require(owner.effectInstances[edit.slot]);
    if (!edit.config) {
      owner.removeEffect(instance);
      return;
    }
    // A replacement receives a fresh identity; existing bound automation must be explicitly removed/remapped.
    const spec = owner.toSpec();
    const chain = [
      ...("effectChain" in spec ? (spec.effectChain ?? []) : "inserts" in spec ? (spec.inserts ?? []) : []),
    ];
    chain[edit.slot] = edit.config;
    if ("effectChain" in owner) owner.effectChain = chain;
    else owner.inserts = chain;
    sources[edit.slot] = sourceConfig!;
    owner.configurationSources.chain = sources;
    return;
  }
  const owner = require(edit.kind === "channel" ? project.channels[edit.index] : project.mixerChannels[edit.index]);
  const values = edit.values;
  // All scalar ranges have been checked before the first mutation.
  if (values.level !== undefined) owner.level = values.level;
  if (values.pan !== undefined) {
    if ("pan" in owner) owner.pan = values.pan;
    else owner.balance = values.pan;
  }
  if (values.mute !== undefined) owner.mute = values.mute;
  if (values.solo !== undefined) owner.solo = values.solo;
}
