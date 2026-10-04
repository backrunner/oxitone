import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { expect, it } from "vitest";
import { Project } from "@oxitone/core";
import { ProjectDocument } from "../src/source/index.js";

function pcm(wav: Buffer) {
  expect(wav.toString("ascii", 0, 4)).toBe("RIFF");
  for (let offset = 12; offset + 8 <= wav.length;) {
    const size = wav.readUInt32LE(offset + 4);
    if (wav.toString("ascii", offset, offset + 4) === "data")
      return Array.from({ length: size / 4 }, (_, i) => wav.readFloatLE(offset + 8 + i * 4));
    offset += 8 + size + (size % 2);
  }
  throw new Error("Missing rendered PCM");
}

it("round-trips an actual C plugin through atomic source edits, rejection, Undo/Redo and saved offline PCM", async () => {
  const root = await mkdtemp(join(tmpdir(), "oxitone-external-configuration-"));
  let document: ProjectDocument | undefined;
  try {
    await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
    await symlink(
      fileURLToPath(new URL("../../core", import.meta.url)),
      join(root, "node_modules/@oxitone/core"),
      "dir",
    );
    const pkg = join(root, "node_modules/configuration-fixture");
    await mkdir(pkg);
    await writeFile(
      join(pkg, "package.json"),
      JSON.stringify({ name: "configuration-fixture", type: "module", exports: "./index.js" }),
    );
    const libraryPath = join(pkg, "configuration.dylib");
    await promisify(execFile)("cc", [
      "-std=c11",
      "-shared",
      "-fPIC",
      "-O2",
      "-I",
      fileURLToPath(new URL("../../../include", import.meta.url)),
      fileURLToPath(new URL("./fixtures/configuration.c", import.meta.url)),
      "-o",
      libraryPath,
    ]);
    const binary = await readFile(libraryPath);
    const manifest = {
      pluginId: "fixture.configuration",
      pluginVersion: "1.0.0",
      abiMajor: 1,
      abiMinor: 0,
      minHostVersion: "0.1.0",
      kind: "effect",
      inputLayout: "stereo",
      outputLayout: "stereo",
      sidechainInput: false,
      reportsTail: false,
      parameters: [
        ["band.2.gain", "Band gain", 2],
        ["mix", "Plugin mix", 1],
      ].map(([id, label, max]) => ({
        id,
        label,
        max,
        min: 0,
        default: 1,
        unit: "normalized",
        smoothing: "none",
        rate: "audio",
        automation: true,
        mapping: "linear",
      })),
    };
    const registration = { libraryPath, expectedHash: createHash("sha256").update(binary).digest("hex"), manifest };
    const factory = `export const registration = ${JSON.stringify(registration)};
export const preset = () => ({pluginId:'fixture.configuration', pluginVersion:'1.0.0', parameters:{'band.2.gain':0.5,mix:0.8}, mix:0.75, bypass:false});`;
    await writeFile(join(pkg, "index.js"), factory);
    const entry = join(root, "song.ts");
    const original = `import { Project, Pattern } from '@oxitone/core';
import { registration, preset } from 'configuration-fixture';
const project = new Project({ seed: 881 });
project.registerPlugin(registration, { allowPlugins: 'any' });
const shared = preset();
const first = project.addChannel({ effectChain: [shared] });
project.addChannel({ effectChain: [shared] });
project.master.addEffect(shared);
project.addTrack().use(first).add(new Pattern({lengthBeats:1,notes:[{pitch:60,start:0,duration:0.5,velocity:0.2}]})).at({bar:1});
export default project;
`;
    await writeFile(entry, original);
    document = await ProjectDocument.open({ entry });
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    const first = document.frame!.snapshot.channels[0]!.id;
    const site = () =>
      document!.view.configurationSites.find(
        (s) => s.scope === "reference" && s.usages.length === 1 && s.usages[0]!.owner === first,
      )!;
    const effect = () => document!.frame!.snapshot.channels[0]!.effectChain[0]!;
    const untouched = () => [
      document!.frame!.snapshot.channels[1]!.effectChain,
      document!.frame!.snapshot.mixerChannels[0]!.inserts,
    ];
    const others = structuredClone(untouched());
    const render = async (name: string) => {
      const frame = document!.frame!;
      const project = Project.fromSnapshot(frame.snapshot);
      for (const plugin of frame.plugins) project.registerPlugin(plugin, { allowPlugins: "any" });
      const path = join(root, `${name}.wav`);
      await project.renderWav({ path, bitDepth: "float32", dither: "none", tailSeconds: 0 });
      return readFile(path);
    };
    const baseline = await render("before");
    const patch = { "band.2.gain": 1.2, mix: 0.3 };
    await document.editConfiguration(0, site().handle, { kind: "parameters", values: patch }, site().usages[0]!.handle);
    expect(document.view.revision).toBe(1);
    expect(effect()).toMatchObject({ parameters: patch, mix: 0.75, bypass: false });
    expect(untouched()).toEqual(others);
    await document.editConfiguration(1, site().handle, { kind: "parameters", values: patch }, site().usages[0]!.handle);
    expect(document.view.revision).toBe(1);
    const accepted = document.view.files[0]!.text;
    for (const values of [{ unknown: 1 }, { mix: 2 }, { mix: NaN }, { "band.2.gain": Infinity }]) {
      await expect(document.editConfiguration(1, site().handle, { kind: "parameters", values })).rejects.toBeDefined();
      expect(document.view.revision).toBe(1);
      expect(document.view.files[0]!.text).toBe(accepted);
      expect(effect().parameters).toEqual(patch);
    }
    await expect(
      document.editConfiguration(0, site().handle, { kind: "parameters", values: { mix: 0.6 } }),
    ).rejects.toBeDefined();
    await expect(
      document.editConfiguration(1, site().handle, { kind: "parameters", values: { mix: 0.6 } }, "wrong-instance"),
    ).rejects.toMatchObject({ code: "EditScopeConflict" });
    await document.undo(1);
    expect(await render("undo")).toEqual(baseline);
    await document.redo(document.view.revision);
    expect(effect().parameters).toEqual(patch);
    const revised = await render("edited");
    expect(revised).not.toEqual(baseline);
    // Independent DSP oracle: plugin dry/wet nested inside host Mix, with the master unchanged.
    const ratio = (1 + 0.75 * 0.3 * (1.2 - 1)) / (1 + 0.75 * 0.8 * (0.5 - 1));
    const beforePcm = pcm(baseline),
      afterPcm = pcm(revised);
    expect(afterPcm.length).toBe(beforePcm.length);
    let peak = 0,
      error = 0;
    for (let i = 0; i < afterPcm.length; i++) {
      peak = Math.max(peak, Math.abs(afterPcm[i]!));
      error = Math.max(error, Math.abs(afterPcm[i]! - beforePcm[i]! * ratio));
    }
    expect(peak).toBeGreaterThan(0.001);
    expect(error).toBeLessThan(2e-6);
    await document.editConfiguration(
      document.view.revision,
      site().handle,
      { kind: "host", values: { bypass: true, mix: 0.2 } },
      site().usages[0]!.handle,
    );
    expect(effect()).toMatchObject({ parameters: patch, mix: 0.2, bypass: true });
    await document.undo(document.view.revision);
    expect(await render("undo-host")).toEqual(revised);
    const saved = document.view.files[0]!.text;
    await document.save(document.view.revision);
    document.close();
    document = await ProjectDocument.open({ entry });
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    expect(effect()).toMatchObject({ parameters: patch, mix: 0.75, bypass: false });
    expect(untouched()).toEqual(others);
    expect(await render("reopened")).toEqual(revised);
    expect(await readFile(entry, "utf8")).toBe(saved);
    expect(await readFile(join(pkg, "index.js"), "utf8")).toBe(factory);
    expect(await readFile(libraryPath)).toEqual(binary);
    expect(saved).not.toMatch(/instanceId|sessionId|__oxitone_project_/);
  } finally {
    document?.close();
    await rm(root, { recursive: true, force: true });
  }
});
