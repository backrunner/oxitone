import { Project, wavetable } from "@oxitone/core";
import type { InstrumentRef } from "@oxitone/protocol";
import { drumInstrument } from "../song.js";
import { uprightPiano } from "./piano.js";
import { automation, bar, fx, note, preview, sections, type Hit } from "./shared.js";
import { horizonHook } from "./themes.js";
import { skyChords, horizonLead, motionBass, skyPad, tapeGlass } from "./synth-patches.js";

export const dubstep = { slug: "after-the-horizon", title: "After the Horizon / 地平线之后", bpm: 140, bars: 104,
  sections: [["First light / Piano", 0], ["Lift / Build I", 8], ["Open sky / Drop I", 24],
    ["Weightless / Break", 40], ["Signal / Build II", 56], ["Beyond / Drop II", 72],
    ["Afterglow / Reprise", 88], ["Horizon / Outro", 96]] as const };

/** 104 bars in F# minor; recurring piano theme becomes a layered, half-time drop. */
export function createDubstepSong(piano: (project: Project) => InstrumentRef = uprightPiano): Project {
  const p = new Project({ name: dubstep.title, seed: 2026090802 }); p.setTempo(dubstep.bpm);
  const hall = p.addMixerChannel({ name: "Sky hall · return", inserts: [fx("reverb", { decaySeconds: 3.4, damping: 0.43, predelayMs: 28 })] });
  const echo = p.addMixerChannel({ name: "Dotted echo · return", inserts: [fx("delay", { timeBeats: 0.75, feedback: 0.36 })] });
  echo.send(hall, { ratio: 0.2 });
  const keysBus = p.addMixerChannel({ name: "Upright piano" }); keysBus.send(hall, { ratio: 0.32 });
  const kickBus = p.addMixerChannel({ name: "Kick · detector" });
  const drumsBus = p.addMixerChannel({ name: "Snare / tops", inserts: [fx("saturator", { driveDb: 2, outputDb: -1 }, 0.2)] });
  drumsBus.send(hall, { ratio: 0.08 });
  const duck = () => fx("compressor", { thresholdDb: -27, ratio: 7, attackMs: 0.4, releaseMs: 180, kneeDb: 5 });
  const synthBus = p.addMixerChannel({ name: "Wide chords · ducked", inserts: [duck()] });
  const bassBus = p.addMixerChannel({ name: "Bass · ducked", inserts: [duck()] });
  kickBus.send(synthBus, { ratio: 1, sidechain: true }); kickBus.send(bassBus, { ratio: 1, sidechain: true });
  synthBus.send(hall, { ratio: 0.16 });
  const leadBus = p.addMixerChannel({ name: "Horizon lead", inserts: [duck()] });
  kickBus.send(leadBus, { ratio: 1, sidechain: true }); leadBus.send(echo, { ratio: 0.19 }); leadBus.send(hall, { ratio: 0.13 });
  const keys = p.addChannel({ name: "VSCO upright · felt intro", instrument: piano(p), mixerChannelId: keysBus.id, level: 1.5 });
  const kick = p.addChannel({ name: "Native kick", instrument: drumInstrument(), mixerChannelId: kickBus.id, level: 0.9 });
  const drums = p.addChannel({ name: "Native snare / hats", instrument: drumInstrument(), mixerChannelId: drumsBus.id, level: 0.77 });
  const saw = p.addChannel({ name: "Seven-voice supersaw", mixerChannelId: synthBus.id, level: 0.45,
    instrument: skyChords(),
    effectChain: [fx("saturator", { driveDb: 3, outputDb: -2 }, 0.18)] });
  const sub = p.addChannel({ name: "Mono sub", mixerChannelId: bassBus.id, level: 0.48,
    instrument: wavetable({ oscA: { wave: "sine" }, voiceMode: "mono",
      filter: { cutoff: 240 }, amp: { attack: 0.006, decay: 0.1, sustain: 0.85, release: 0.06 } }) });
  const growl = p.addChannel({ name: "Moving mid bass", mixerChannelId: bassBus.id, level: 0.24,
    instrument: motionBass(),
    effectChain: [fx("saturator", { driveDb: 8, outputDb: -5 }, 0.7)] });
  const lead = p.addChannel({ name: "Horizon · morph lead", mixerChannelId: leadBus.id, level: 0.43,
    instrument: horizonLead() });
  const air = p.addChannel({ name: "Bloom pad / riser", mixerChannelId: synthBus.id, level: 0.1,
    instrument: skyPad() });
  const sparkle = p.addChannel({ name: "Prism · countermelody", mixerChannelId: leadBus.id, pan: -0.22, level: 0.16,
    instrument: tapeGlass() });
  const kt = p.addTrack("Piano · F#m9 / Dmaj9 / Aadd9 / E").use(keys), mt = p.addTrack("Piano · horizon theme").use(keys);
  const kickT = p.addTrack("Kick").use(kick), dt = p.addTrack("Half-time snare / hats / rolls").use(drums);
  kickT.midiChannel = 10; dt.midiChannel = 10;
  const ct = p.addTrack("Supersaw chords").use(saw), st = p.addTrack("Sub").use(sub);
  const wt = p.addTrack("Mid bass · syncopation").use(growl), lt = p.addTrack("Lead · horizon theme").use(lead);
  const at = p.addTrack("Bloom / build tension").use(air);
  const sparkT = p.addTrack("Drop II · answering phrase").use(sparkle);
  const chords = [[54, 61, 64, 68], [50, 57, 61, 64], [57, 61, 64, 71], [52, 59, 64, 68]];
  const roots = [30, 26, 33, 28];
  for (let b = 0; b < dubstep.bars; b++) {
    const drop = b >= 24 && b < 40 || b >= 72 && b < 88;
    const build = b >= 8 && b < 24 || b >= 56 && b < 72;
    const buildPos = b < 24 ? b - 8 : b - 56;
    const end = b >= 96, final = b === 103, second = b >= 72;
    const index = b >= 102 ? 0 : Math.floor(b / 2) % 4, chord = chords[index]!, root = roots[index]!;
    if (!drop || b % 2 === 0) bar(kt, b, chord.map((pitch, i) => note(pitch,
      i * 0.018, final ? 2.5 : 2.9, drop ? 0.66 : 0.46 + i * 0.035)), "Open piano voicing");
    if (!drop && (!build || buildPos < 8) && b < 102) {
      const theme = horizonHook(b, -1).map(hit => ({ ...hit, velocity: hit.velocity * (end ? 0.6 : 0.82) }));
      bar(mt, b, b >= 40 && b < 48 ? theme.slice(0, b % 2 ? 1 : 2) : theme,
        b >= 40 && b < 48 ? "Horizon · distant fragment" : "Horizon · eight-bar piano theme");
    }
    if (b === 102) bar(mt, b, [note(78, 0, 3.4, 0.56)], "F# · home");
    if (drop) {
      const rhythm = second && b >= 80 ? [0, 1.5, 3] : b % 2 ? [0, 0.75, 1.5, 2.5, 3.25] : [0, 1.5, 2.5, 3.5];
      bar(ct, b, rhythm.flatMap((t, i) => chord.slice(1).map(pitch => note(pitch + 12, t,
        second && b >= 80 ? 0.8 : i === 0 ? 0.62 : 0.38, 0.75))), second && b >= 80 ? "Sky · open final chorus" : "Sky chords");
      bar(st, b, rhythm.map(t => note(root, t, t === 0 ? 1.2 : 0.42, 0.9)), "Sub anchor");
      bar(wt, b, [0.75, 1.5, 3, 3.5].map((t, i) => note(root + 12 + (i === 3 && second ? 12 : 0), t, 0.32, 0.8)), "Bass answer");
      bar(lt, b, horizonHook(b), second ? "Horizon · final chorus" : "Horizon · drop hook");
      if (second && b % 2) bar(sparkT, b,
        [note(chord[2]! + 12, 0.125, 0.6, 0.55), note(chord[1]! + 24, 1.75, 0.7, 0.5), note(chord[2]! + 12, 3.125, 0.65, 0.55)], "Prism · answer");
      if (second && b % 2) bar(kt, b, [note(chord[2]! + 24, 1.25, 0.4, 0.7), note(chord[1]! + 24, 3, 0.65, 0.66)], "Piano sparkle");
    }
    if ((build || drop || b >= 44 && b < 56 || b >= 88 && b < 100) && !final) {
      bar(at, b, chord.slice(1).map(pitch => note(pitch + 12, 0, 3.8, build ? 0.42 + buildPos * 0.018 : 0.55)), "Bloom");
    }
    if (drop || build || b >= 88 && b < 96) {
      const ks = drop ? [0, 1.5, ...(b % 2 ? [3.25] : [])] : build && buildPos >= 12 ? [0, 1, 2, 3] : [0];
      if (!(build && buildPos === 15)) bar(kickT, b, ks.map(t => note(36, t, 0.08, drop ? 1 : 0.68)), "Kick / pulse");
      const hits: Hit[] = [];
      if (build && buildPos >= 8) {
        const step = buildPos >= 14 ? 0.25 : buildPos >= 12 ? 0.5 : 1;
        for (let t = 0; t < (buildPos === 15 ? 3.5 : 4); t += step) hits.push(note(38, t, 0.06,
          0.3 + buildPos * 0.016 + (t % 1 === 0 ? 0.08 : 0)));
      } else hits.push(note(38, 2, 0.1, drop ? 0.95 : 0.6));
      if (!(build && buildPos === 15)) {
        for (let i = 0; i < 8; i++) hits.push(note(i === 7 && b % 2 ? 46 : 42, i * 0.5, 0.05, i % 2 ? 0.36 : 0.5));
      }
      if (drop && b % 4 === 3) for (const t of [3.25, 3.5, 3.75]) hits.push(note(38, t, 0.06, 0.38 + (t - 3) * 0.35));
      bar(dt, b, hits, build ? "Build · snare acceleration" : "Half-time / turn");
    }
  }
  growl.automate("oscA.position", automation.polyline([
    { beat: 0, value: 0.32 }, { beat: 96, value: 0.4 }, { beat: 160, value: 0.55 },
    { beat: 288, value: 0.52 }, { beat: 352, value: 0.65 },
  ]));
  air.automate("filter.cutoff", automation.polyline([
    { beat: 0, value: 0.48 }, { beat: 32, value: 0.46 }, { beat: 95, value: 0.93 }, { beat: 96, value: 0.62 },
    { beat: 160, value: 0.44 }, { beat: 224, value: 0.46 }, { beat: 287, value: 0.96 }, { beat: 288, value: 0.64 },
    { beat: 352, value: 0.56 }, { beat: 416, value: 0.38 },
  ]));
  p.master.inserts = [fx("utility", { gainDb: 10 }),
    fx("compressor", { thresholdDb: -12, ratio: 1.6, attackMs: 20, releaseMs: 180, kneeDb: 4 }),
    fx("limit", { ceilingDb: -1.5, releaseMs: 90 })];
  sections(p, dubstep.sections, dubstep.bars, 1);
  return p;
}

export default function createProject() { return preview(createDubstepSong()); }
