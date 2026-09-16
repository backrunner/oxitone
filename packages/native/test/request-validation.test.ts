import { beforeEach, expect, it, vi } from "vitest";
import { ErrorCode, OxitoneError, type ProjectSnapshot } from "@oxitone/protocol";
import { call } from "../src/call.js";
import { compile, createEngine, enqueueTransport, exportMidi, renderWav, setParameter } from "../src/index.js";

vi.mock("../src/call.js", () => ({
  call: vi.fn(() => {
    throw new Error("Invalid requests must not reach native code");
  }),
  native: vi.fn(),
}));
beforeEach(() => {
  vi.mocked(call).mockClear();
});
const engine = { id: "eng_test", protocolVersion: "1.2" };
const snapshot = {} as ProjectSnapshot;

it.each([
  () => createEngine({ sampleRate: -1 }),
  () => compile(engine, "{}", { assetBaseDir: "" }),
  () => compile(engine, snapshot),
  () => renderWav(engine, snapshot, { path: "unused.wav" }),
  () => exportMidi(engine, snapshot, { ppq: 0 }),
  () => renderWav(engine, snapshot, { path: "" }),
  () => enqueueTransport(engine, { command: "seek", seconds: -1 }),
  () => setParameter(engine, "", "level", 1),
  () => setParameter(engine, "chn_test", "level", 1, Number.MAX_SAFE_INTEGER + 1),
  () => setParameter(engine, "chn_test", "level", 1, -1n),
])("uses a stable InvalidProject error before calling native code (%#)", (run) => {
  expect(run).toThrow(OxitoneError);
  expect(run).toThrowError(expect.objectContaining({ code: ErrorCode.InvalidProject }));
  expect(call).not.toHaveBeenCalled();
});

it.each([
  { bar: 2, seconds: 1 },
  { beat: { numerator: 4, denominator: 1 }, marker: "mkr_intro" },
  { seconds: 1, frames: "48000" },
])("rejects conflicting render coordinates instead of choosing one: %j", (start) => {
  expect(() => renderWav(engine, snapshot, { path: "unused.wav", start })).toThrowError(
    expect.objectContaining({ code: ErrorCode.InvalidProject }),
  );
  expect(call).not.toHaveBeenCalled();
});
