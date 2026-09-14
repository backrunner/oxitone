import { effect, type Project } from "@oxitone/core";
import { electronicKit } from "../electronic-kit.js";
import { snareClap } from "./bass-patches.js";
import {
  kickAttack,
  snareBody,
  snareTail,
  rideCymbal,
  shaker,
  tomFill,
  crashCymbal,
  reverseCymbal,
  buildRiser,
} from "./drum-patches.js";
import { fx } from "./shared.js";

const hp = (cutoffHz: number) => fx("filter", { mode: 1, cutoffHz, resonance: 0.707 });
const lp = (cutoffHz: number) => fx("filter", { mode: 0, cutoffHz, resonance: 0.707 });

/** Dry kick/snare stay forward; tops, build rolls and short room have separate gain paths. */
export function createDrumMix(p: Project) {
  const room = p.addMixerChannel({
    name: "Drum room · gated-size space",
    inserts: [
      effect("reverb", { decaySeconds: 0.38, predelayMs: 14, highpassHz: 1100, lowpassHz: 7000, width: 1.25 }),
      effect("convolver", { highpassHz: 900, lowpassHz: 6500 }, { mix: 0.18 }),
    ],
  });
  const kickBus = p.addMixerChannel({
    name: "Kick · detector",
    inserts: [
      hp(30),
      fx("eq", { "band2.freqHz": 290, "band2.q": 0.9, "band2.gainDb": -2.2 }),
      fx("distortion", { mode: 0, driveDb: 5, outputDb: -3, toneHz: 11500 }, 0.24),
    ],
  });
  const snareBus = p.addMixerChannel({
    name: "Snare · body / crack / tail",
    inserts: [
      hp(130),
      fx("eq", {
        "band2.freqHz": 195,
        "band2.q": 0.9,
        "band2.gainDb": 1.5,
        "band3.freqHz": 2300,
        "band3.q": 0.8,
        "band3.gainDb": 1.2,
      }),
      fx("distortion", { driveDb: 7, outputDb: -4, toneHz: 12500 }, 0.32),
      effect("compressor", { thresholdDb: -14, ratio: 1.8, attackMs: 25, releaseMs: 90, makeupDb: 3 }),
    ],
  });
  snareBus.send(room, { ratio: 0.16 });
  const topBus = p.addMixerChannel({ name: "Tops · hats / ride / shaker", level: 1.3, inserts: [hp(2500), lp(14500)] });
  topBus.send(room, { ratio: 0.055 });
  const buildBus = p.addMixerChannel({
    name: "Build · rolls / risers / fills",
    inserts: [
      hp(130),
      effect(
        "compactor",
        { thresholdDb: -24, upwardDb: 3, transient: 0.05, attackMs: 8, releaseMs: 55 },
        { mix: 0.35 },
      ),
    ],
  });
  buildBus.send(room, { ratio: 0.09 });
  const kick = p.addChannel({
    name: "Circuit · punch kick",
    instrument: {
      ...electronicKit(),
      parameters: {
        ...electronicKit().parameters,
        kickTune: 54,
        kickSweep: 240,
        kickDecay: 0.68,
        kickClick: 0.42,
      },
    },
    mixerChannelId: kickBus.id,
    level: 1.35,
  });
  const click = p.addChannel({
    name: "Drum · kick attack",
    instrument: kickAttack(),
    mixerChannelId: kickBus.id,
    level: 0.42,
  });
  const drums = p.addChannel({
    name: "Circuit · snare crack",
    instrument: {
      ...electronicKit(),
      parameters: {
        ...electronicKit().parameters,
        snareSnap: 0.58,
        snareDecay: 1.25,
      },
    },
    mixerChannelId: snareBus.id,
    level: 0.9,
    effectChain: [lp(11000)],
  });
  const body = p.addChannel({
    name: "Drum · snare body",
    instrument: snareBody(),
    mixerChannelId: snareBus.id,
    level: 1.2,
    effectChain: [hp(155)],
  });
  const tail = p.addChannel({
    name: "Drum · snare noise tail",
    instrument: snareTail(),
    mixerChannelId: snareBus.id,
    level: 1.7,
    effectChain: [lp(7600), effect("spreader", { width: 1.1, amount: 0.14, bassMonoHz: 1000 })],
  });
  const clap = p.addChannel({
    name: "Clap · staggered width",
    instrument: snareClap(),
    mixerChannelId: snareBus.id,
    level: 0.9,
    effectChain: [hp(1100)],
  });
  const tops = p.addChannel({
    name: "Circuit · closed / open hats",
    instrument: electronicKit(),
    mixerChannelId: topBus.id,
    level: 0.7,
  });
  const ride = p.addChannel({
    name: "Drum · ride metal",
    instrument: rideCymbal(),
    mixerChannelId: topBus.id,
    level: 0.6,
    pan: 0.25,
  });
  const shake = p.addChannel({
    name: "Drum · sixteenth shaker",
    instrument: shaker(),
    mixerChannelId: topBus.id,
    level: 0.32,
    pan: -0.35,
  });
  const tom = p.addChannel({
    name: "Drum · descending tom fill",
    instrument: tomFill(),
    mixerChannelId: buildBus.id,
    level: 0.6,
    effectChain: [hp(100)],
  });
  const roll = p.addChannel({
    name: "Circuit · build snare roll",
    instrument: {
      ...electronicKit(),
      parameters: {
        ...electronicKit().parameters,
        snareDecay: 0.58,
        snareSnap: 0.6,
      },
    },
    mixerChannelId: buildBus.id,
    level: 0.85,
    effectChain: [hp(330), lp(9500)],
  });
  const pulse = p.addChannel({
    name: "Circuit · build kick pulse",
    instrument: {
      ...electronicKit(),
      parameters: {
        ...electronicKit().parameters,
        kickDecay: 0.5,
        kickClick: 0.12,
      },
    },
    mixerChannelId: buildBus.id,
    level: 0.75,
    effectChain: [hp(95), lp(2500)],
  });
  const impact = p.addChannel({
    name: "Drum · crash arrivals",
    instrument: crashCymbal(),
    mixerChannelId: topBus.id,
    level: 0.7,
  });
  const reverse = p.addChannel({
    name: "Drum · reverse cymbal",
    instrument: reverseCymbal(),
    mixerChannelId: buildBus.id,
    level: 0.35,
  });
  const riser = p.addChannel({
    name: "Air · tonal tension riser",
    instrument: buildRiser(),
    mixerChannelId: buildBus.id,
    level: 0.14,
    effectChain: [hp(750)],
  });
  return {
    kick,
    click,
    drums,
    body,
    tail,
    clap,
    tops,
    ride,
    shake,
    tom,
    roll,
    pulse,
    impact,
    reverse,
    riser,
    kickBus,
    snareBus,
    topBus,
    buildBus,
  };
}
