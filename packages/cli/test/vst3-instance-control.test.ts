import { mkdir, mkdtemp, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, it, vi } from "vitest";
import { vst3InfoSchema } from "@oxitone/protocol";
import { evaluateSourceProject } from "../src/source/eval/project-evaluation.js";
import { SourceOwnership } from "../src/source/files/ownership.js";
import { writeVst3Configuration } from "../src/source/editing/vst3-configuration-writer.js";
import { controlVst3Instance } from "../src/source/plugins/vst3-instance-control.js";
import type { Vst3Runtime } from "../src/preview/vst3-client.js";
import { assertProjectConfigurationEdit } from "../src/source/document/project-equivalence.js";

it("controls one live instance and replaces captured source state while preserving identities and host settings", async () => {
  const root = await mkdtemp(join(tmpdir(), "oxitone-vst3-instance-"));
  try {
    await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
    await symlink(
      fileURLToPath(new URL("../../core", import.meta.url)),
      join(root, "node_modules/@oxitone/core"),
      "dir",
    );
    const entry = join(root, "song.ts"),
      classId = "1".repeat(32),
      sha256 = "a".repeat(64);
    const state = { formatVersion: 1 as const, classId, sha256, stateBase64: "AQID", parameters: { "9": 0.5 } };
    const config = {
      pluginId: `vst3.${classId}`,
      pluginVersion: `0.0.0+${sha256}`,
      parameters: { "9": 0.7, "8": 0.1 },
      state,
      mix: 0.3,
      bypass: true,
    };
    let text = `import { Project } from '@oxitone/core';
const preset = ${JSON.stringify(config)};
const project = new Project({seed: 88});
project.addChannel({effectChain: [preset]});
project.addChannel({effectChain: [preset]});
export default project;
`;
    await writeFile(entry, text);
    const ownership = await SourceOwnership.open([root], [entry]);
    const signal = new AbortController().signal;
    let before = await evaluateSourceProject(entry, new Map([[entry, text]]), ownership, signal);
    const owner = before.frame.snapshot.channels[0]!.id;
    const metadata = vst3InfoSchema.parse({
      protocolVersion: 1,
      classId,
      sha256,
      name: "Fixture",
      vendor: "Test",
      version: "1",
      category: "Fx",
      inputChannels: 2,
      outputChannels: 2,
      audioBuses: { inputs: [{ channels: 2, active: true }], outputs: [{ channels: 2, active: true }] },
      noteInput: false,
      noteOutput: false,
      parameters: [],
      configuration: state,
    });
    const findSite = () =>
      before.configurationSites.find(
        (s) => s.scope === "reference" && s.usages.length === 1 && s.usages[0]!.owner === owner,
      )!;
    before.frame.vst3Plugins = [
      {
        registrationVersion: 1,
        source: { bundlePath: "/tmp/Fixture.vst3", classId, allowPlugins: "any" },
        helperPath: "/tmp/host",
        metadata,
      },
    ];
    const instanceId = findSite().usages[0]!.handle;
    const runtime = {
      inventory: vi.fn<Vst3Runtime["inventory"]>(async () => ({
        instanceControlVersion: 1,
        graphGeneration: "17",
        state: "active",
        instances: [{ instanceId, pluginId: config.pluginId, pluginVersion: config.pluginVersion }],
      })),
      control: vi.fn<Vst3Runtime["control"]>(async () => ({
        instanceControlVersion: 1,
        graphGeneration: "17",
        instanceId,
        state: { editorOpen: true, nextSequence: 5, info: metadata },
      })),
    };
    expect(
      await controlVst3Instance(before, findSite(), "1", { kind: "openEditor" }, runtime, signal, () => {}),
    ).toBeUndefined();
    expect(runtime.control).toHaveBeenCalledWith(
      "1",
      expect.objectContaining({ graphGeneration: "17", instanceId, command: { kind: "openEditor" } }),
      signal,
    );
    expect(await controlVst3Instance(before, findSite(), "1", { kind: "capture" }, runtime, signal, () => {})).toEqual(
      state,
    );
    await expect(
      controlVst3Instance(before, findSite(), "1", { kind: "openEditor" }, undefined, signal, () => {}),
    ).rejects.toMatchObject({ code: "PluginHostUnavailable" });
    runtime.control.mockResolvedValueOnce({
      instanceControlVersion: 1,
      graphGeneration: "17",
      instanceId,
      state: { editorOpen: true, nextSequence: 5, info: { ...metadata, classId: "2".repeat(32) } },
    });
    await expect(
      controlVst3Instance(before, findSite(), "1", { kind: "capture" }, runtime, signal, () => {}),
    ).rejects.toMatchObject({ code: "PluginManifestMismatch" });
    await expect(
      controlVst3Instance(before, findSite(), "1", { kind: "capture" }, runtime, signal, () => {
        throw new Error("stale");
      }),
    ).rejects.toThrow("stale");
    for (const value of [0.2, 0.4, 0.6]) {
      const site = findSite();
      const next = {
        ...state,
        stateBase64: Buffer.from([Math.round(value * 10)]).toString("base64"),
        parameters: { "9": value },
      };
      const candidate = writeVst3Configuration(text, site, next);
      const after = await evaluateSourceProject(entry, new Map([[entry, candidate.text]]), ownership, signal);
      expect(() =>
        assertProjectConfigurationEdit(before.frame.snapshot, after.frame.snapshot, site.usages, candidate.config),
      ).not.toThrow();
      expect(after.frame.snapshot.channels[0]!.effectChain[0]).toMatchObject({
        mix: 0.3,
        bypass: true,
        parameters: { "9": value },
        state: next,
      });
      expect(after.frame.snapshot.channels[1]!.effectChain[0]!.state).toEqual(state);
      expect(after.frame.snapshot.channels[0]!.effectChain[0]!.parameters).toEqual({ "9": value });
      text = candidate.text;
      before = after;
      expect(text.match(/withState\(/g)).toHaveLength(1);
      expect(text.match(/replaceParameters\(/g)).toHaveLength(1);
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
