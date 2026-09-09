import { checkProtocolVersion, ErrorCode, OxitoneError, projectSnapshotSchema, type ProjectSnapshot } from "@oxitone/protocol";
import { parseAuthoring } from "./authoring-validation.js";

function invalid(message: string): never { throw new OxitoneError(ErrorCode.InvalidProject, message); }

/** Validate ownership before constructing builders. Full DSP/plugin/DAG validation remains in Rust. */
export function parseRestorableSnapshot(input: ProjectSnapshot): { snapshot: ProjectSnapshot; ids: Set<string> } {
  if (typeof input !== "object" || input === null || typeof input.protocolVersion !== "string") {
    invalid("missing project protocolVersion");
  }
  checkProtocolVersion(input.protocolVersion);
  const snapshot = parseAuthoring(projectSnapshotSchema, input, "project");
  const ids = new Set<string>();
  const master = snapshot.mixerChannels.find((bus) => bus.id === "mix_master");
  if (snapshot.id === "mix_master") invalid("project ID is reserved for Master");
  for (const entity of [snapshot, ...snapshot.markers, ...snapshot.tracks, ...snapshot.channels,
    ...snapshot.mixerChannels, ...snapshot.patterns, ...snapshot.patternClips, ...snapshot.samples,
    ...snapshot.sampleClips, ...snapshot.automation, ...(snapshot.automationClips ?? []), ...snapshot.patterns.flatMap((pattern) => pattern.notes)]) {
    if (entity.id === undefined) continue;
    if (entity.id === "mix_master" && entity !== master) {
      invalid("entity ID is reserved for Master");
    }
    if (ids.has(entity.id)) invalid(`duplicate entity id: ${entity.id}`);
    ids.add(entity.id);
  }
  const channelIds = new Set(snapshot.channels.map((entity) => entity.id));
  const patternIds = new Set(snapshot.patterns.map((entity) => entity.id));
  const sampleIds = new Set(snapshot.samples.map((entity) => entity.id));
  const mixerIds = new Set(snapshot.mixerChannels.map((entity) => entity.id));
  const requireId = (id: string, entities: ReadonlySet<string>) => {
    if (!entities.has(id)) invalid(`unknown entity reference: ${id}`);
  };
  for (const pattern of snapshot.patterns) {
    for (const part of pattern.parts ?? []) {
      requireId(part.channelId, channelIds);
      requireId(part.patternId, patternIds);
    }
  }
  const patternClips = new Map(snapshot.patternClips.map((clip) => [clip.id, clip]));
  const sampleClips = new Map(snapshot.sampleClips.map((clip) => [clip.id, clip]));
  const membership = new Set<string>();
  for (const track of snapshot.tracks) {
    for (const id of track.channelIds) requireId(id, channelIds);
    for (const [list, clips] of [[track.patternClipIds, patternClips], [track.sampleClipIds, sampleClips]] as const) {
      for (const id of list) {
        const clip = clips.get(id);
        if (clip === undefined || clip.trackId !== track.id || membership.has(id)) invalid(`inconsistent clip ownership: ${id}`);
        membership.add(id);
      }
    }
  }
  for (const clip of snapshot.patternClips) {
    requireId(clip.patternId, patternIds);
    if (!membership.has(clip.id)) invalid(`pattern clip is absent from its track: ${clip.id}`);
  }
  for (const clip of snapshot.sampleClips) {
    requireId(clip.sampleId, sampleIds);
    if (!membership.has(clip.id)) invalid(`sample clip is absent from its track: ${clip.id}`);
  }
  for (const channel of snapshot.channels) {
    if (channel.mixerChannelId !== "mix_master") requireId(channel.mixerChannelId, mixerIds);
  }
  for (const bus of snapshot.mixerChannels) {
    const destinations = new Set<string>();
    for (const send of bus.sends) {
      requireId(send.destinationId, mixerIds);
      if (send.destinationId === "mix_master") invalid("Master cannot be a send destination");
      if (destinations.has(send.destinationId)) invalid("duplicate send destination");
      destinations.add(send.destinationId);
    }
  }
  const plugins = snapshot.channels.flatMap((channel) => [channel.instrument, ...channel.effectChain])
    .concat(snapshot.mixerChannels.flatMap((bus) => bus.inserts));
  for (const plugin of plugins) {
    if (plugin.instanceId) {
      if (ids.has(plugin.instanceId)) invalid(`duplicate plugin instance: ${plugin.instanceId}`);
      ids.add(plugin.instanceId);
    }
    for (const id of Object.values(plugin.resources ?? {})) {
      if (!sampleIds.has(id)) invalid(`unknown sample resource: ${id}`);
    }
    if (plugin.pluginId === "oxitone.slicer" && "state" in plugin) {
      const state = plugin.state;
      if (typeof state !== "object" || state === null || !("sampleId" in state) ||
        typeof state.sampleId !== "string" || !sampleIds.has(state.sampleId)) invalid("unknown slicer sample resource");
    }
  }
  for (const lane of snapshot.automation) {
    if (!ids.has(lane.target.entityId) && lane.target.entityId !== "mix_master") invalid(`unknown automation target: ${lane.target.entityId}`);
  }
  const laneIds = new Set(snapshot.automation.map((lane) => lane.id));
  const trackIds = new Set(snapshot.tracks.map((track) => track.id));
  for (const clip of snapshot.automationClips ?? []) {
    requireId(clip.laneId, laneIds);
    requireId(clip.trackId, trackIds);
    if (snapshot.automation.find(lane => lane.id === clip.laneId)?.playback !== "playlist" ||
      snapshot.tracks.find(track => track.id === clip.trackId)?.tempo !== undefined) {
      invalid("automation clips require Playlist lanes and project-time Tracks");
    }
  }
  if (snapshot.automation.some(lane => lane.playback === "playlist" && lane.target.entityId === snapshot.id)) {
    invalid("tempo automation requires global playback");
  }
  ids.add("mix_master");
  return { snapshot, ids };
}
