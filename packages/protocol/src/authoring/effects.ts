import { z } from "zod";

const n = (min: number, max: number) => z.number().finite().min(min).max(max).optional();
const e = (max: number) => z.number().int().min(0).max(max).optional();
/** Physical values. Host mix/bypass remain outside the DSP parameter table. */
export const effectParameterSchemas = {
  compressor: z
    .object({
      thresholdDb: n(-60, 0),
      ratio: n(1, 40),
      attackMs: n(0.1, 200),
      releaseMs: n(10, 2000),
      kneeDb: n(0, 24),
      makeupDb: n(0, 24),
      detector: e(1),
      sidechainHighpassHz: n(20, 2000),
    })
    .strict(),
  gate: z
    .object({
      thresholdDb: n(-80, 0),
      attackMs: n(0.01, 100),
      holdMs: n(0, 500),
      releaseMs: n(1, 2000),
      hysteresisDb: n(0, 24),
      rangeDb: n(-120, 0),
    })
    .strict(),
  delay: z
    .object({
      timeBeats: n(0.03125, 8),
      timeSeconds: n(0.001, 10),
      feedback: n(0, 0.95),
      feedbackFilterHz: n(100, 18000),
      pingPong: e(1),
      highpassHz: n(20, 2000),
      ducking: n(0, 1),
    })
    .strict(),
  reverb: z
    .object({
      decaySeconds: n(0.1, 12),
      damping: n(0, 1),
      predelayMs: n(0, 100),
      highpassHz: n(20, 2000),
      lowpassHz: n(1000, 20000),
      ducking: n(0, 1),
      width: n(0, 2),
    })
    .strict(),
  nonlinearFilter: z
    .object({ mode: e(3), cutoffHz: n(20, 20000), resonance: n(0, 1), driveDb: n(0, 30), outputDb: n(-30, 12) })
    .strict(),
  compactor: z
    .object({
      thresholdDb: n(-60, 0),
      upwardDb: n(0, 24),
      attackMs: n(0.1, 100),
      releaseMs: n(10, 1000),
      transient: n(-1, 1),
      outputDb: n(-24, 12),
    })
    .strict(),
  multibandDynamics: z
    .object({
      depth: n(0, 1),
      time: n(0.1, 10),
      upwardDb: n(0, 36),
      downwardRatio: n(1, 40),
      lowHz: n(60, 500),
      highHz: n(1000, 8000),
      lowGainDb: n(-18, 18),
      midGainDb: n(-18, 18),
      highGainDb: n(-18, 18),
      outputDb: n(-24, 12),
    })
    .strict(),
  resonator: z
    .object({
      frequencyHz: n(20, 4000),
      decaySeconds: n(0.02, 8),
      spread: n(0, 1),
      brightness: n(0, 1),
      inharmonicity: n(0, 1),
      outputDb: n(-36, 12),
    })
    .strict(),
  frequencyShifter: z.object({ shiftHz: n(-5000, 5000), stereoHz: n(-100, 100), outputDb: n(-24, 12) }).strict(),
  pitchShifter: z.object({ semitones: n(-24, 24), cents: n(-100, 100), outputDb: n(-24, 12) }).strict(),
  flanger: z
    .object({ rateHz: n(0.01, 10), delayMs: n(0.2, 10), depthMs: n(0, 10), feedback: n(-0.95, 0.95), stereo: n(0, 1) })
    .strict(),
  convolver: z
    .object({ predelayMs: n(0, 200), highpassHz: n(20, 2000), lowpassHz: n(1000, 20000), outputDb: n(-36, 12) })
    .strict(),
  bitcrush: z.object({ bits: n(2, 24), rateHz: n(200, 48000), jitter: n(0, 1), outputDb: n(-24, 12) }).strict(),
  tape: z
    .object({ driveDb: n(0, 24), wow: n(0, 1), flutter: n(0, 1), toneHz: n(1000, 20000), outputDb: n(-24, 12) })
    .strict(),
  spreader: z.object({ width: n(0, 2), amount: n(0, 1), bassMonoHz: n(20, 1000) }).strict(),
  limiter: z.object({ ceilingDb: n(-12, 0), releaseMs: n(5, 1000), inputDb: n(-12, 24) }).strict(),
} as const;

export const effectPluginIds = {
  compressor: "oxitone.compressor",
  gate: "oxitone.gate",
  delay: "oxitone.delay",
  reverb: "oxitone.reverb",
  nonlinearFilter: "oxitone.nonlinear-filter",
  compactor: "oxitone.compactor",
  multibandDynamics: "oxitone.multiband-dynamics",
  resonator: "oxitone.resonator",
  frequencyShifter: "oxitone.frequency-shifter",
  pitchShifter: "oxitone.pitch-shifter",
  flanger: "oxitone.flanger",
  convolver: "oxitone.convolver",
  bitcrush: "oxitone.bitcrush",
  tape: "oxitone.tape",
  spreader: "oxitone.spreader",
  limiter: "oxitone.limiter",
} as const;
export type EffectKind = keyof typeof effectParameterSchemas;
export type EffectParameters<K extends EffectKind> = z.infer<(typeof effectParameterSchemas)[K]>;
