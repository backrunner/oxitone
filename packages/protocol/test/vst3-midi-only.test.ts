import { expect, it } from "vitest";
import { vst3InfoSchema, vst3StreamReadySchema, vst3StreamStartSchema } from "../src/index.js";

const info = {
  protocolVersion: 1,
  classId: "1".repeat(32),
  sha256: "a".repeat(64),
  name: "MIDI Generator",
  vendor: "Fixture",
  version: "1",
  category: "Fx|Tools",
  inputChannels: 0,
  outputChannels: 0,
  audioBuses: { inputs: [], outputs: [] },
  noteInput: false,
  noteOutput: true,
  parameters: [],
  configuration: null,
};

it("preserves real zero audio buses for MIDI-only inspection and stream handshakes", () => {
  expect(vst3InfoSchema.parse(info)).toEqual(info);
  const { protocolVersion: _, name: _n, vendor: _v, version: _version, configuration: _c, ...ports } = info;
  const ready = {
    ...ports,
    streamProtocolVersion: 11,
    sampleRate: 48000,
    blockSize: 128,
    latencyFrames: 0,
    tailFrames: 0,
    helperTimeConstraint: false,
  };
  expect(vst3StreamReadySchema.parse(ready)).toEqual(ready);
  for (const change of [
    { noteOutput: false },
    { outputChannels: 2 },
    { inputChannels: 2, audioBuses: { inputs: [{ channels: 2, active: true }], outputs: [] } },
  ]) {
    expect(vst3InfoSchema.safeParse({ ...info, ...change }).success).toBe(false);
    expect(vst3StreamReadySchema.safeParse({ ...ready, ...change }).success).toBe(false);
  }
  const start = {
    streamProtocolVersion: 11,
    source: { bundlePath: "/tmp/Midi.vst3", classId: info.classId },
    options: { midiOutput: true, busActivation: { inputs: [], outputs: [] } },
  };
  expect(vst3StreamStartSchema.parse(start).options.busActivation).toEqual({ inputs: [], outputs: [] });
  expect(vst3StreamStartSchema.safeParse({ ...start, streamProtocolVersion: 9 }).success).toBe(false);
});
