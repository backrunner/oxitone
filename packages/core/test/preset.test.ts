import { mkdtemp, readFile, rename, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { it, expect } from "vitest";
import { inspectSample } from "@oxitone/native";
import {
  Project,
  Pattern,
  wavetable,
  sampler,
  createChannelPreset,
  createPluginPreset,
  applyPreset,
  savePreset,
  loadPreset,
  validatePreset,
  presetEffect,
} from "../src/index.js";

function song() {
  const project = new Project({ seed: 42 });
  const channel = project.addChannel({ instrument: wavetable({ oscA: { wave: "triangle" } }), level: 0.2 });
  project
    .addTrack()
    .use(channel)
    .add(new Pattern({ lengthBeats: 1, notes: [{ pitch: 60, start: 0, duration: 0.5, velocity: 0.5 }] }))
    .at({ bar: 1 });
  return { project, channel };
}

it("round trips canonical channel settings and preserves authoring on invalid preset application", async () => {
  const dir = await mkdtemp(join(tmpdir(), "oxitone-preset-"));
  const { project, channel } = song();
  try {
    channel.effectChain = [{ pluginId: "oxitone.utility", pluginVersion: "1.0.0", parameters: { gainDb: -6 } }];
    const preset = createChannelPreset(channel, { name: "Soft keys" });
    const path = join(dir, "keys.oxitonepreset.json");
    await savePreset(preset, path);
    const bytes = await readFile(path);
    const loaded = await loadPreset(path);
    await savePreset(loaded.preset, path);
    expect(await readFile(path)).toEqual(bytes);
    const before = join(dir, "before.wav");
    await project.renderWav({ path: before });
    channel.level = 0;
    channel.instrument = wavetable({ oscA: { wave: "sine" } });
    await applyPreset(project, channel, loaded.preset);
    await project.renderWav({ path: join(dir, "after.wav") });
    expect(await readFile(join(dir, "after.wav"))).toEqual(await readFile(before));
    const snapshot = project.snapshot();
    preset.instrument.parameters.unknown = 1;
    await expect(applyPreset(project, channel, preset)).rejects.toMatchObject({ code: "InvalidProject" });
    await expect(savePreset(preset, path)).rejects.toMatchObject({ code: "InvalidProject" });
    expect(project.snapshot()).toEqual(snapshot);
    expect(await readFile(path)).toEqual(bytes);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

it("moves sample presets and remaps resource IDs without embedding audio or losing playback", async () => {
  const dir = await mkdtemp(join(tmpdir(), "oxitone-preset-assets-"));
  const moved = `${dir}-moved`;
  const { project, channel } = song();
  try {
    const source = join(dir, "source.wav");
    await project.renderWav({ path: source, tailSeconds: 0.1 });
    const info = inspectSample(source);
    const sample = project.addSample({ ...info, assetUri: source, frames: BigInt(info.frames) });
    channel.instrument = sampler(sample);
    const preset = createChannelPreset(channel, { samples: project.samples });
    await savePreset(preset, join(dir, "sample.oxitonepreset.json"));
    await rename(dir, moved);
    const loaded = await loadPreset(join(moved, "sample.oxitonepreset.json"));
    expect(loaded.preset.samples[0]!.assetUri).toBe("source.wav");
    const destination = song();
    await applyPreset(destination.project, destination.channel, loaded.preset, { assetBaseDir: loaded.assetBaseDir });
    expect(destination.channel.instrument.resources?.sample).toBe(destination.project.samples[0]!.id);
    const report = await destination.project.renderWav({ path: join(moved, "applied.wav") });
    expect(report.files[0]!.peakDbfs).toBeGreaterThan(-80);
    const restored = Project.fromSnapshot(destination.project.snapshot(), { assetBaseDir: moved });
    await applyPreset(restored, restored.channels[0]!, loaded.preset);
    await restored.renderWav({ path: join(moved, "reapplied.wav") });
    expect(await readFile(join(moved, "reapplied.wav"))).toEqual(await readFile(join(moved, "applied.wav")));
    const file = join(moved, "sample.oxitonepreset.json");
    const raw = JSON.parse(await readFile(file, "utf8"));
    raw.samples[0].assetUri = "../escape.wav";
    await writeFile(file, JSON.stringify(raw));
    await expect(loadPreset(file)).rejects.toMatchObject({ code: "InvalidProject" });
    raw.samples[0].assetUri = "source.wav";
    raw.samples[0].sha256 = "0".repeat(64);
    await writeFile(file, JSON.stringify(raw));
    await expect(loadPreset(file)).rejects.toMatchObject({ code: "AssetUnavailable" });
  } finally {
    await rm(dir, { recursive: true, force: true });
    await rm(moved, { recursive: true, force: true });
  }
});

it("validates effect, format/ABI, version, range and structured-state contracts", () => {
  const effect = createPluginPreset(
    { pluginId: "oxitone.delay", pluginVersion: "1.0.0", parameters: { feedback: 0.3 }, mix: 0.2 },
    "effect",
  );
  expect(presetEffect(validatePreset(effect)).mix).toBe(0.2);
  expect(() => validatePreset({ ...effect, abiMajor: 2 } as never)).toThrow();
  expect(() => validatePreset({ ...effect, formatVersion: "2.0" } as never)).toThrow();
  expect(() => validatePreset({ ...effect, pluginVersion: "99.0.0" } as never)).toThrow();
  expect(() => validatePreset({ ...effect, parameters: { feedback: 99 } } as never)).toThrow();
  expect(() =>
    validatePreset(createPluginPreset({ ...wavetable(), state: { invalid: true } }, "instrument")),
  ).toThrow();
});
