import {
  automationSourceSchema,
  beatToWire,
  canonicalEncode,
  hash64,
  hash64Input,
  Pcg32,
  PCG32_INCREMENT,
  PCG32_MULTIPLIER,
} from "../index.js";
import type { AutomationSourceSpec, Hash64Part } from "../index.js";
import { write } from "./output.js";

const automationFixtures: Record<string, AutomationSourceSpec> = {
  gate: {
    kind: "gate",
    periodBeats: beatToWire(0.5),
    duty: 0.5,
    phase: beatToWire(0),
    on: 1,
    off: 0,
  },
  wave: { kind: "wave", wave: "sine", periodBeats: beatToWire(8), min: 0.2, max: 0.9 },
  chance: {
    kind: "chance",
    rate: 2,
    probability: 0.72,
    seed: 17,
    smoothBeats: beatToWire(0.04),
    randomPhase: "absolute",
  },
  curve: {
    kind: "curve",
    interpolation: "smooth",
    points: [
      { beat: beatToWire(0), value: 0, curve: { kind: "bezier", out: [0.3, 0], in: [0.7, 1] } },
      { beat: beatToWire(2), value: 1 },
    ],
  },
  nested: {
    kind: "binary",
    op: "mix",
    amount: 0.25,
    left: {
      kind: "map",
      min: 0.2,
      max: 0.9,
      input: { kind: "unary", op: "invert", input: { kind: "constant", value: 0.8 } },
    },
    right: { kind: "wave", wave: "saw", periodBeats: beatToWire(4) },
  },
};

export function generateAutomationFixtures(): void {
  for (const [name, source] of Object.entries(automationFixtures)) {
    write(`schemas/fixtures/automation/${name}.canonical.json`, canonicalEncode(automationSourceSchema.parse(source)));
  }
}

/** Golden pcg32-v1 vectors; the Rust implementation must match bit for bit. */
export function generatePcg32Vectors(): void {
  const vectors = [0n, 42n].map((seed) => {
    const rng = new Pcg32(seed);
    const outputs = Array.from({ length: 32 }, () => String(rng.nextU32()));
    const floats = Array.from({ length: 8 }, () => String(rng.nextFloat()));
    return { seed: seed.toString(10), outputs, floats };
  });
  write(
    "schemas/fixtures/pcg32-v1.json",
    canonicalEncode({
      algorithm: "pcg32-v1",
      multiplier: PCG32_MULTIPLIER.toString(10),
      increment: PCG32_INCREMENT.toString(10),
      seeding: "state=0; advance; state+=seed; advance (fixed increment)",
      floatConversion: "nextU32 * 2^-32",
      vectors,
    }),
  );
}

/** Golden hash64-v1 vectors. */
export function generateHash64Vectors(): void {
  const partSets: Hash64Part[][] = [
    [99, 17, "auto_0003"],
    [99, 17, "auto_0003", 3],
    [0],
    ["chance"],
    [0xffff_ffff_ffff_ffffn, ""],
  ];
  write(
    "schemas/fixtures/hash64.json",
    canonicalEncode({
      algorithm: "hash64-v1",
      encoding: "parts as n:<u64 decimal> or s:<utf8>, joined by |; SHA-256; first 8 bytes big-endian",
      vectors: partSets.map((parts) => ({
        input: hash64Input(...parts),
        hash: hash64(...parts).toString(10),
      })),
    }),
  );
}
