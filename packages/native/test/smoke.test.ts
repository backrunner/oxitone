import { describe, expect, it } from "vitest";
import { ErrorCode, OxitoneError, PROTOCOL_VERSION, type ProjectSnapshot } from "@oxitone/protocol";
import { compile, createEngine, dispose, enqueueTransport, getProtocolVersion } from "../src/index.js";

const minimalSnapshot: ProjectSnapshot = {
  protocolVersion: PROTOCOL_VERSION,
  revision: "1",
  id: "prj_smoke",
  sampleRate: 48_000,
  blockSize: 128,
  seed: 42,
  tempoMap: [{ startBeat: { numerator: 0, denominator: 1 }, bpm: 120 }],
  timeSignatureMap: [{ startBar: 1, numerator: 4, denominator: 4 }],
  markers: [],
  tracks: [],
  patterns: [],
  patternClips: [],
  sampleClips: [],
  samples: [],
  channels: [],
  mixerChannels: [],
  automation: [],
};

describe("native facade smoke test", () => {
  it("resolves seconds at the engine rate, preserves the cursor on compile, and rejects conflicting positions", () => {
    const engine = createEngine({ sampleRate: 24000 });
    try {
      compile(engine, minimalSnapshot);
      expect(enqueueTransport(engine, { command: "seek", seconds: 1.25 }).cursor).toBe("30000");
      compile(engine, { ...minimalSnapshot, revision: "2" });
      expect(enqueueTransport(engine, { command: "pause" }).cursor).toBe("30000");
      for (const extra of [{ frame: "1" }, { beat: { numerator: 1, denominator: 1 } }]) {
        expect(() => enqueueTransport(engine, { command: "seek", seconds: 1, ...extra })).toThrowError(
          expect.objectContaining({ code: ErrorCode.InvalidProject }),
        );
      }
      expect(() => enqueueTransport(engine, { command: "seek", seconds: 1e30 })).toThrowError(
        expect.objectContaining({ code: ErrorCode.InvalidProject }),
      );
      expect(enqueueTransport(engine, { command: "pause" }).cursor).toBe("30000");
    } finally {
      dispose(engine);
    }
  });
  it("reports the protocol version", () => {
    expect(getProtocolVersion()).toBe(PROTOCOL_VERSION);
  });

  it("creates an engine and echoes a compiled snapshot", () => {
    const engine = createEngine({ sampleRate: 48_000, blockSize: 128 });
    expect(engine.id).toMatch(/^eng_[0-9a-f]{16}$/);
    expect(engine.protocolVersion).toBe(PROTOCOL_VERSION);

    const echo = compile(engine, minimalSnapshot);
    expect(echo.id).toBe(minimalSnapshot.id);
    expect(echo.protocolVersion).toBe(PROTOCOL_VERSION);
    expect(echo.revision).toBe(minimalSnapshot.revision);

    dispose(engine);
  });

  it("creates an engine without options", () => {
    const engine = createEngine();
    expect(engine.id).toMatch(/^eng_/);
    dispose(engine);
    expect(() => compile(engine, minimalSnapshot)).toThrow(OxitoneError);
  });

  it("rejects a malformed snapshot with a stable code", () => {
    const engine = createEngine();
    try {
      compile(engine, JSON.stringify({ protocolVersion: PROTOCOL_VERSION }));
      expect.unreachable();
    } catch (error) {
      expect(error).toBeInstanceOf(OxitoneError);
      expect((error as OxitoneError).code).toBe(ErrorCode.InvalidProject);
    }
    dispose(engine);
  });

  it("rejects an unsupported protocol version with a stable code", () => {
    const engine = createEngine();
    try {
      compile(engine, JSON.stringify({ ...minimalSnapshot, protocolVersion: "2.0" }));
      expect.unreachable();
    } catch (error) {
      expect(error).toBeInstanceOf(OxitoneError);
      expect((error as OxitoneError).code).toBe(ErrorCode.ProtocolVersionUnsupported);
    }
    dispose(engine);
  });
});
