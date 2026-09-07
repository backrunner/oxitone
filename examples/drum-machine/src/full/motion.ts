import type { createMix } from "./mix.js";
import { automation } from "./shared.js";
import { applyBuildMotion } from "./build-motion.js";

type Mix = ReturnType<typeof createMix>;
const hz = (value: number) => Math.log(value / 20) / Math.log(1000);
const curve = (points: Map<number, number>) => automation.polyline(
  [...points].sort(([a], [b]) => a - b).map(([beat, value]) => ({ beat, value })));

/** Gain recovery follows the actual kick/snare events; spectral motion follows chord attacks. */
export function applyMotion(mix: Mix, hits: number[], chordAttacks: number[], kicks: number[]) {
  applyBuildMotion(mix.percussion);
  const ordered = [...new Set(hits)].sort((a, b) => a - b);
  for (const [bus, floor] of [[mix.music, 0.1], [mix.hall, 0.05], [mix.echo, 0.08], [mix.throws, 0.08]] as const) {
    const points = new Map<number, number>([[0, 0.5]]);
    for (const [i, beat] of ordered.entries()) {
      const snare = beat % 4 === 2;
      const recovery = Math.min(snare ? 0.66 : 0.38, (ordered[i + 1] ?? beat + 1) - beat - 0.015);
      if (beat > 0.012) points.set(beat - 0.012, 0.5);
      points.set(beat, floor); points.set(beat + recovery * (snare ? 0.32 : 0.2), floor + 0.02);
      points.set(beat + recovery * 0.58, 0.36); points.set(beat + recovery, 0.5);
    }
    for (const end of [96, 288]) {
      points.set(end - 1.2, 0.5); points.set(end - 0.9, 0);
      points.set(end - 0.012, 0);
    }
    bus.automate("level", curve(points));
  }
  for (const [bus, floor] of [[mix.subBus, 0.07], [mix.lowBus, 0.14]] as const) {
    const points = new Map<number, number>([[0, 0.5]]);
    for (const beat of kicks) {
      points.set(beat - 0.012, 0.5); points.set(beat, floor);
      points.set(beat + 0.075, floor); points.set(beat + 0.18, 0.35); points.set(beat + 0.28, 0.5);
    }
    bus.automate("level", curve(points));
  }
  const brightness = new Map<number, number>([[0, hz(8000)]]);
  for (const beat of chordAttacks) {
    const open = beat >= 320 ? 12000 : beat >= 288 ? 11000 : 9500;
    brightness.set(beat, hz(open * 0.55)); brightness.set(beat + 0.08, hz(open));
    brightness.set(beat + 0.42, hz(open * 0.72));
  }
  mix.saw.automate("filter.cutoff", curve(brightness));
  mix.growl.automate("oscA.position", automation.polyline([
    { beat: 0, value: 0.26 }, { beat: 96, value: 0.3 }, { beat: 128, value: 0.42 },
    { beat: 160, value: 0.34 }, { beat: 288, value: 0.44 }, { beat: 320, value: 0.56 }, { beat: 352, value: 0.38 },
  ]));
  mix.air.automate("filter.cutoff", automation.polyline([
    { beat: 0, value: hz(1500) }, { beat: 32, value: hz(1400) }, { beat: 94, value: hz(7200) },
    { beat: 96, value: hz(2800) }, { beat: 160, value: hz(1100) }, { beat: 224, value: hz(1800) },
    { beat: 286, value: hz(8300) }, { beat: 288, value: hz(3200) }, { beat: 352, value: hz(2400) },
    { beat: 416, value: hz(650) },
  ]));
  mix.lift.automate("filter.cutoff", automation.polyline([
    { beat: 0, value: hz(800) }, { beat: 48, value: hz(800) }, { beat: 94, value: hz(13000) },
    { beat: 96, value: hz(800) }, { beat: 240, value: hz(800) }, { beat: 286, value: hz(14500) },
    { beat: 288, value: hz(800) },
  ]));
  // Release the roll and pad before the downbeat; the new kick arrives into a pocket.
  const liftGain = new Map<number, number>([[0, 0.095]]), padGain = new Map<number, number>([[0, 0.08]]);
  for (const end of [96, 288]) {
    liftGain.set(end - 2, 0.095); liftGain.set(end - 0.9, 0); liftGain.set(end, 0);
    liftGain.set(end + 0.01, 0.095);
    padGain.set(end - 2, 0.08); padGain.set(end - 0.9, 0); padGain.set(end, 0); padGain.set(end + 0.25, 0.08);
  }
  mix.lift.automate("level", curve(liftGain)); mix.air.automate("level", curve(padGain));
  const throws = new Map<number, number>([[0, 0]]);
  for (const bar of [31, 39, 79, 87]) {
    const beat = bar * 4;
    throws.set(beat + 3.2, 0); throws.set(beat + 3.25, 0.26);
    throws.set(beat + 3.85, 0.26); throws.set(beat + 3.98, 0);
  }
  mix.leadBus.automate(`send.${mix.throws.id}.ratio`, curve(throws));
}
