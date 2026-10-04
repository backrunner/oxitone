import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, expect, it } from "vitest";
import { ProjectDocument } from "../src/source/index.js";
import { cacheSample } from "@oxitone/native";
const cleanup: (() => Promise<unknown>)[] = [];
afterEach(async () => {
  for (const action of cleanup.splice(0).reverse()) await action();
});
async function fixture() {
  const root = await mkdtemp(join(tmpdir(), "oxitone-sample-drop-"));
  cleanup.push(() => rm(root, { recursive: true, force: true }));
  await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
  await symlink(fileURLToPath(new URL("../../core", import.meta.url)), join(root, "node_modules/@oxitone/core"), "dir");
  const entry = join(root, "song.ts"),
    path = join(root, "kick.wav");
  const wav = Buffer.alloc(44 + 9600);
  wav.write("RIFF");
  wav.writeUInt32LE(wav.length - 8, 4);
  wav.write("WAVEfmt ", 8);
  wav.writeUInt32LE(16, 16);
  wav.writeUInt16LE(1, 20);
  wav.writeUInt16LE(1, 22);
  wav.writeUInt32LE(48000, 24);
  wav.writeUInt32LE(96000, 28);
  wav.writeUInt16LE(2, 32);
  wav.writeUInt16LE(16, 34);
  wav.write("data", 36);
  wav.writeUInt32LE(9600, 40);
  for (let i = 0; i < 4800; i++) wav.writeInt16LE(Math.round(Math.sin(i / 30) * 8000), 44 + i * 2);
  await writeFile(path, wav);
  await writeFile(
    entry,
    "import { Project, chord } from '@oxitone/core'; const p = new Project(); const ch = p.addChannel(); p.addTrack('Lane').use(ch).pattern(chord(60, 'major')).at({bar:1}); export default p;\n",
  );
  const doc = await ProjectDocument.open({ entry });
  cleanup.push(async () => doc.close());
  expect(doc.view.status, doc.view.diagnostic?.message).toBe("ready");
  return { root, entry, path, doc };
}
it("imports via Rust, saves a portable source transaction, reopens and undoes it without deleting assets", async () => {
  const { root, entry, path, doc } = await fixture();
  await doc.sampleDrop(0, {
    source: { kind: "file", path },
    destination: { kind: "arrangement", track: 0, startBeat: 2.5 },
  });
  const sample = doc.frame!.snapshot.samples[0]!;
  expect(sample.assetUri).toMatch(/^assets\/samples\/[a-f0-9]{64}\.wav$/);
  expect(sample.frames).toBe("4800");
  expect(doc.frame!.snapshot.sampleClips[0]!.startBeat).toEqual({ numerator: 5, denominator: 2 });
  await doc.save(1);
  const source = await readFile(entry, "utf8");
  expect(source).toContain(".useSample(");
  expect(source).not.toContain(path);
  expect(source).not.toContain(sample.id);
  const reopened = await ProjectDocument.open({ entry });
  try {
    expect(reopened.view.status, reopened.view.diagnostic?.message).toBe("ready");
    expect(reopened.frame!.snapshot.samples).toEqual(doc.frame!.snapshot.samples);
  } finally {
    reopened.close();
  }
  await doc.undo(1);
  expect(doc.frame!.snapshot.samples).toHaveLength(0);
  expect((await readFile(join(root, sample.assetUri))).length).toBeGreaterThan(44);
  await doc.redo(2);
  expect(doc.frame!.snapshot.samples).toHaveLength(1);
});
it("rejects unsupported plugin drops and symlink asset directories without publishing draft mutations", async () => {
  const { root, path, doc } = await fixture();
  const before = doc.view.files;
  await expect(
    doc.sampleDrop(0, {
      source: { kind: "file", path },
      destination: { kind: "plugin", owner: "channel", index: 0, resource: "sample" },
    }),
  ).rejects.toThrow();
  expect(doc.view.files).toEqual(before);
  expect(doc.view.revision).toBe(0);
  await rm(join(root, "assets"), { recursive: true });
  await symlink(tmpdir(), join(root, "assets"));
  await expect(
    doc.sampleDrop(0, { source: { kind: "file", path }, destination: { kind: "arrangement", startBeat: 0 } }),
  ).rejects.toMatchObject({ code: "AssetUnavailable" });
  expect(doc.view.revision).toBe(0);
});
it("batch duplicates and clipboard paste round-trip source and one-step Undo", async () => {
  const { doc } = await fixture();
  await doc.arrange(0, {
    action: "batch",
    edits: [4, 8, 12].map((startBeat) => ({
      action: "duplicate",
      kind: "pattern",
      resource: 0,
      clip: 0,
      track: 0,
      startBeat,
    })),
  });
  expect(doc.frame!.snapshot.patternClips).toHaveLength(4);
  await doc.undo(1);
  expect(doc.frame!.snapshot.patternClips).toHaveLength(1);
  const { id: _id, patternId: _pattern, trackId: _track, ...settings } = doc.frame!.snapshot.patternClips[0]!;
  await doc.arrange(2, {
    action: "paste",
    kind: "pattern",
    resource: 0,
    track: 0,
    startBeat: 8,
    settings,
    channels: [0],
  });
  expect(doc.frame!.snapshot.patternClips).toHaveLength(2);
});
it("saves and reopens a sampler resource replacement without changing controls or automation binding", async () => {
  const { root, entry, path, doc } = await fixture();
  const cached = cacheSample(path, join(root, "cache"));
  const sample = {
    id: "smp_source",
    assetUri: `cache/${cached.sha256}.wav`,
    sha256: cached.sha256,
    format: "wav",
    sampleRate: cached.sampleRate,
    channels: cached.channels,
    frames: cached.frames,
  };
  await doc.changeCode(
    0,
    entry,
    `import { Project, sampler, chord, createAutomationNamespace } from '@oxitone/core';
const p = new Project(); const audio = p.importSampleRef(${JSON.stringify(sample)});
const ch = p.addChannel({ instrument: sampler(audio, { level: 0.6 }) });
ch.instrumentInstance.param('level').automate(createAutomationNamespace().constant(0.4));
p.addTrack('Sampler').use(ch).pattern(chord(60, 'major')).at({bar:1}); export default p;\n`,
  );
  expect(doc.view.status, doc.view.diagnostic?.message).toBe("ready");
  const before = doc.frame!.snapshot;
  const wav = await readFile(path);
  wav.writeInt16LE(8000, 44);
  await writeFile(path, wav);
  await doc.sampleDrop(1, {
    source: { kind: "file", path },
    destination: { kind: "plugin", owner: "channel", index: 0, resource: "sample" },
  });
  const after = doc.frame!.snapshot;
  expect(after.channels[0]!.instrument.instanceId).toBe(before.channels[0]!.instrument.instanceId);
  expect(after.channels[0]!.instrument.parameters).toEqual(before.channels[0]!.instrument.parameters);
  expect(after.automation).toEqual(before.automation);
  expect(after.channels[0]!.instrument.resources).not.toEqual(before.channels[0]!.instrument.resources);
  await doc.save(2);
  const reopened = await ProjectDocument.open({ entry });
  try {
    expect(reopened.view.status, reopened.view.diagnostic?.message).toBe("ready");
    expect(reopened.frame!.snapshot.channels).toEqual(after.channels);
    expect(reopened.frame!.snapshot.automation).toEqual(after.automation);
    expect(reopened.frame!.snapshot.samples).toEqual(after.samples);
  } finally {
    reopened.close();
  }
});
