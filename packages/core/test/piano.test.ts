import { expect, it } from "vitest";
import { Project, grandPiano, softPiano, type PianoBank } from "../src/index.js";
import { multisamplerStateSchema } from "@oxitone/protocol";

const bank = (): PianoBank => {
  const p = new Project();
  return {
    keyRange: [21, 108],
    layers: Array.from({ length: 4 }, (_, layer) =>
      [21, 48, 72, 108].map((rootKey) => ({
        rootKey,
        sample: p.addSample({
          assetUri: `${rootKey}-${layer}.wav`,
          format: "wav",
          sha256: "00".repeat(32),
          frames: 48000,
          sampleRate: 48000,
          channels: 2,
        }),
      })),
    ),
  };
};
it("maps all 88 keys and 127 velocities without gaps or overlaps and uses softer recorded strikes", () => {
  const source = bank();
  const before = source.layers.map((layer) => [...layer]);
  const grand = grandPiano(source),
    soft = softPiano(source);
  for (const instrument of [grand, soft]) {
    const { regions } = multisamplerStateSchema.parse(instrument.state);
    for (let key = 21; key <= 108; key++)
      for (let v = 1; v <= 127; v++) {
        expect(
          regions.filter(
            (r) => key >= r.keyRange[0] && key <= r.keyRange[1] && v >= r.velocityRange[0] && v <= r.velocityRange[1],
          ),
        ).toHaveLength(1);
      }
  }
  expect(Object.values(soft.resources!)).toEqual(source.layers.slice(0, 2).flatMap((l) => l.map((s) => s.sample.id)));
  expect(Object.keys(grand.resources!)).toHaveLength(16);
  expect(source.layers).toEqual(before);
  expect(soft.parameters["amp.attack"]).toBeGreaterThan(grand.parameters["amp.attack"]!);
});
it("rejects mismatched layers and preserves options validation", () => {
  const b = bank(),
    invalid = expect.objectContaining({ code: "InvalidProject" });
  for (const candidate of [
    { ...b, layers: [b.layers[0]!] },
    { ...b, keyRange: [108, 21] },
    { ...b, layers: [b.layers[0]!, b.layers[1]!.slice(1)] },
    { ...b, layers: [null, b.layers[0]] },
    { ...b, layers: [[null], [null]] },
    { ...b, layers: [[{ rootKey: 60 }], [{ rootKey: 60 }]] },
  ]) {
    expect(() => grandPiano(candidate as PianoBank)).toThrowError(invalid);
  }
  expect(() => softPiano(b, { level: NaN })).toThrowError(invalid);
  expect(() => grandPiano(b, null as never)).toThrowError(invalid);
});
