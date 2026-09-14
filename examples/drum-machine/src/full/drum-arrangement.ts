import type { Project } from "@oxitone/core";
import type { createDrumMix } from "./drum-mix.js";
import { bar, note, type Hit } from "./shared.js";
import { dropHats, dropPhrase } from "./phrasing.js";

export function createDrumArrangement(p: Project, d: ReturnType<typeof createDrumMix>) {
  const kick = p.addTrack("Kick").use(d.kick),
    click = p.addTrack("Kick · attack layer").use(d.click);
  const snare = p.addTrack("Half-time snare · crack").use(d.drums),
    body = p.addTrack("Snare · body layer").use(d.body);
  const tail = p.addTrack("Snare · noise tail layer").use(d.tail),
    clap = p.addTrack("Clap · snare layer").use(d.clap);
  const hats = p.addTrack("Metallic tops / shuffle").use(d.tops),
    ride = p.addTrack("Ride · drop drive").use(d.ride);
  const shake = p.addTrack("Shaker · sixteenth motion").use(d.shake),
    tom = p.addTrack("Toms · phrase fills").use(d.tom);
  const roll = p.addTrack("Build · accelerating snare roll").use(d.roll),
    pulse = p.addTrack("Build · kick pulse").use(d.pulse);
  const crash = p.addTrack("Drop / phrase impacts").use(d.impact),
    reverse = p.addTrack("Reverse · phrase pickup").use(d.reverse);
  const riser = p.addTrack("Build · tonal riser").use(d.riser);
  const duckHits: number[] = [],
    kickHits: number[] = [];
  return {
    duckHits,
    kickHits,
    write(b: number) {
      const drop = (b >= 24 && b < 40) || (b >= 72 && b < 88);
      const build = (b >= 8 && b < 24) || (b >= 56 && b < 72);
      const buildPos = b < 24 ? b - 8 : b - 56;
      if (drop) {
        const { kicks, position, second, turn } = dropPhrase(b);
        kickHits.push(...kicks.map((t) => b * 4 + t));
        duckHits.push(...kicks.map((t) => b * 4 + t), b * 4 + 2);
        bar(
          kick,
          b,
          kicks.map((t) => note(36, t, 0.08, 1)),
          "Kick · half-time impact",
        );
        bar(
          click,
          b,
          kicks.map((t) => note(60, t, 0.05, 0.88)),
          "Kick · high attack",
        );
        const ghosts: Hit[] = position % 4 === 3 ? [note(38, 3.5, 0.07, 0.38)] : [];
        bar(snare, b, [note(38, 2, 0.12, 1), ...ghosts], "Snare · main crack / ghost");
        bar(body, b, [note(54, 2, 0.62, 0.92)], "Snare · 185 Hz body");
        bar(tail, b, [note(60, 2.012, 0.4, 0.87)], "Snare · wide noise sustain");
        bar(
          clap,
          b,
          [note(60, 2.007, 0.06, 0.48), note(60, 2.038, 0.08, 0.74), note(60, 2.073, 0.2, 0.55)],
          "Clap · three staggered strikes",
        );
        bar(hats, b, dropHats(b), "Metal · open / closed drive");
        if (second || position >= 4)
          bar(
            ride,
            b,
            (turn ? [0, 1, 2] : [0, 1, 2, 3]).map((t) => note(89, t + 0.015, 0.4, t % 2 ? 0.52 : 0.66)),
            "Ride · quarter-note lift",
          );
        bar(
          shake,
          b,
          Array.from({ length: turn ? 12 : 16 }, (_, i) =>
            note(60, i * 0.25 + (i % 2 ? 0.022 : 0.005), 0.065, i % 4 === 2 ? 0.64 : i % 2 ? 0.38 : 0.22),
          ),
          "Shaker · layered subdivision",
        );
        if (position % 4 === 3)
          bar(
            tom,
            b,
            (turn ? [3, 3.25, 3.625] : [3.25, 3.75]).map((t, i) => note([50, 47, 42][i]!, t, 0.2, 0.75 + i * 0.06)),
            "Toms · descending turnaround",
          );
      } else if (build) {
        const step = buildPos < 8 ? 1 : buildPos < 12 ? 0.5 : buildPos < 15 ? 0.25 : 0.125;
        const end = buildPos === 15 ? 3 : 4;
        bar(
          roll,
          b,
          Array.from({ length: Math.round(end / step) }, (_, i) =>
            note(38, i * step, 0.04, (buildPos < 8 ? 0.32 : 0.43) + buildPos * 0.018 + (i % 4 === 0 ? 0.11 : 0)),
          ),
          "Build · quarter → eighth → sixteenth → thirty-second",
        );
        const pulseStep = buildPos < 12 ? 1 : 0.5;
        bar(
          pulse,
          b,
          Array.from({ length: Math.floor(end / pulseStep) }, (_, i) =>
            note(36, i * pulseStep, 0.06, 0.45 + buildPos * 0.023),
          ),
          "Build · tightening kick pulse",
        );
        if (buildPos >= 8)
          bar(riser, b, [note(66, 0, buildPos === 15 ? 2.9 : 3.96, 0.65)], "Build · rising gated tone");
        if (buildPos < 15)
          bar(
            hats,
            b,
            [0.5, 1.5, 2.5, 3.5].map((t) => note(42, t, 0.07, 0.4)),
            "Build · offbeat hat",
          );
        if (buildPos === 15)
          bar(tom, b, [note(50, 2.5, 0.16, 0.7), note(42, 2.75, 0.18, 0.85)], "Fill · pre-drop call");
      } else if (b >= 88 && b < 96) {
        bar(kick, b, [note(36, 0, 0.08, 0.64)], "Reprise · kick pulse");
        bar(snare, b, [note(38, 2, 0.12, 0.54)], "Reprise · distant snare");
        bar(
          hats,
          b,
          [0.5, 1.5, 2.5, 3.5].map((t) => note(42, t, 0.06, 0.27)),
          "Reprise · light hats",
        );
      }
      if ([24, 28, 32, 36, 40, 72, 76, 80, 84, 88, 96].includes(b))
        bar(crash, b, [note(89, 0, 1.7, drop ? 0.85 : 0.4)], "Crash · dry arrival + short space");
      if ([23, 31, 39, 71, 79, 87, 95].includes(b))
        bar(reverse, b, [note(60, build ? 2.08 : 3.08, 0.88, 0.85)], "Reverse · intake");
    },
  };
}
