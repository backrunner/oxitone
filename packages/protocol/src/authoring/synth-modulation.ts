import { z } from "zod";

export const modulationSources = [
  "off",
  "lfo1",
  "lfo2",
  "ampEnv",
  "filterEnv",
  "modEnv",
  "velocity",
  "keytrack",
  "noteRandom",
  "macro1",
  "macro2",
  "macro3",
  "macro4",
] as const;
export const modulationTargets = [
  "pitch",
  "cutoff",
  "positionA",
  "positionB",
  "warpA",
  "warpB",
  "fm",
  "ring",
  "mix",
  "pan",
  "level",
] as const;
export const modulationRouteSchema = z.strictObject({
  source: z.enum(modulationSources),
  target: z.enum(modulationTargets),
  amount: z.number().finite().min(-1).max(1),
  curve: z.number().finite().min(-1).max(1).optional(),
});
export const curvedEnvelopeSchema = z.strictObject({
  attack: z.number().finite().min(0).max(8).optional(),
  decay: z.number().finite().min(0).max(8).optional(),
  sustain: z.number().finite().min(0).max(1).optional(),
  release: z.number().finite().min(0).max(8).optional(),
  attackCurve: z.number().finite().min(-1).max(1).optional(),
  decayCurve: z.number().finite().min(-1).max(1).optional(),
  releaseCurve: z.number().finite().min(-1).max(1).optional(),
});
