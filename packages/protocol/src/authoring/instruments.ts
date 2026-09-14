import { z } from "zod";
import { beatWireSchema } from "../base/beat.js";
import { entityIdSchema, frameWireSchema } from "../base/primitives.js";
import { curvedEnvelopeSchema, modulationRouteSchema } from "./synth-modulation.js";

const range = (min: number, max: number) => z.number().finite().min(min).max(max);
const wave = z.enum(["sine", "saw", "square", "triangle", "organ", "glass"]);
export const envelopeOptionsSchema = z.strictObject({
  attack: range(0, 8).optional(),
  decay: range(0, 8).optional(),
  sustain: range(0, 1).optional(),
  release: range(0, 8).optional(),
});
export type EnvelopeOptions = z.infer<typeof envelopeOptionsSchema>;
export const oscillatorOptionsSchema = z.strictObject({
  wave: wave.optional(),
  morphTo: wave.optional(),
  position: range(0, 1).optional(),
  phase: range(0, 1).optional(),
  phaseSpread: range(0, 1).optional(),
  pitch: range(-24, 24).optional(),
  unison: range(1, 16).int().optional(),
  octave: range(-4, 4).int().optional(),
  level: range(0, 1).optional(),
  detune: range(0, 100).optional(),
  spread: range(0, 1).optional(),
  bank: z.enum(["pair", "analog", "digital", "vowel"]).optional(),
  warpMode: z.enum(["off", "bend", "asymmetric", "sync"]).optional(),
  warp: range(0, 1).optional(),
});
export type OscillatorOptions = z.infer<typeof oscillatorOptionsSchema>;
export const wavetableOptionsSchema = z.strictObject({
  oscA: oscillatorOptionsSchema.optional(),
  oscB: oscillatorOptionsSchema.optional(),
  mix: range(0, 1).optional(),
  filter: z
    .strictObject({
      type: z.enum(["lowpass", "highpass", "bandpass"]).optional(),
      cutoff: range(20, 20000).optional(),
      resonance: range(0, 1).optional(),
    })
    .optional(),
  filterEnvelope: curvedEnvelopeSchema.extend({ amount: range(-48, 48).optional() }).optional(),
  amp: curvedEnvelopeSchema.optional(),
  voiceMode: z.enum(["poly", "mono", "legato"]).optional(),
  sub: z
    .strictObject({
      level: range(0, 1).optional(),
      octave: range(-4, 4).int().optional(),
      wave: z.enum(["sine", "triangle", "saw", "square", "pulse", "rounded"]).optional(),
    })
    .optional(),
  noise: z.strictObject({ level: range(0, 1).optional() }).optional(),
  lfo: z
    .strictObject({
      shape: z.enum(["sine", "triangle", "ramp", "square"]).optional(),
      rateHz: range(0.01, 30).optional(),
      phase: range(0, 1).optional(),
      pitch: range(-12, 12).optional(),
      cutoff: range(-48, 48).optional(),
      positionA: range(-1, 1).optional(),
      positionB: range(-1, 1).optional(),
      level: range(0, 1).optional(),
    })
    .optional(),
  lfo2: z
    .strictObject({
      shape: z.enum(["sine", "triangle", "ramp", "square"]).optional(),
      rateHz: range(0.01, 30).optional(),
      phase: range(0, 1).optional(),
    })
    .optional(),
  modEnvelope: curvedEnvelopeSchema.optional(),
  fm: range(0, 1).optional(),
  ring: range(0, 1).optional(),
  macros: z.array(range(0, 1)).max(4).optional(),
  modulation: z.array(modulationRouteSchema).max(8).optional(),
  glide: range(0, 2).optional(),
  level: range(0, 2).optional(),
  pan: range(-1, 1).optional(),
});
export type WavetableOptions = z.infer<typeof wavetableOptionsSchema>;
export const samplerOptionsSchema = z.strictObject({
  rootKey: range(0, 127).int().optional(),
  velocitySensitivity: range(0, 1).optional(),
  amp: envelopeOptionsSchema.optional(),
  loop: z.enum(["off", "forward"]).optional(),
  startSeconds: range(0, 600).optional(),
  level: range(0, 2).optional(),
  pan: range(-1, 1).optional(),
});
export type SamplerOptions = z.infer<typeof samplerOptionsSchema>;

export const slicePositionSchema = z.union([
  z.strictObject({ frames: frameWireSchema }),
  z.strictObject({ beat: beatWireSchema }),
]);
export const sliceSpecSchema = z.strictObject({
  start: slicePositionSchema,
  end: slicePositionSchema.optional(),
  level: range(0, 2).optional(),
  pan: range(-1, 1).optional(),
  rate: range(0.25, 4).optional(),
  reverse: z.boolean().optional(),
});
export const slicerStateSchema = z.strictObject({
  sampleId: entityIdSchema,
  slices: z.union([
    z.array(sliceSpecSchema).min(1).max(64),
    z.strictObject({ grid: range(1, 64).int() }),
    z.strictObject({
      onset: z.strictObject({ algorithm: z.literal("onset-v1"), sensitivity: range(0, 1).optional() }),
    }),
  ]),
  triggerNote: range(0, 127).int().optional(),
  playMode: z.enum(["oneshot", "gate"]),
  tempoSync: z.enum(["off", "repitch"]).optional(),
});
export type SlicerState = z.infer<typeof slicerStateSchema>;
