import { note, type Hit } from "./shared.js";
type Phrase = readonly (readonly (readonly [pitch: number, start: number, duration: number])[])[];

// Eight-bar melodies: repeated rhythmic cells, answering phrases, space and a cadence.
// Chords change underneath the recurring hook instead of turning every bar into an arpeggio.
const windowTheme: Phrase = [
  [[69, 0.5, 0.4], [72, 1.25, 0.4], [74, 2, 1.35]],
  [[72, 0.5, 0.5], [69, 1.5, 0.4], [65, 2.25, 0.6], [64, 3.25, 0.45]],
  [[69, 0.5, 0.4], [72, 1.25, 0.4], [74, 2, 1.35]],
  [[77, 0.5, 0.5], [76, 1.25, 0.5], [74, 2.25, 0.75], [72, 3.5, 0.35]],
  [[72, 0.5, 0.5], [69, 1.25, 0.5], [67, 2.25, 1.25]],
  [[69, 0.5, 0.4], [72, 1.25, 0.4], [76, 2, 1.5]],
  [[74, 0.5, 0.5], [72, 1.25, 0.4], [67, 2, 1.35]],
  [[64, 0.5, 0.5], [67, 1.25, 0.5], [69, 2.5, 0.8], [72, 3.5, 0.35]],
];
const horizonTheme: Phrase = [
  [[73, 0, 0.4], [73, 0.75, 0.4], [76, 1.5, 0.4], [78, 2, 1.5]],
  [[81, 0.25, 0.6], [80, 1, 0.5], [78, 2, 1.5]],
  [[73, 0, 0.4], [73, 0.75, 0.4], [76, 1.5, 0.4], [78, 2, 1.5]],
  [[76, 0.25, 0.5], [74, 1, 0.5], [73, 2, 0.75], [69, 3.25, 0.6]],
  [[76, 0, 0.5], [78, 0.75, 0.4], [76, 1.5, 0.4], [73, 2, 1.5]],
  [[71, 0.25, 0.5], [73, 1, 0.5], [76, 2, 1], [78, 3.5, 0.4]],
  [[80, 0, 1], [83, 1.5, 0.75], [80, 2.5, 0.5], [76, 3.25, 0.6]],
  [[78, 0, 0.5], [76, 0.75, 0.4], [73, 1.5, 0.4], [71, 2.25, 0.5], [73, 3.25, 0.6]],
];
export function windowHook(bar: number, octave = 0): Hit[] {
  return windowTheme[bar % 8]!.map(([pitch, start, duration], i) => note(pitch + octave * 12, start, duration, i === 2 ? 0.68 : 0.6));
}
export function horizonHook(bar: number, octave = 0): Hit[] {
  return horizonTheme[bar % 8]!.map(([pitch, start, duration], i) => note(pitch + octave * 12, start, duration, i === 3 ? 0.86 : 0.77));
}
