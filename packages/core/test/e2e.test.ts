/**
 * End-to-end: `@oxitone/core` builders → facade → real native addon
 * (`.node`). Covers compile, renderWav (header/determinism/loudness),
 * mixer-channel stems, transport + setParameter error paths, realtime
 * simulated playback (latency breakdown, diagnostics), and the
 * exportMidi regression path.
 */
import { existsSync, mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { ErrorCode, OxitoneError } from "@oxitone/protocol";
import {
  compile as nativeCompile,
  createEngine,
  dispose as nativeDispose,
  enqueueTransport,
  getDiagnostics,
  getOutputLatency,
  listOutputDevices,
  setParameter,
} from "@oxitone/native";
import { createAutomationNamespace, Pattern, Project } from "../src/index.js";

function buildProject(): { project: Project; channelId: string } {
  const project = new Project({ seed: 42 });
  project.setTempo(120);
  const channel = project.addChannel({ name: "keys" }); // default wavetable instrument
  const track = project.addTrack("keys").use(channel);
  const pattern = new Pattern({
    lengthBeats: 4,
    notes: [
      { pitch: 60, start: 0, duration: 1, velocity: 0.9 },
      { pitch: 64, start: 1, duration: 1, velocity: 0.9 },
      { pitch: 67, start: 2, duration: 1, velocity: 0.9 },
      { pitch: 72, start: 3, duration: 1, velocity: 0.9 },
    ],
  });
  track.add(pattern).at({ bar: 1 }).loop(2); // 8 beats = 4 s at 120 BPM
  const automation = createAutomationNamespace();
  channel.automate("level", automation.sine({ periodBeats: 2, min: 0.4, max: 1 }));
  return { project, channelId: channel.id };
}

interface WavInfo {
  bytes: Buffer;
  format: number;
  channels: number;
  sampleRate: number;
  bitsPerSample: number;
  dataOffset: number;
  dataSize: number;
}

function parseWav(path: string): WavInfo {
  const bytes = readFileSync(path);
  expect(bytes.toString("ascii", 0, 4)).toBe("RIFF");
  expect(bytes.toString("ascii", 8, 12)).toBe("WAVE");
  let offset = 12;
  let fmt: Pick<WavInfo, "format" | "channels" | "sampleRate" | "bitsPerSample"> | undefined;
  let dataOffset = -1;
  let dataSize = -1;
  while (offset + 8 <= bytes.length) {
    const id = bytes.toString("ascii", offset, offset + 4);
    const size = bytes.readUInt32LE(offset + 4);
    if (id === "fmt ") {
      fmt = {
        format: bytes.readUInt16LE(offset + 8),
        channels: bytes.readUInt16LE(offset + 10),
        sampleRate: bytes.readUInt32LE(offset + 12),
        bitsPerSample: bytes.readUInt16LE(offset + 22),
      };
    }
    if (id === "data") {
      dataOffset = offset + 8;
      dataSize = size;
    }
    offset += 8 + size + (size % 2);
  }
  if (fmt === undefined || dataOffset < 0) {
    throw new Error(`malformed WAV: ${path}`);
  }
  return { bytes, ...fmt, dataOffset, dataSize };
}

function peakAndRms(wav: WavInfo): { peak: number; rms: number } {
  const frames = wav.dataSize / 4;
  let peak = 0;
  let sumSquares = 0;
  for (let i = 0; i < frames; i += 1) {
    const value = Math.abs(wav.bytes.readFloatLE(wav.dataOffset + i * 4));
    peak = Math.max(peak, value);
    sumSquares += value * value;
  }
  return { peak, rms: Math.sqrt(sumSquares / frames) };
}

function codeOf(fn: () => unknown): string {
  try {
    fn();
  } catch (error) {
    expect(error).toBeInstanceOf(OxitoneError);
    return (error as OxitoneError).code;
  }
  throw new Error("expected the call to throw");
}

describe("native e2e (real .node)", () => {
  it("compiles and renders a deterministic float32 stereo WAV", async () => {
    const dir = mkdtempSync(join(tmpdir(), "oxitone-e2e-"));
    const { project } = buildProject();

    const session = await project.compile();
    expect(session.engineId).toMatch(/^eng_/);

    const out1 = join(dir, "mix1.wav");
    const report = await session.renderWav({ path: out1 });
    expect(report.files).toHaveLength(1);
    const file = report.files[0]!;
    expect(file.path).toBe(out1);
    expect(file.durationSeconds).toBeGreaterThan(3.9);
    expect(file.durationSeconds).toBeLessThan(4.1);
    expect(Number.isFinite(file.peakDbfs)).toBe(true);
    expect(Number.isFinite(file.truePeakDbfs)).toBe(true);
    expect(Number.isFinite(file.integratedLufs)).toBe(true);
    expect(file.peakDbfs).toBeGreaterThan(-30);
    expect(typeof report.graphLatencyFrames).toBe("string");
    expect(BigInt(report.graphLatencyFrames)).toBeGreaterThanOrEqual(0n);

    const wav = parseWav(out1);
    expect(wav.format).toBe(3); // IEEE float
    expect(wav.channels).toBe(2);
    expect(wav.sampleRate).toBe(48_000);
    expect(wav.bitsPerSample).toBe(32);
    const { peak, rms } = peakAndRms(wav);
    expect(peak).toBeGreaterThan(0.05);
    expect(rms).toBeGreaterThan(0.005);

    // Determinism: a second render is byte-identical.
    const out2 = join(dir, "mix2.wav");
    await session.renderWav({ path: out2 });
    expect(readFileSync(out2).equals(readFileSync(out1))).toBe(true);

    await session.dispose();
  });

  it("exports Master once when the project has no other mixer buses", async () => {
    const dir = mkdtempSync(join(tmpdir(), "oxitone-e2e-stems-"));
    const { project } = buildProject();

    // One-shot project-level render (temporary engine).
    const report = await project.renderWav({ path: dir, stems: "mixer-channels" });
    expect(report.files).toHaveLength(1);
    expect(report.files[0]!.stem).toBeUndefined();
    expect(report.files[0]!.path).toBe(join(dir, "master.wav"));
    expect(report.files.some((file) => file.stem === project.masterMixerChannelId)).toBe(false);
    for (const file of report.files) {
      expect(existsSync(file.path)).toBe(true);
      const wav = parseWav(file.path);
      expect(wav.sampleRate).toBe(48_000);
      expect(wav.channels).toBe(2);
    }
  });

  it("keeps transport and setParameter error paths stable", async () => {
    const { project, channelId } = buildProject();
    const engine = createEngine({ audioBackend: "simulated", renderAheadBlocks: 16 });
    try {
      // Not compiled yet: stable codes for both commands.
      expect(codeOf(() => enqueueTransport(engine, { command: "play" }))).toBe(
        ErrorCode.InvalidProject,
      );
      expect(codeOf(() => setParameter(engine, channelId, "level", 0.5))).toBe(
        ErrorCode.InvalidProject,
      );

      nativeCompile(engine, project.snapshot());

      const playing = enqueueTransport(engine, { command: "play" });
      expect(playing.state).toBe("playing");
      const sought = enqueueTransport(engine, { command: "seek", beat: { numerator: 2, denominator: 1 } });
      expect(sought.cursor).toBe("48000"); // 2 beats at 120 BPM / 48 kHz
      expect(enqueueTransport(engine, { command: "pause" }).state).toBe("paused");
      expect(enqueueTransport(engine, { command: "stop" })).toEqual({
        state: "stopped",
        cursor: "0",
      });

      // Valid event, then target/range errors with stable codes.
      setParameter(engine, channelId, "level", 0.5);
      expect(codeOf(() => setParameter(engine, "chn_ghost", "level", 0.5))).toBe(
        ErrorCode.AutomationTargetInvalid,
      );
      expect(codeOf(() => setParameter(engine, channelId, "nonsense", 0.5))).toBe(
        ErrorCode.AutomationTargetInvalid,
      );
      expect(codeOf(() => setParameter(engine, channelId, "level", 3))).toBe(
        ErrorCode.AutomationRange,
      );
    } finally {
      nativeDispose(engine);
    }
  });

  it("reports simulated output latency and processes PCM without opening a system device", async () => {
    const devices = listOutputDevices();
    // Device enumeration is read-only and may be empty on a headless machine.
    expect(Array.isArray(devices)).toBe(true);

    const { project } = buildProject();
    const engine = createEngine({ audioBackend: "simulated", renderAheadBlocks: 16 });
    try {
      // Not playing yet: latency is unavailable with a stable code.
      expect(codeOf(() => getOutputLatency(engine))).toBe(ErrorCode.DeviceUnavailable);

      nativeCompile(engine, project.snapshot());
      enqueueTransport(engine, { command: "play" });

      const latency = getOutputLatency(engine);
      const breakdown = latency.breakdown;
      const parts = [
        breakdown.ring,
        breakdown.resampler,
        breakdown.deviceBuffer,
        breakdown.safetyOffset,
        breakdown.deviceLatency,
      ].reduce((sum, part) => sum + BigInt(part), 0n);
      expect(BigInt(latency.frames)).toBe(parts);
      expect(latency.seconds).toBeCloseTo(Number(latency.frames) / 48_000, 6);

      // The same renderer/worker runs against the simulated sink only.
      await new Promise((resolve) => setTimeout(resolve, 300));
      const diagnostics = getDiagnostics(engine);
      expect(diagnostics.blocks).toBeGreaterThan(0);
      expect(diagnostics.xruns).toBe(0);
      expect(diagnostics.engineLoad).toBeLessThan(0.7);
    } finally {
      nativeDispose(engine);
    }
  });

  it("exportMidi regression: file and base64 modes still work", async () => {
    const dir = mkdtempSync(join(tmpdir(), "oxitone-e2e-midi-"));
    const { project } = buildProject();

    const midiPath = join(dir, "export.mid");
    const fileReport = await project.exportMidi({ path: midiPath });
    expect(fileReport.path).toBe(midiPath);
    expect(fileReport.bytes).toBeGreaterThan(0);
    expect(existsSync(midiPath)).toBe(true);
    expect(readFileSync(midiPath).toString("ascii", 0, 4)).toBe("MThd");
    expect(fileReport.diagnostics.noteTrackCount).toBe(1);
    expect(fileReport.diagnostics.channelAssignments).toHaveLength(1);

    const session = await project.compile();
    const memoryReport = await session.exportMidi({});
    expect(typeof memoryReport.bytesBase64).toBe("string");
    expect(memoryReport.diagnostics.ppq).toBe(960);
    await session.dispose();
  });
});
