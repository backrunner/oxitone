import { describe, expect, it } from "vitest";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { ErrorCode } from "@oxitone/protocol";
import { inspectSample } from "@oxitone/native";
import { Pattern, Project, multisampler, sampler, slicer, wavetable } from "../src/index.js";

describe("built-in instruments through native rendering", () => {
  it("renders all helper options and restores slice resources through project files", async () => {
    const root = await mkdtemp(join(tmpdir(), "oxitone-instruments-"));
    try {
      const project = new Project();
      const channel = project.addChannel({
        instrument: wavetable({
          oscA: {
            wave: "sine",
            morphTo: "organ",
            position: 0.35,
            phase: 0.2,
            phaseSpread: 0.6,
            pitch: 0,
            octave: 1,
            unison: 2,
            detune: 8,
            spread: 0.5,
            bank: "analog",
            warpMode: "bend",
            warp: 0.3,
          },
          oscB: {
            wave: "glass",
            morphTo: "triangle",
            position: 0.6,
            phase: 0.1,
            phaseSpread: 0.4,
            pitch: 0,
            octave: -1,
            unison: 1,
            detune: 0,
            spread: 0,
          },
          mix: 0.2,
          sub: { wave: "rounded", level: 0.1, octave: -2 },
          noise: { level: 0.02 },
          fm: 0.1,
          macros: [0.2],
          modulation: [{ source: "macro1", target: "warpA", amount: 0.5 }],
          lfo: {
            shape: "triangle",
            rateHz: 4,
            phase: 0.25,
            pitch: 0.1,
            cutoff: 12,
            positionA: 0.2,
            positionB: -0.2,
            level: 0.2,
          },
          filter: { type: "lowpass", cutoff: 4000, resonance: 0.2 },
          filterEnvelope: { amount: 12, attack: 0.01, decay: 0.1, sustain: 0.5, release: 0.1 },
          amp: { attack: 0.01, decay: 0.1, sustain: 0.5, release: 0.1 },
          voiceMode: "poly",
          glide: 0,
          level: 0.5,
          pan: 0,
        }),
      });
      project
        .addTrack()
        .use(channel)
        .add(new Pattern({ lengthBeats: 1, notes: [{ pitch: 60, start: 0, duration: 1, velocity: 1 }] }))
        .at({ bar: 1 });
      const source = join(root, "source.wav");
      const options = { end: { seconds: 0.25 }, tailSeconds: 0 };
      expect((await project.renderWav({ ...options, path: source })).files[0]!.peakDbfs).toBeGreaterThan(-60);
      const info = inspectSample(source);
      const sample = project.addSample({
        ...info,
        assetUri: source,
        frames: BigInt(info.frames),
        musicalLengthBeats: 0.5,
      });
      channel.instrument = sampler(sample, {
        rootKey: 60,
        loop: "forward",
        startSeconds: 0,
        velocitySensitivity: 0.5,
        amp: { attack: 0, decay: 0, sustain: 1, release: 0.1 },
        level: 0.5,
        pan: 0,
      });
      const output = join(root, "out.wav");
      expect((await project.renderWav({ ...options, path: output })).files[0]!.peakDbfs).toBeGreaterThan(-60);
      channel.instrument = multisampler(
        [
          { sample, rootKey: 60, keyRange: [0, 127], velocityRange: [1, 64] },
          { sample, rootKey: 48, keyRange: [0, 127], velocityRange: [65, 127], gain: 0.6 },
        ],
        { transpose: -12, loop: "forward", amp: { attack: 0, release: 0.1 } },
      );
      expect((await project.renderWav({ ...options, path: output })).files[0]!.peakDbfs).toBeGreaterThan(-60);
      const bankAudio = await readFile(output);
      const bankPath = join(root, "bank");
      await project.save(bankPath);
      const bank = await Project.load(bankPath);
      await bank.renderWav({ ...options, path: output });
      expect(await readFile(output)).toEqual(bankAudio);
      const ref = bank.channels[0]!.instrument;
      bank.channels[0]!.instrument = { ...ref, resources: { ...ref.resources, region_0: "smp_missing" } };
      await expect(bank.compile()).rejects.toMatchObject({ code: ErrorCode.InvalidProject });
      channel.instrument = slicer(sample, {
        slices: [{ start: { frames: 0n }, end: { beat: 0.5 }, level: 0.8, pan: 0, rate: 1, reverse: true }],
        triggerNote: 60,
        playMode: "oneshot",
        tempoSync: "repitch",
      });
      expect((await project.renderWav({ ...options, path: output })).files[0]!.peakDbfs).toBeGreaterThan(-60);
      const audio = await readFile(output);
      const saved = join(root, "saved");
      await project.save(saved);
      await rm(source);
      const loaded = await Project.load(saved);
      await loaded.renderWav({ ...options, path: output });
      expect(await readFile(output)).toEqual(audio);
      const restored = loaded.channels[0]!;
      restored.instrument = {
        ...restored.instrument,
        state: { ...(restored.instrument.state as object), tempoSync: "stretch" },
      };
      await expect(loaded.compile()).rejects.toMatchObject({ code: ErrorCode.InvalidProject });
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
});
