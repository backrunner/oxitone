import { Pattern, Project, createAutomationNamespace, wavetable } from "@oxitone/core";

/** A deterministic two-bar phrase, authored entirely in musical time. */
export function createSong(): Project {
  const project = new Project({ name: "First phrase", seed: 42 });
  project.setTempo(120);
  const keys = project.addChannel({
    name: "Keys",
    level: 0.5,
    instrument: wavetable({
      oscA: { wave: "triangle" },
      amp: { attack: 0.005, decay: 0.08, sustain: 0.5, release: 0.05 },
    }),
    effectChain: [
      { pluginId: "oxitone.delay", pluginVersion: "1.0.0", parameters: { timeBeats: 0.5, feedback: 0.2 }, mix: 0.15 },
    ],
  });
  const pitches = [60, 64, 67, 72, 67, 64, 62, 67];
  const phrase = new Pattern({
    lengthBeats: 8,
    notes: pitches.map((pitch, i) => ({ pitch, start: i, duration: 0.75, velocity: 0.8 })),
  });
  project.addTrack("Phrase").use(keys).add(phrase).at({ bar: 1 });
  const automation = createAutomationNamespace();
  keys.automate("pan", automation.sine({ periodBeats: 8, min: 0.3, max: 0.7 }));
  return project;
}
