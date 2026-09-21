import { mkdir, mkdtemp, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, it } from "vitest";
import { pluginInstallManifestSchema, pluginCatalogEntrySchema, vst3InfoSchema } from "@oxitone/protocol";
import { discoverProjectPlugins } from "../src/source/plugins/plugin-discovery.js";
import { vst3PluginKind } from "../src/source/plugins/vst3-discovery.js";

it("discovers package VST3 classes statically and rejects bundle escapes without executing package code", async () => {
  const root = await mkdtemp(join(tmpdir(), "oxitone-vst3-catalog-"));
  try {
    const pkg = join(root, "node_modules/gain-pack");
    await mkdir(join(pkg, "Gain.vst3"), { recursive: true });
    await mkdir(join(root, "External.vst3"));
    await symlink(join(root, "External.vst3"), join(pkg, "Linked.vst3"));
    await writeFile(join(root, "package.json"), JSON.stringify({ dependencies: { "gain-pack": "1.0.0" } }));
    await writeFile(
      join(pkg, "package.json"),
      JSON.stringify({ version: "1.0.0", main: "index.js", oxitone: { plugins: "plugins.json" } }),
    );
    await writeFile(join(pkg, "index.js"), "throw new Error('Must never execute');");
    const base = {
      pluginId: "gain",
      pluginVersion: "1",
      displayName: "Gain",
      vendor: "Fixture",
      kind: "effect",
      classId: "1".repeat(32),
    };
    await writeFile(
      join(pkg, "plugins.json"),
      JSON.stringify({
        formatVersion: 1,
        vst3: [
          { ...base, bundle: "Gain.vst3" },
          { ...base, bundle: "../../External.vst3" },
          { ...base, bundle: "Linked.vst3" },
        ],
      }),
    );
    const { plugins } = await discoverProjectPlugins(root);
    expect(plugins).toHaveLength(3);
    for (const plugin of plugins) expect(() => pluginCatalogEntrySchema.parse(plugin.entry)).not.toThrow();
    expect(plugins[0]!.entry).toMatchObject({ source: "vst3", validation: "unverified", parameters: [] });
    expect(plugins[1]!.entry.availability).toBe("missing");
    expect(plugins[2]!.entry.availability).toBe("missing");
    expect(new Set(plugins.map((p) => p.entry.handle)).size).toBe(3);
    expect(plugins.every((p) => !p.registration)).toBe(true);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
it("rejects empty and unsupported persisted install formats", () => {
  for (const input of [{ formatVersion: 1 }, { formatVersion: 2, vst3: [] }])
    expect(() => pluginInstallManifestSchema.parse(input)).toThrow();
});

it("classifies MIDI-only processors as Channel instruments even when vendors label them Fx", () => {
  for (const noteInput of [false, true]) {
    const info = vst3InfoSchema.parse({
      protocolVersion: 1,
      classId: "1".repeat(32),
      sha256: "a".repeat(64),
      name: "MIDI processor",
      vendor: "Fixture",
      version: "1",
      category: "Fx|Tools",
      inputChannels: 0,
      outputChannels: 0,
      audioBuses: { inputs: [], outputs: [] },
      noteInput,
      noteOutput: true,
      parameters: [],
      configuration: null,
    });
    expect(vst3PluginKind(info)).toBe("instrument");
  }
});
