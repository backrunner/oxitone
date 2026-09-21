import { expect, it } from "vitest";
import {
  vst3StreamManagerSchema,
  vst3StreamReadySchema,
  vst3StreamScheduleSchema,
  vst3StreamStartSchema,
} from "../src/index.js";

const start = {
  streamProtocolVersion: 11,
  source: { bundlePath: "/tmp/Test.vst3", classId: "1".repeat(32) },
  options: {},
};
it("requires an explicit helper scheduling result and complete bus metadata in stream v11", () => {
  const ready = {
    streamProtocolVersion: 11,
    sampleRate: 48000,
    blockSize: 128,
    classId: "1".repeat(32),
    sha256: "a".repeat(64),
    inputChannels: 2,
    outputChannels: 2,
    audioBuses: { inputs: [{ channels: 2, active: true }], outputs: [{ channels: 2, active: true }] },
    noteInput: false,
    noteOutput: false,
    category: "Fx",
    latencyFrames: 0,
    tailFrames: 0,
    parameters: [],
    helperTimeConstraint: false,
  };
  expect(vst3StreamReadySchema.parse(ready)).toEqual(ready);
  for (const invalid of [
    { streamProtocolVersion: 1 },
    { streamProtocolVersion: 2 },
    { streamProtocolVersion: 3 },
    { streamProtocolVersion: 4 },
    { streamProtocolVersion: 5 },
    { streamProtocolVersion: 6 },
    { streamProtocolVersion: 7 },
    { streamProtocolVersion: 8 },
    { streamProtocolVersion: 9 },
    { streamProtocolVersion: 10 },
    { streamProtocolVersion: 12 },
    { helperTimeConstraint: undefined },
    { helperTimeConstraint: "true" },
    { audioBuses: undefined },
    { audioBuses: { inputs: [], outputs: [] } },
    { audioBuses: { inputs: [], outputs: [{ channels: 6, active: true }] } },
    { audioBuses: { inputs: Array(17).fill({ channels: 2, active: true }), outputs: [{ channels: 2, active: true }] } },
  ])
    expect(vst3StreamReadySchema.safeParse({ ...ready, ...invalid }).success).toBe(false);
});

it("bounds explicit bus activation without accepting PCM or channel-layout mutation", () => {
  const busActivation = { inputs: [true, false, true], outputs: [true, true] };
  expect(vst3StreamStartSchema.parse({ ...start, options: { busActivation } }).options.busActivation).toEqual(
    busActivation,
  );
  for (const value of [
    { inputs: Array(17).fill(true), outputs: [true] },
    { inputs: [true], outputs: Array(17).fill(true) },
    { inputs: [1], outputs: [true] },
    { ...busActivation, channels: 6 },
  ])
    expect(vst3StreamStartSchema.safeParse({ ...start, options: { busActivation: value } }).success).toBe(false);
});
it("bounds native lifetime capacity and requires a fixed audio format", () => {
  const config = { managerVersion: 1, sampleRate: 48000, blockSize: 128, capacity: 2 };
  expect(vst3StreamManagerSchema.parse(config)).toEqual(config);
  for (const invalid of [
    { managerVersion: 2 },
    { sampleRate: 0 },
    { sampleRate: 384000 },
    { blockSize: 0 },
    { blockSize: 4097 },
    { capacity: 1 },
    { capacity: 17 },
    { capacity: 2.5 },
    { sampleRate: undefined },
    { autoRestart: true },
  ])
    expect(vst3StreamManagerSchema.safeParse({ ...config, ...invalid }).success).toBe(false);
});
it("defines only control initialization and applies the same offline configuration defaults", () => {
  const parsed = vst3StreamStartSchema.parse(start);
  expect(parsed.source.allowPlugins).toBe("signed-only");
  expect(parsed.options).toEqual({
    sampleRate: 48000,
    blockSize: 128,
    parameters: {},
    tempo: 120,
    timeSignature: [4, 4],
    processingMode: "realtime",
    midiOutput: false,
  });
  for (const input of [
    { ...start, streamProtocolVersion: 1 },
    { ...start, streamProtocolVersion: 2 },
    { ...start, streamProtocolVersion: 3 },
    { ...start, streamProtocolVersion: 4 },
    { ...start, streamProtocolVersion: 5 },
    { ...start, streamProtocolVersion: 6 },
    { ...start, streamProtocolVersion: 7 },
    { ...start, streamProtocolVersion: 8 },
    { ...start, streamProtocolVersion: 9 },
    { ...start, streamProtocolVersion: 12 },
    { ...start, options: { blockSize: 0 } },
    { ...start, options: { sampleRate: 384000 } },
    { ...start, options: { parameters: { "4294967296": 1 } } },
    { ...start, options: { parameters: { "0": Number.NaN } } },
    { ...start, options: { timeSignature: [4, 3] } },
    { ...start, options: { frames: 128 } },
    { ...start, pcm: [0, 0] },
  ])
    expect(vst3StreamStartSchema.safeParse(input).success).toBe(false);
});

it("requires an explicit epoch and fixed native scheduling latency", () => {
  const schedule = { scheduleVersion: 1, epoch: 0, latencyBlocks: 2 };
  expect(vst3StreamScheduleSchema.parse(schedule)).toEqual(schedule);
  expect(vst3StreamScheduleSchema.parse({ ...schedule, epoch: Number.MAX_SAFE_INTEGER, latencyBlocks: 16 })).toEqual({
    ...schedule,
    epoch: Number.MAX_SAFE_INTEGER,
    latencyBlocks: 16,
  });
  for (const input of [
    { ...schedule, scheduleVersion: 2 },
    { ...schedule, epoch: -1 },
    { ...schedule, epoch: Number.MAX_SAFE_INTEGER + 1 },
    { ...schedule, latencyBlocks: 1 },
    { ...schedule, latencyBlocks: 17 },
    { ...schedule, latencyBlocks: 2.5 },
    { ...schedule, seekFrame: 128 },
  ])
    expect(vst3StreamScheduleSchema.safeParse(input).success).toBe(false);
});

it("validates a fresh session's explicit project position before loading a helper", () => {
  const transport = {
    projectFrame: 96000,
    continuousFrame: 256000,
    projectBeat: 5.5,
    barBeat: 3.5,
    tempo: 137,
    timeSignature: [7, 8],
    playing: true,
    cycle: [3.5, 10.5],
  };
  expect(vst3StreamStartSchema.parse({ ...start, options: { transport } }).options.transport).toEqual(transport);
  for (const invalid of [{ projectFrame: -1 }, { tempo: 0 }, { cycle: [4, 4] }, { reset: true }]) {
    expect(
      vst3StreamStartSchema.safeParse({ ...start, options: { transport: { ...transport, ...invalid } } }).success,
    ).toBe(false);
  }
});
