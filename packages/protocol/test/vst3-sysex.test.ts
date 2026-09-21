import { expect, it } from "vitest";
import { vst3EventSchema } from "../src/index.js";

it("accepts complete bounded SysEx without truncating or accepting embedded status bytes", () => {
  for (const data of [
    [0xf0, 0xf7],
    [0xf0, 0x7d, 0, 127, 0xf7],
    [0xf0, ...Array(4094).fill(1), 0xf7],
  ]) {
    const event = { type: "sysEx", frame: 127, data };
    expect(vst3EventSchema.parse(event)).toEqual(event);
  }
  for (const data of [
    [],
    [0xf0],
    [0, 0xf7],
    [0xf0, 0],
    [0xf0, 0x80, 0xf7],
    [0xf0, 0xf8, 0xf7],
    [0xf0, -1, 0xf7],
    [0xf0, 1.5, 0xf7],
    [0xf0, ...Array(4095).fill(1), 0xf7],
  ]) {
    expect(vst3EventSchema.safeParse({ type: "sysEx", frame: 0, data }).success).toBe(false);
  }
  expect(vst3EventSchema.safeParse({ type: "sysEx", frame: 0, data: { offset: 0, length: 3 } }).success).toBe(false);
});
