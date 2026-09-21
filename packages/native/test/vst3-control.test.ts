import { expect, it } from "vitest";
import { PROTOCOL_VERSION, type ProjectSnapshot } from "@oxitone/protocol";
import {
  compile,
  controlVst3Instance,
  createEngine,
  dispose,
  enqueueTransport,
  getVst3Instances,
} from "../src/index.js";
import { loadNativeBinding } from "../src/load.js";
import * as browser from "../src/browser.js";

const snapshot: ProjectSnapshot = {
  protocolVersion: PROTOCOL_VERSION,
  revision: "1",
  id: "prj_control",
  sampleRate: 48000,
  blockSize: 128,
  seed: 1,
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
const target = {
  instanceControlVersion: 1 as const,
  graphGeneration: "1",
  instanceId: "missing",
  command: { kind: "poll" as const },
};

it("publishes fresh graph generations, preserves failed compilation and rejects stale async commands", async () => {
  const engine = createEngine({ audioBackend: "simulated" });
  try {
    expect(() => getVst3Instances(engine)).toThrow();
    compile(engine, snapshot);
    const first = getVst3Instances(engine);
    expect(first).toMatchObject({ instanceControlVersion: 1, state: "active", instances: [] });
    const request = { ...target, graphGeneration: first.graphGeneration };
    await expect(controlVst3Instance(engine, request)).rejects.toMatchObject({ code: "PluginConfigInvalid" });
    expect(() => compile(engine, { ...snapshot, protocolVersion: "99.0" })).toThrow();
    expect(getVst3Instances(engine)).toEqual(first);
    // Identical source revision still creates an independent native lifetime.
    compile(engine, snapshot);
    expect(getVst3Instances(engine).graphGeneration).not.toBe(first.graphGeneration);
    await expect(controlVst3Instance(engine, request)).rejects.toMatchObject({ code: "PluginTaskConflict" });
    enqueueTransport(engine, { command: "play" });
    compile(engine, snapshot);
    await expect.poll(() => getVst3Instances(engine).state).toBe("active");
    enqueueTransport(engine, { command: "pause" });
    compile(engine, snapshot);
    await expect.poll(() => getVst3Instances(engine).state).toBe("active");
  } finally {
    dispose(engine);
  }
});

it("validates raw N-API requests independently of the TS facade and returns promises", async () => {
  const binding = loadNativeBinding();
  for (const [change, code] of [
    [{ instanceControlVersion: 2 }, "ProtocolVersionUnsupported"],
    [{ graphGeneration: "01" }, "PluginConfigInvalid"],
    [{ graphGeneration: "18446744073709551616" }, "PluginConfigInvalid"],
    [{ instanceId: "insert.0" }, "PluginConfigInvalid"],
    [{ timeoutMs: 0 }, "PluginConfigInvalid"],
    [{ extra: true }, "PluginConfigInvalid"],
  ] as const) {
    const pending = binding.controlVst3Instance("missing", JSON.stringify({ ...target, ...change }));
    expect(pending).toBeInstanceOf(Promise);
    await expect(
      pending.catch((error: Error) => {
        throw JSON.parse(error.message);
      }),
    ).rejects.toMatchObject({ code });
  }
});

it("fails browser controls with a stable capability error", async () => {
  const engine = { id: "missing", protocolVersion: PROTOCOL_VERSION };
  expect(() => browser.getVst3Instances(engine)).toThrowError(
    expect.objectContaining({ code: "PluginCapabilityUnsupported" }),
  );
  await expect(browser.controlVst3Instance(engine, target)).rejects.toMatchObject({
    code: "PluginCapabilityUnsupported",
  });
});
