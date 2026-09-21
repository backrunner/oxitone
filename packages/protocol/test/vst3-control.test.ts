import { expect, it } from "vitest";
import { vst3ControlRequestSchema, vst3ControlResponseSchema } from "../src/index.js";

it("validates a frozen stream's bounded restart reasons without accepting empty or unknown changes", () => {
  const restart = { reasons: ["io", "latency", "parameters", "reload"], latencyFrames: 64, tailFrames: 0xffffffff };
  const reply = { controlProtocolVersion: 1, ok: true, state: { editorOpen: false, nextSequence: 17, restart } };
  expect(vst3ControlResponseSchema.parse(reply)).toEqual(reply);
  for (const invalid of [
    null,
    {},
    { ...restart, reasons: [] },
    { ...restart, reasons: ["io", "io"] },
    { ...restart, reasons: ["unknown"] },
    { ...restart, latencyFrames: -1 },
    { ...restart, tailFrames: 2 ** 32 },
    { ...restart, apply: true },
  ]) {
    expect(vst3ControlResponseSchema.safeParse({ ...reply, state: { ...reply.state, restart: invalid } }).success).toBe(
      false,
    );
  }
});

it("bounds live commands and excludes audio and persisted source mutations", () => {
  for (const command of [
    { kind: "poll" },
    { kind: "openEditor" },
    { kind: "closeEditor" },
    { kind: "capture" },
    { kind: "setParameter", parameterId: 0xffffffff, value: 0.5 },
  ]) {
    expect(vst3ControlRequestSchema.parse({ controlProtocolVersion: 1, command }).command).toEqual(command);
  }
  for (const command of [
    { kind: "poll", pcm: [] },
    { kind: "restore", state: {} },
    { kind: "setParameter", parameterId: -1, value: 0.5 },
    { kind: "setParameter", parameterId: 0, value: Number.NaN },
  ]) {
    expect(vst3ControlRequestSchema.safeParse({ controlProtocolVersion: 1, command }).success).toBe(false);
  }
  expect(vst3ControlRequestSchema.safeParse({ controlProtocolVersion: 2, command: { kind: "poll" } }).success).toBe(
    false,
  );
});
it("requires a bounded audio sequence and a valid captured configuration", () => {
  const reply = { controlProtocolVersion: 1, ok: true, state: { editorOpen: true, nextSequence: 17 } };
  expect(vst3ControlResponseSchema.parse(reply)).toEqual(reply);
  for (const state of [
    { editorOpen: true, nextSequence: -1 },
    { editorOpen: false, nextSequence: 2 ** 53 },
    { editorOpen: true, nextSequence: 1, info: {} },
  ]) {
    expect(vst3ControlResponseSchema.safeParse({ ...reply, state }).success).toBe(false);
  }
});
