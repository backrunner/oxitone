import { expect, it } from "vitest";
import { vst3ControlCommandSchema, vst3EditPageSchema } from "../src/index.js";

const position = {
  audioSequence: 4,
  reset: true,
  transport: {
    projectFrame: 128,
    continuousFrame: 512,
    projectBeat: 0.5,
    barBeat: 0,
    tempo: 120,
    timeSignature: [4, 4],
    playing: true,
    cycle: [0, 4],
  },
};
const page = {
  captureId: "2",
  status: "stopped",
  firstSequence: 0,
  nextSequence: 3,
  pendingEvents: 0,
  endPosition: position,
  events: [
    { sequence: 0, parameterId: 7, kind: "begin", position },
    { sequence: 1, parameterId: 7, kind: "value", value: 0.5, position },
    { sequence: 2, parameterId: 7, kind: "end", position },
  ],
};
it("preserves ordered brackets and explicit native positions with strict cursor and lifecycle validation", () => {
  expect(vst3EditPageSchema.parse(page)).toEqual(page);
  for (const bad of [
    { ...page, nextSequence: 4 },
    { ...page, pendingEvents: 1 },
    { ...page, endPosition: undefined },
    { ...page, status: "failed" },
    { ...page, firstSequence: 1 },
    { ...page, events: [{ ...page.events[0], value: 0.2 }, ...page.events.slice(1)] },
    { ...page, events: page.events.map((event) => ({ ...event, position: { ...position, audioSequence: 5 } })) },
  ])
    expect(vst3EditPageSchema.safeParse(bad).success).toBe(false);
  expect(
    vst3EditPageSchema.safeParse({
      captureId: "2",
      status: "failed",
      firstSequence: 0,
      nextSequence: 3,
      pendingEvents: 0,
      events: [],
      error: { code: "BudgetExceeded", message: "lost feedback" },
    }).success,
  ).toBe(true);
});
it("rejects malformed capture ids and unknown cursor fields without throwing from safeParse", () => {
  for (const captureId of ["", "0", "01", "-1", "abc", "18446744073709551616"]) {
    expect(vst3ControlCommandSchema.safeParse({ kind: "readEdits", captureId, fromSequence: 0 }).success).toBe(false);
  }
  expect(vst3ControlCommandSchema.parse({ kind: "startEdits" })).toEqual({ kind: "startEdits" });
  expect(vst3ControlCommandSchema.safeParse({ kind: "readEdits", captureId: "1", fromSequence: -1 }).success).toBe(
    false,
  );
  expect(
    vst3ControlCommandSchema.safeParse({ kind: "stopEdits", captureId: "1", fromSequence: 0, force: true }).success,
  ).toBe(false);
});
it("requires a bounded sorted recording selection and host-applied sample spans before the stop boundary", () => {
  const selection = { kind: "startRecording", mode: "touch", parameterIds: [0, 7] };
  expect(vst3ControlCommandSchema.parse(selection)).toEqual(selection);
  for (const override of [
    { parameterIds: [] },
    { parameterIds: [7, 0] },
    { parameterIds: [0, 0] },
    { parameterIds: Array.from({ length: 33 }, (_, i) => i) },
    { mode: "latch" },
  ])
    expect(vst3ControlCommandSchema.safeParse({ ...selection, ...override }).success).toBe(false);
  const recorded = {
    ...page,
    nextSequence: 1,
    recording: { mode: "touch", parameterIds: [7], sampleRate: 48000 },
    events: [
      {
        kind: "sample",
        parameterId: 7,
        sequence: 0,
        value: 0.5,
        frames: 128,
        position: { ...position, audioSequence: 3 },
      },
    ],
  };
  expect(vst3EditPageSchema.parse(recorded)).toEqual(recorded);
  for (const bad of [
    { ...recorded, recording: undefined },
    { ...recorded, recording: null },
    ...[{ frames: 0 }, { frames: 4097 }, { value: NaN }, { value: 1.1 }, { parameterId: 99 }, { position }].map(
      (override) => ({ ...recorded, events: [{ ...recorded.events[0], ...override }] }),
    ),
  ])
    expect(vst3EditPageSchema.safeParse(bad).success).toBe(false);
});
