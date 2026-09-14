import type { ProjectSnapshot } from "@oxitone/protocol";

/** MIDI-only copy: retain unrestricted authoring/audio tracks and group drum exports on GM 10. */
export function midiSnapshot(source: ProjectSnapshot): ProjectSnapshot {
  const copy = structuredClone(source);
  // Explicit demo timbre families; shared MIDI channels cannot reproduce independent patches/CCs.
  const families: [RegExp, number][] = [
    [/^Soft Piano/, 1],
    [/^Grand Piano/, 2],
    [/pure mono sub/, 3],
    [/harmonic bassline/, 4],
    [/^Undertow/, 5],
    [/^Signal/, 6],
    [/^Formant/, 7],
    [/^Laser/, 8],
    [/^Horizon/, 9],
    [/^Clap|^Drum/, 10],
    [/supersaw|upper pulse/, 11],
    [/center chord body|distorted chord edge/, 12],
    [/^Ember|^Orbit/, 13],
    [/^Bloom/, 14],
    [/^Prism|^Halo/, 15],
    [/^Air/, 16],
  ];
  for (const track of [...copy.tracks].sort((a, b) => a.id.localeCompare(b.id))) {
    const channel = copy.channels.find((c) => c.id === track.channelIds[0]);
    if (!channel) continue;
    if (channel.instrument.pluginId === "example.drums") track.midiChannel = 10;
    else {
      const family = families.find(([pattern]) => pattern.test(channel.name ?? ""));
      if (!family) throw new Error(`Declare a MIDI export family for ${channel.name ?? channel.id}`);
      track.midiChannel = family[1];
    }
  }
  return copy;
}
