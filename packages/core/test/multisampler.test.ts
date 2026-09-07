import { expect, it } from "vitest";
import { ErrorCode, type MultisamplerOptions } from "@oxitone/protocol";
import { Project, multisampler, type SampleRegion } from "../src/index.js";

function fixture() {
  const project = new Project();
  const sample = project.addSample({ assetUri: "piano.wav", sha256: "00".repeat(32),
    sampleRate: 48000, channels: 1, format: "wav", frames: 48000 });
  const region: SampleRegion = { sample, rootKey: 60, keyRange: [48, 72], velocityRange: [1, 64] };
  return { project, region };
}
it("builds independent portable key/velocity state without mutating authoring data", () => {
  const { project, region } = fixture();
  const before = project.snapshot();
  const ref = multisampler([region, { ...region, velocityRange: [65, 127], gain: 0.7 }],
    { transpose: 12, amp: { release: 0.3 }, loop: "forward" });
  expect(ref.parameters).toEqual({ transpose: 12, "amp.release": 0.3, loop: 1 });
  expect(ref.resources).toEqual({ region_0: region.sample.id, region_1: region.sample.id });
  expect(project.snapshot()).toEqual(before);
  region.keyRange[0] = 0;
  expect(ref.state).toHaveProperty("regions.0.keyRange", [48, 72]);
  project.addChannel({ instrument: ref });
  expect(Project.fromSnapshot(project.snapshot()).snapshot()).toEqual(project.snapshot());
});
it("rejects overlap, invalid boundaries and unknown options with InvalidProject", () => {
  const { region } = fixture();
  const bad = expect.objectContaining({ code: ErrorCode.InvalidProject });
  for (const regions of [[], [region, region], [{ ...region, keyRange: [72, 48] }],
    [{ ...region, keyRange: [0, 128] }], [{ ...region, velocityRange: [0, 64] }],
    [{ ...region, velocityRange: [65, 64] }], [{ ...region, gain: NaN }]]) {
    expect(() => multisampler(regions as SampleRegion[])).toThrowError(bad);
  }
  for (const options of [{ rootKey: 60 }, { transpose: 49 }, { transpose: Infinity }]) {
    expect(() => multisampler([region], options as MultisamplerOptions)).toThrowError(bad);
  }
});
