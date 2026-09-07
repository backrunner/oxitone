import { Project } from "@oxitone/core";
import type { InstrumentRef } from "@oxitone/protocol";
import { drumInstrument } from "../song.js";
import { uprightPiano } from "./piano.js";
import { automation, bar, fx, note, preview, sections, type Hit } from "./shared.js";
import { windowHook } from "./themes.js";
import { warmBass, tapeGlass, eveningPad } from "./synth-patches.js";

export const lofi = { slug: "rain-on-the-window", title: "Rain on the Window / 窗边雨", bpm: 80, bars: 60,
  sections: [["Rain / Intro", 0], ["Streetlights / Verse A", 8], ["Window / Hook I", 16], ["Empty platform / Bridge", 24],
    ["Last train / Verse B", 32], ["Window / Hook II", 40], ["Home / Reprise", 48], ["Lights out / Outro", 56]] as const };

/** Three-minute original in D minor: voiced ninths, swung drums, piano answers and an outro. */
export function createLofiSong(piano: (project: Project) => InstrumentRef = uprightPiano): Project {
  const p = new Project({ name: lofi.title, seed: 2026090801 });
  p.setTempo(lofi.bpm);
  const room = p.addMixerChannel({ name: "Room · return", inserts: [fx("reverb", { decaySeconds: 1.65, damping: 0.72, predelayMs: 12 })] });
  const keysBus = p.addMixerChannel({ name: "Upright · warm tape", inserts: [
    fx("filter", { cutoffHz: 4300, resonance: 0.707 }), fx("saturator", { driveDb: 3 }, 0.18)] });
  keysBus.send(room, { ratio: 0.19 });
  const drumBus = p.addMixerChannel({ name: "Dusty drums", inserts: [fx("filter", { cutoffHz: 6700, resonance: 0.707 })] });
  drumBus.send(room, { ratio: 0.055 });
  const bassBus = p.addMixerChannel({ name: "Round bass" });
  const melodyBus = p.addMixerChannel({ name: "Window reflections", inserts: [fx("delay", { timeBeats: 0.75, feedback: 0.32 }, 0.22)] });
  melodyBus.send(room, { ratio: 0.28 });
  const keys = p.addChannel({ name: "VSCO upright · 3 dynamics", instrument: piano(p), mixerChannelId: keysBus.id, level: 1.5 });
  const drums = p.addChannel({ name: "Native lofi kit", instrument: drumInstrument(), mixerChannelId: drumBus.id, level: 0.6 });
  const bass = p.addChannel({ name: "Sine / triangle bass", mixerChannelId: bassBus.id, level: 0.44,
    instrument: warmBass() });
  const bell = p.addChannel({ name: "Tape glass · hook", mixerChannelId: melodyBus.id, level: 0.24, pan: 0.16,
    instrument: tapeGlass() });
  const pad = p.addChannel({ name: "Evening air", mixerChannelId: melodyBus.id, level: 0.065, pan: -0.2,
    instrument: eveningPad() });
  const kt = p.addTrack("Piano · Dm9 / Bbmaj9 / Fmaj9 / Cadd9").use(keys);
  const mt = p.addTrack("Piano · answering melody").use(keys);
  const dt = p.addTrack("Drums · swung pocket").use(drums); dt.midiChannel = 10;
  const bt = p.addTrack("Bass").use(bass), gt = p.addTrack("Glass motif").use(bell), pt = p.addTrack("Air").use(pad);
  const chords = [[50, 57, 60, 64, 65], [46, 53, 57, 60, 62], [53, 57, 60, 64, 67], [48, 55, 60, 62, 64]];
  const roots = [38, 34, 41, 36];
  for (let b = 0; b < lofi.bars; b++) {
    const bridge = b >= 24 && b < 32, outro = b >= 56, intro = b < 8;
    const index = b >= 58 ? 0 : Math.floor(b / 2) % 4, chord = chords[index]!, root = roots[index]!;
    const final = b === 59, sparse = intro || bridge || outro;
    const chorus = b >= 16 && b < 24 || b >= 40 && b < 48;
    const ks: Hit[] = [];
    for (const start of final ? [0] : sparse ? [0.04] : [0.04, 2.56]) {
      chord.forEach((pitch, i) => ks.push(note(pitch, start + i * 0.018, final ? 2.4 : sparse ? 2.85 : 1.06,
        0.38 + i * 0.031 + (b % 3) * 0.022)));
    }
    bar(kt, b, ks, `${["Dm9", "Bbmaj9", "Fmaj9", "Cadd9"][index]} · ${sparse ? "open" : "pocket"}`);
    if (b >= 8 && b < 56 && !bridge) bar(mt, b, windowHook(b, chorus ? -1 : 0), chorus ? "Window · piano harmony" : "Window · eight-bar theme");
    if (intro && b >= 4 && b % 2 === 0 || bridge && b % 2 === 0) bar(mt, b, windowHook(b).slice(0, 2), "Window · fragment");
    if (b === 56) bar(mt, b, windowHook(0), "Window · farewell");
    if (b === 58) bar(mt, b, [note(74, 0.5, 3, 0.5)], "D · home");
    if (b >= 8 && b < 56 && (!bridge || b >= 28)) {
      bar(bt, b, [0, 1.8, 2.65, ...(b % 2 ? [3.5] : [])].map((start, i) =>
        note(root + (i === 3 ? 7 : 0), start, i === 0 ? 1.3 : 0.42, 0.76)), "Bass pocket");
    }
    if (b >= 4 && b < 58 && (!bridge || b >= 30)) {
      const hits: Hit[] = [];
      if (!intro && b < 56) for (const t of [0, 2.45, ...(b % 2 ? [3.25] : [])]) hits.push(note(36, t, 0.08, 0.86));
      for (const t of [1.04, 3.04]) hits.push(note(38, t, 0.08, intro || outro ? 0.32 : 0.63));
      for (let i = 0; i < 8; i++) hits.push(note(i === 7 && b % 4 === 3 ? 46 : 42,
        i / 2 + (i % 2 ? 0.085 : 0.006), 0.06, i % 2 ? 0.29 : 0.43));
      if (b % 4 === 3) hits.push(note(38, 2.81, 0.06, 0.2));
      if ([23, 47, 55].includes(b)) for (const t of [3.5, 3.75]) hits.push(note(38, t, 0.08, 0.38));
      bar(dt, b, hits, intro || outro ? "Brushes" : "Dusty swing");
    }
    if (chorus) bar(gt, b, windowHook(b).map(hit => ({ ...hit, velocity: hit.velocity * (b >= 40 ? 1 : 0.88) })), "Window · glass hook");
    if (b >= 48 && b < 56 && b % 2) bar(gt, b, [note(chord[2]! + 12, 3, 0.7, 0.4)], "Glass · afterthought");
    if (b >= 24 && b < 56 && b % 2 === 0) bar(pt, b,
      chord.slice(1, 4).map(pitch => note(pitch + 12, 0, 3.7, 0.5)), "Air voicing");
  }
  keysBus.automate("insert.0.parameter.cutoffHz", automation.polyline([
    { beat: 0, value: 0.62 }, { beat: 32, value: 0.8 }, { beat: 96, value: 0.7 },
    { beat: 128, value: 0.82 }, { beat: 224, value: 0.75 }, { beat: 240, value: 0.55 },
  ]));
  p.master.inserts = [fx("utility", { gainDb: 9 }), fx("limit", { ceilingDb: -1.5, releaseMs: 150 })];
  sections(p, lofi.sections, lofi.bars, 1);
  return p;
}

export default function createProject() { return preview(createLofiSong()); }
