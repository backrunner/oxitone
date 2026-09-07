import { Project } from "@oxitone/core";
import { bar, note, preview, sections, type Hit } from "./shared.js";
import { horizonHook } from "./themes.js";
import { createMix } from "./mix.js";
import { applyMotion } from "./motion.js";
import { bassAnswer, dropHats, dropPhrase } from "./phrasing.js";

export const dubstep = { slug: "after-the-horizon", title: "After the Horizon / 地平线之后", bpm: 140, bars: 104,
  sections: [["First light / Crystal", 0], ["Lift / Build I", 8], ["Open sky / Drop I", 24],
    ["Weightless / Break", 40], ["Signal / Build II", 56], ["Beyond / Drop II", 72],
    ["Afterglow / Reprise", 88], ["Horizon / Outro", 96]] as const };

/** 104 bars in F# minor; crystal hook, contrasted builds and layered half-time drops. */
export function createDubstepSong(): Project {
  const p = new Project({ name: dubstep.title, seed: 2026090802 }); p.setTempo(dubstep.bpm);
  const mix = createMix(p);
  const { keys, kick, drums, tops, saw, body, sub, growl, vowel, lead, air, sparkle, arp, lift, fall, impact,
    pluck, reese, shimmer } = mix;
  const kt = p.addTrack("Crystal · F#m9 / Dmaj9 / Aadd9 / E").use(keys), mt = p.addTrack("Crystal · horizon theme").use(keys);
  const kickT = p.addTrack("Kick").use(kick), dt = p.addTrack("Half-time snare / hats / rolls").use(drums);
  const ct = p.addTrack("Supersaw chords").use(saw), st = p.addTrack("Sub").use(sub);
  const wt = p.addTrack("Mid bass · syncopation").use(growl), lt = p.addTrack("Lead · horizon theme").use(lead);
  const at = p.addTrack("Bloom / build tension").use(air);
  const sparkT = p.addTrack("Drop II · answering phrase").use(sparkle);
  const bodyT = p.addTrack("Chords · center body").use(body), vowelT = p.addTrack("Vowel bass · response").use(vowel);
  const topT = p.addTrack("Metallic tops / shuffle").use(tops), arpT = p.addTrack("Orbit · pluck movement").use(arp);
  const liftT = p.addTrack("Noise · build lifts").use(lift), fallT = p.addTrack("Noise · downlifters").use(fall);
  const impactT = p.addTrack("Drop / phrase impacts").use(impact);
  const pluckT = p.addTrack("Ember · chord pluck answers").use(pluck);
  const reeseT = p.addTrack("Undertow · bridge bassline").use(reese);
  const haloT = p.addTrack("Halo · final chorus air").use(shimmer);
  const duckHits: number[] = [], chordAttacks: number[] = [];
  const chords = [[54, 61, 64, 68], [50, 57, 61, 64], [57, 61, 64, 71], [52, 59, 64, 68]];
  const roots = [30, 26, 33, 28];
  for (let b = 0; b < dubstep.bars; b++) {
    const drop = b >= 24 && b < 40 || b >= 72 && b < 88;
    const build = b >= 8 && b < 24 || b >= 56 && b < 72;
    const buildPos = b < 24 ? b - 8 : b - 56;
    const end = b >= 96, final = b === 103;
    const index = b >= 102 ? 0 : Math.floor(b / 2) % 4, chord = chords[index]!, root = roots[index]!;
    if (!drop && !(build && buildPos >= 12)) bar(kt, b, chord.map((pitch, i) => note(pitch,
      i * 0.018, final ? 2.5 : 2.9, 0.56 + i * 0.035)), "Crystal · open voicing");
    if (!drop && (!build || buildPos < 8) && b < 102) {
      const theme = horizonHook(b, -1).map(hit => ({ ...hit, velocity: hit.velocity * (end ? 0.6 : 0.82) }));
      bar(mt, b, b >= 40 && b < 48 ? theme.slice(0, b % 2 ? 1 : 2) : theme,
        b >= 40 && b < 48 ? "Horizon · distant fragment" : "Horizon · eight-bar crystal theme");
    }
    if (b === 102) bar(mt, b, [note(78, 0, 3.4, 0.56)], "F# · home");
    if (drop) {
      const { accents: rhythm, open, second, position, turn } = dropPhrase(b);
      chordAttacks.push(...rhythm.map(t => b * 4 + t));
      bar(ct, b, rhythm.flatMap((t, i) => chord.slice(1).map(pitch => note(pitch + 12, t,
        open ? 0.85 : i === 0 ? 0.62 : 0.34, i === 0 ? 0.8 : 0.7))), open ? "Sky · open final chorus" : "Sky · rhythmic bloom");
      bar(st, b, [note(root, 0, 1.35, 0.88), note(root, 1.5, 0.83, 0.82),
        note(root, 2.5, turn ? 0.7 : 1.27, 0.85)], "Sub · connected foundation");
      bar(bodyT, b, rhythm.flatMap(t => chord.slice(0, 3).map(pitch => note(pitch, t, open ? 0.7 : 0.35, 0.66))), "Sky · chord foundation");
      bar(wt, b, bassAnswer(root, b), turn ? "Signal · turnaround" : "Signal · bass answer");
      bar(lt, b, horizonHook(b), second ? "Horizon · final chorus" : "Horizon · drop hook");
      if (second && b % 2) bar(sparkT, b,
        [note(chord[2]! + 12, 1.75, 0.6, 0.5), note(chord[1]! + 24, 3.0, 0.24, 0.46)], "Prism · answer");
      if (b % 2 && (position >= 4 || second)) bar(vowelT, b,
        [note(root + 12, 1.125, 0.22, 0.72), note(root + 24, 3.125, 0.22, 0.76)], "Formant · call / response");
      if (position >= 4 && b % 4 === 2) bar(arpT, b, [0.25, 1.25, 2.75, 3.75].map((t, i) =>
        note(chord[1 + i % 3]! + 24, t, 0.16, 0.46)), "Orbit · phrase sparkle");
      if (position >= 4 && !turn) bar(pluckT, b, [0.5, 2.75].flatMap(t =>
        chord.slice(1).map(pitch => note(pitch, t, 0.2, 0.64))), "Ember · offbeat chord answer");
      if (open) bar(haloT, b, [note(chord[2]! + 24, 0.25, 3.2, 0.5)], "Halo · final lift");
    }
    if ((build || drop && b % 8 >= 4 || b >= 44 && b < 56 || b >= 88 && b < 100) && !final) {
      bar(at, b, chord.slice(1).map(pitch => note(pitch + 12, 0, build && buildPos === 15 ? 3.35 : 3.8,
        build ? 0.38 + buildPos * 0.014 : 0.45)), "Bloom · widening horizon");
    }
    if (b >= 48 && b < 56 || build && buildPos < 8 || b >= 88 && b < 96) {
      bar(reeseT, b, [note(root + 12, 0, 1.8, 0.62), note(root + 12, 2.5, 1.25, 0.54)], "Undertow · rolling bridge");
      if (b % 2) bar(pluckT, b, [0.75, 2.5, 3.25].flatMap(t =>
        chord.slice(1).map(pitch => note(pitch, t, 0.18, 0.5))), "Ember · bridge pulse");
    }
    if (build && buildPos >= 4) {
      bar(liftT, b, [note(72, 0, buildPos === 15 ? 3.25 : 3.9, 0.08 + buildPos * 0.026)], "Air · rising tension");
      if (buildPos < 15) bar(arpT, b, Array.from({ length: buildPos >= 12 ? 16 : 8 }, (_,i) =>
        note(chord[1 + i % 3]! + 12, i * (buildPos >= 12 ? 0.25 : 0.5), 0.17, 0.35 + buildPos * 0.015)), "Orbit · accelerating lift");
    }
    if ([24, 32, 40, 72, 80, 88, 96].includes(b)) {
      bar(fallT, b, [note(72, 0, 2.2, b === 40 || b === 96 ? 0.3 : 0.58)], "Air · transition wash");
      bar(impactT, b, [note(46, 0, 0.1, b === 40 || b === 96 ? 0.45 : 0.9)], "Impact · phrase arrival");
    }
    if (b >= 48 && b < 56 || b >= 88 && b < 100) bar(st, b,
      [note(root, 0, 2.9, b >= 96 ? 0.35 : 0.52)], "Sub · reprise foundation");
    if (drop || build || b >= 88 && b < 96) {
      const ks = drop ? dropPhrase(b).kicks : build && buildPos >= 12 ? [0, 1, 2, 3] : [0];
      if (!(build && buildPos === 15)) bar(kickT, b, ks.map(t => note(36, t, 0.08, drop ? 1 : 0.68)), "Kick / pulse");
      if (drop) duckHits.push(...ks.map(t => b * 4 + t), b * 4 + 2);
      const hits: Hit[] = [];
      if (build && buildPos >= 8) {
        const step = buildPos >= 14 ? 0.25 : buildPos >= 12 ? 0.5 : 1;
        for (let t = 0; t < (buildPos === 15 ? 3.5 : 4); t += step) hits.push(note(38, t, 0.06,
          0.3 + buildPos * 0.016 + (t % 1 === 0 ? 0.08 : 0)));
      } else hits.push(note(38, 2, 0.1, drop ? 0.95 : 0.6));
      if (!(build && buildPos === 15)) {
        const hats = drop ? dropHats(b) : Array.from({ length: build && buildPos < 8 ? 4 : 8 }, (_, i) =>
          note(42, i * (build && buildPos < 8 ? 1 : 0.5) + 0.02, 0.05, i % 2 ? 0.4 : 0.3));
        bar(topT, b, hats, "Metal · offbeat / shuffle");
      }
      if (drop && b % 4 === 3) for (const t of b % 8 === 7 ? [3, 3.5, 3.75] : [3.75])
        hits.push(note(38, t, 0.06, 0.32 + (t - 3) * 0.3));
      bar(dt, b, hits, build ? "Build · snare acceleration" : "Half-time / turn");
    }
  }
  applyMotion(mix, duckHits, chordAttacks);
  // Final fader trim is included in offline true-peak validation, after the insert ceiling.
  sections(p, dubstep.sections, dubstep.bars, 1.2);
  return p;
}

export default function createProject() { return preview(createDubstepSong()); }
