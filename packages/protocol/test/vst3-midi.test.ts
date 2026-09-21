import { expect, it } from "vitest";
import { vst3EventSchema, vst3StreamStartSchema } from "../src/index.js";

it("uses canonical channel-voice bytes and rejects system messages and malformed padding", () => {
  for (const message of [
    [0x80, 60, 10],
    [0x9f, 127, 127],
    [0xa2, 61, 99],
    [0xb0, 7, 64],
    [0xc3, 14, 0],
    [0xd5, 22, 0],
    [0xe7, 1, 64],
  ]) {
    const event = { type: "midi", frame: 13, message };
    expect(vst3EventSchema.parse(event)).toEqual(event);
  }
  for (const message of [
    [0x7f, 0, 0],
    [0xf0, 0, 0],
    [0x90, 128, 0],
    [0x90, 0, 128],
    [0xc0, 1, 1],
    [0xd0, 1, 1],
    [0xc0, 1],
  ])
    expect(vst3EventSchema.safeParse({ type: "midi", frame: 0, message }).success).toBe(false);
});
it("requires explicit MIDI capture and a current stream protocol", () => {
  const start = {
    streamProtocolVersion: 11,
    source: { bundlePath: "/tmp/Test.vst3", classId: "1".repeat(32) },
    options: {},
  };
  expect(vst3StreamStartSchema.parse(start).options.midiOutput).toBe(false);
  expect(vst3StreamStartSchema.parse({ ...start, options: { midiOutput: true } }).options.midiOutput).toBe(true);
  expect(vst3StreamStartSchema.safeParse({ ...start, options: { midiOutput: 1 } }).success).toBe(false);
});
