import { Pattern, Project, createAutomationNamespace, wavetable } from "@oxitone/core";
import type { InstrumentRef } from "@oxitone/protocol";

type Hit = { pitch: number; start: number; duration: number; velocity: number };
export const drumInstrument = (): InstrumentRef => ({
  pluginId: "example.drums", pluginVersion: "1.0.0", parameters: { volume: 0.85, decay: 1 },
});
export const referenceGain = { pluginId: "fixture.gain", pluginVersion: "1.0.0", parameters: { gain: 1 } };

function drumBar(bar: number): Pattern {
  const notes: Hit[] = [];
  const hit = (pitch: number, start: number, velocity: number) => notes.push({ pitch, start, velocity, duration: 0.08 });
  if (bar === 15) {
    hit(36, 0, 1); hit(46, 0, 0.7);
  } else {
    if (bar !== 8) for (const beat of [0, 2, ...(bar >= 2 && bar % 2 === 1 ? [2.75] : [])]) hit(36, beat, 0.95);
    for (const beat of [1, 3]) hit(38, beat, 0.78);
    for (let i = 0; i < 8; i++) {
      const open = i === 7 && bar >= 2 && bar !== 8;
      hit(open ? 46 : 42, i / 2 + (i % 2 ? 0.04 : 0), i % 2 ? 0.5 : 0.68);
    }
    if (bar >= 2 && bar % 2 === 0) hit(38, 2.75, 0.2);
    if ([3, 7, 11, 14].includes(bar)) {
      for (const [i, start] of [3.25, 3.5, 3.75].entries()) hit(38, start, 0.35 + i * 0.17);
    }
  }
  return new Pattern({ lengthBeats: 4, notes });
}

/** 16-bar original arrangement: intro, groove, breakdown, return and ending. */
export function createDrumSong(): Project {
  const project = new Project({ name: "Midnight Circuit / 午夜回路", seed: 20260907 });
  project.setTempo(112);
  const automation = createAutomationNamespace();
  const drums = project.addChannel({ name: "Native drum machine", instrument: drumInstrument(),
    level: 1, effectChain: [referenceGain] });
  const drumTrack = project.addTrack("Drums · MIDI 10").use(drums);
  drumTrack.midiChannel = 10;
  const bass = project.addChannel({ name: "Round bass", level: 0.65,
    instrument: wavetable({ oscA: { wave: "triangle" }, filter: { cutoff: 700, resonance: 0.12 },
      amp: { attack: 0.008, decay: 0.12, sustain: 0.55, release: 0.08 } }) });
  const bassTrack = project.addTrack("Bass").use(bass);
  const keys = project.addChannel({ name: "Soft keys", level: 0.42, pan: -0.15,
    instrument: wavetable({ oscA: { wave: "triangle" }, filter: { cutoff: 2600 },
      amp: { attack: 0.015, decay: 0.22, sustain: 0.3, release: 0.25 } }),
    effectChain: [{ pluginId: "oxitone.delay", pluginVersion: "1.0.0",
      parameters: { timeBeats: 0.75, feedback: 0.25 }, mix: 0.2 }] });
  const keyTrack = project.addTrack("Am7 · Fmaj7 · Cmaj7 · G6").use(keys);
  const lead = project.addChannel({ name: "Bell motif", level: 0.45, pan: 0.22,
    instrument: wavetable({ oscA: { wave: "sine" },
      amp: { attack: 0.003, decay: 0.18, sustain: 0.15, release: 0.14 } }),
    effectChain: [{ pluginId: "oxitone.delay", pluginVersion: "1.0.0",
      parameters: { timeBeats: 0.5, feedback: 0.3 }, mix: 0.25 }] });
  const leadTrack = project.addTrack("Melody").use(lead);
  const chords = [[57, 60, 64, 67], [53, 57, 60, 64], [55, 59, 60, 64], [55, 59, 62, 64]];
  const roots = [33, 29, 36, 31];
  for (let bar = 0; bar < 16; bar++) {
    drumTrack.add(drumBar(bar)).at({ bar: bar + 1 });
    const chord = chords[Math.floor(bar / 2) % 4]!;
    const root = roots[Math.floor(bar / 2) % 4]!;
    if (bar >= 2 && bar !== 8) {
      const beats = bar === 15 ? [0] : [0, 0.75, 2, 2.75, 3.5];
      bassTrack.add(new Pattern({ lengthBeats: 4, notes: beats.map((start, i) => ({
        pitch: root + (i === 4 ? 12 : 0), start, duration: bar === 15 ? 1.5 : 0.45, velocity: 0.78,
      })) })).at({ bar: bar + 1 });
    }
    const starts = bar === 15 ? [0] : [0.5, 2.5];
    keyTrack.add(new Pattern({ lengthBeats: 4, notes: starts.flatMap((start) => chord.map((pitch) => ({
      pitch, start, duration: bar === 15 ? 1.5 : 0.8, velocity: bar < 2 ? 0.48 : 0.62,
    }))) })).at({ bar: bar + 1 });
    if ((bar >= 4 && bar < 8) || (bar >= 10 && bar < 15)) {
      const melody = [chord[2]! + 12, chord[3]! + 12, chord[1]! + 12, chord[2]! + 12];
      leadTrack.add(new Pattern({ lengthBeats: 4, notes: melody.map((pitch, i) => ({
        pitch, start: [0.5, 1.25, 2.5, 3.25][i]!, duration: 0.35, velocity: i % 2 ? 0.55 : 0.7,
      })) })).at({ bar: bar + 1 });
    }
  }
  drums.automate("decay", automation.sine({ periodBeats: 16, min: 0.28, max: 0.42 }));
  drums.automate("insert.0.parameter.gain", automation.polyline([
    { beat: 0, value: 0.35 }, { beat: 8, value: 0.5 }, { beat: 60, value: 0.5 }, { beat: 64, value: 0 },
  ]));
  project.master.automate("level", automation.polyline([
    { beat: 0, value: 0.9 }, { beat: 61, value: 0.9 }, { beat: 64, value: 0 },
  ])); // Master level 0..2: normalized 0.9 gives physical 1.8.
  for (const [name, beat] of [["Intro", 0], ["Groove", 8], ["Breakdown", 32], ["Return", 40], ["Ending", 60]] as const) {
    project.addMarker(name, beat);
  }
  return project;
}
