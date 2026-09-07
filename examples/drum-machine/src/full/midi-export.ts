import type { ProjectSnapshot } from "@oxitone/protocol";

/** MIDI-only copy: retain unrestricted authoring/audio tracks and group drum exports on GM 10. */
export function midiSnapshot(source: ProjectSnapshot): ProjectSnapshot {
  const copy = structuredClone(source);
  const channels = new Map<string, number>();
  const available = Array.from({ length: 16 }, (_, i) => i + 1).filter(c => c !== 10);
  for (const track of [...copy.tracks].sort((a,b) => a.id.localeCompare(b.id))) {
    const channel = copy.channels.find(c => c.id === track.channelIds[0]);
    if (!channel) continue;
    if (channel.instrument.pluginId === "example.drums") track.midiChannel = 10;
    else {
      if (!channels.has(channel.id)) {
        const next = available.shift();
        if (next !== undefined) channels.set(channel.id, next);
      }
      // Leave excess parts unassigned so the native exporter reports MidiChannelLimit.
      track.midiChannel = channels.get(channel.id);
    }
  }
  return copy;
}
