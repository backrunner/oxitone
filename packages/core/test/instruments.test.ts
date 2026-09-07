import { describe, expect, it } from "vitest";
import { ErrorCode, type SamplerOptions, type WavetableOptions } from "@oxitone/protocol";
import { Project, sampler, slicer, wavetable, type SlicerOptions } from "../src/index.js";

function sample() {
  return new Project().addSample({ assetUri: "loop.wav", sha256: "00".repeat(32),
    sampleRate: 48000, channels: 1, format: "wav", frames: 48000 });
}

describe("built-in instrument authoring", () => {
  it("maps named controls and leaves omitted native defaults intact", () => {
    expect(wavetable()).toEqual({ pluginId: "oxitone.wavetable", pluginVersion: "1.0.0", parameters: {} });
    expect(wavetable({ oscA: { wave: "triangle", unison: 3 }, oscB: { wave: "sine", pitch: 12 },
      filter: { type: "bandpass", cutoff: 1000 }, filterEnvelope: { attack: 0.2, amount: 12 },
      voiceMode: "legato", mix: 0.5 }).parameters).toEqual({
      "oscA.wavetable": 3, "oscA.unison": 3, "oscB.wavetable": 0, "oscB.pitch": 12,
      "filter.type": 2, "filter.cutoff": 1000, "filterEnv.attack": 0.2,
      "filterEnv.amount": 12, voiceMode: 2, "osc.mix": 0.5,
    });
    const asset = sample();
    expect(sampler(asset, { rootKey: 36, loop: "forward", startSeconds: 0.25 }).resources).toEqual({ sample: asset.id });
    expect(sampler(asset, { rootKey: 36, loop: "forward", startSeconds: 0.25 }).parameters)
      .toEqual({ rootKey: 36, loop: 1, start: 0.25 });
  });

  it("creates portable immutable slice state with precise frame and beat markers", () => {
    const asset = sample();
    const options: SlicerOptions = { slices: [{ start: { frames: 0n }, end: { beat: 0.25 }, reverse: true },
      { start: { frames: 0xffff_ffff_ffff_fffen } }], tempoSync: "repitch", level: 0.5 };
    const instrument = slicer(asset, options);
    expect(instrument.state).toEqual({ sampleId: asset.id, tempoSync: "repitch", playMode: "oneshot",
      slices: [{ start: { frames: "0" }, end: { beat: { numerator: 1, denominator: 4 } }, reverse: true },
        { start: { frames: "18446744073709551614" } }] });
    expect(instrument.parameters).toEqual({ level: 0.5 });
    options.tempoSync = "off";
    expect(instrument.state).toHaveProperty("tempoSync", "repitch");
    expect(slicer(asset, { slices: { onset: { algorithm: "onset-v1" } } }).state)
      .toHaveProperty("slices.onset.algorithm", "onset-v1");
  });

  it("rejects unknown controls, invalid values and unsafe frame numbers with stable errors", () => {
    const bad = expect.objectContaining({ code: ErrorCode.InvalidProject });
    for (const options of [{ filter: { cutoff: 0 } }, { oscA: { unison: 1.5 } }, { glide: Infinity },
      { voiceMode: "unknown" }, { oscA: { wavetable: 1 } }]) {
      expect(() => wavetable(options as WavetableOptions)).toThrowError(bad);
    }
    const asset = sample();
    for (const options of [{ rootKey: 128 }, { startSeconds: -1 }, { loop: "ping-pong" }]) {
      expect(() => sampler(asset, options as SamplerOptions)).toThrowError(bad);
    }
    for (const frames of [-1n, 1.25, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1, 0x1_0000_0000_0000_0000n]) {
      expect(() => slicer(asset, { slices: [{ start: { frames } }] })).toThrowError(bad);
    }
    for (const options of [{ slices: { grid: 65 } }, { slices: [] }, { slices: [null] },
      { slices: { grid: 2 }, tempoSync: "stretch" }, { slices: [{ start: { frames: 0, beat: 0 } }] }]) {
      expect(() => slicer(asset, options as SlicerOptions)).toThrowError(bad);
    }
  });

  it("replaces a channel instrument once and preserves snapshots on failed edits", () => {
    const project = new Project();
    const channel = project.addChannel();
    const revision = project.revisionBigInt;
    const instrument = wavetable({ oscA: { wave: "sine" } });
    channel.instrument = instrument;
    expect(project.revisionBigInt).toBe(revision + 1n);
    instrument.parameters["oscA.wavetable"] = 3;
    const before = project.snapshot();
    expect(channel.instrument.parameters["oscA.wavetable"]).toBe(0);
    expect(() => { channel.instrument = { ...instrument, parameters: { level: NaN } }; })
      .toThrowError(expect.objectContaining({ code: ErrorCode.InvalidProject }));
    expect(project.snapshot()).toEqual(before);
  });
});
