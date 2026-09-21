import { expect, it } from "vitest";
import { vst3TransportSchema } from "../src/index.js";

const position = {
  projectFrame: 96000,
  continuousFrame: 256000,
  projectBeat: 5.5,
  barBeat: 3.5,
  tempo: 137,
  timeSignature: [7, 8],
  playing: true,
  cycle: [3.5, 10.5],
};
it("keeps independent project and continuous positions without inferring beats from tempo", () => {
  expect(vst3TransportSchema.parse(position)).toEqual(position);
  expect(vst3TransportSchema.parse({ ...position, playing: false, cycle: undefined }).playing).toBe(false);
});
it("rejects invalid transport values and undeclared context fields", () => {
  for (const invalid of [
    { projectFrame: -1 },
    { projectFrame: Number.MAX_SAFE_INTEGER },
    { continuousFrame: 1.5 },
    { projectBeat: NaN },
    { barBeat: 6 },
    { barBeat: -1 },
    { tempo: 0 },
    { tempo: Infinity },
    { timeSignature: [0, 4] },
    { timeSignature: [4, 3] },
    { playing: 1 },
    { cycle: [4, 4] },
    { cycle: [-1, 4] },
    { cycle: [0, Infinity] },
    { implicitReset: true },
  ])
    expect(vst3TransportSchema.safeParse({ ...position, ...invalid }).success).toBe(false);
});
