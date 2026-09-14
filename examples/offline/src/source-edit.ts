import { arp, chord, Project, wavetable } from "@oxitone/core";

/** Ordinary executable TS: generated rules and local exceptions, without author IDs. */
export const phrase = arp(chord(60, "major"), "upDown", 0.25, {
  velocityCurve: { from: 0.4, to: 0.8 },
});
export const variation = phrase.repeat(8).edit([
  { select: { iteration: 2, note: { step: 1 } }, remove: true },
  { select: { iteration: 2, note: { step: 3 } }, set: { pitch: 65 } },
  { select: { iteration: 5, note: { step: 0 } }, shift: { pitch: 12 } },
]);

const project = new Project({ name: "Source edits", seed: 42 });
project.setTempo(120);
const keys = project.addChannel({ name: "Keys", level: 0.4, instrument: wavetable({ oscA: { wave: "triangle" } }) });
project.addTrack("Original").use(keys).add(phrase.repeat(8)).at({ bar: 1 });
project.addTrack("Variation").use(keys).add(variation).at({ bar: 3 });
export default project;
