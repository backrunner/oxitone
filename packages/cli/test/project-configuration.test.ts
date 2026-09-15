import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";
import { ProjectDocument } from "../src/source/index.js";

it("edits local or shared npm configurations, distinguishes plugin/host names, rejects invalid values and saves imports", async () => {
  const root = await mkdtemp(join(tmpdir(), "oxitone-config-document-"));
  let document: ProjectDocument | undefined;
  try {
    await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
    await symlink(
      fileURLToPath(new URL("../../core", import.meta.url)),
      join(root, "node_modules/@oxitone/core"),
      "dir",
    );
    const pkg = join(root, "node_modules/presets");
    await mkdir(pkg);
    await writeFile(
      join(pkg, "package.json"),
      '{"name":"presets","version":"1.0.0","type":"module","exports":"./index.js"}',
    );
    const factory =
      "import { wavetable, effect } from '@oxitone/core'; export const voice = () => wavetable({ level: .6 }); export const echo = () => effect('delay', { feedback: .3 }, { mix: .4 });";
    await writeFile(join(pkg, "index.js"), factory);
    const entry = join(root, "song.ts");
    const source =
      "import { Project, pluginConfig as config } from '@oxitone/core';\nimport type { PluginConfig } from '@oxitone/core';\nimport { voice, echo } from 'presets';\nconst shared = voice(); const delay = echo(); const project = new Project();\nconst first = project.addChannel({ instrument: shared, effectChain: [delay], level: .8 });\nconst second = project.addChannel({ instrument: shared, effectChain: [delay] });\nproject.master.addEffect(delay);\nexport default project;\n";
    await writeFile(entry, source);
    document = await ProjectDocument.open({ entry });
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    const definition = document.view.configurationSites.find(
      (site) => site.label === "shared" && site.scope === "definition",
    )!;
    expect(definition.usages).toHaveLength(2);
    await document.editConfiguration(0, definition.handle, { kind: "parameters", values: { level: 0.7 } });
    expect(document.frame!.snapshot.channels.map((channel) => channel.instrument.parameters.level)).toEqual([0.7, 0.7]);
    await document.undo(1);
    const first = document.frame!.snapshot.channels[0]!.id;
    const local = () =>
      document!.view.configurationSites.find(
        (site) =>
          site.scope === "reference" &&
          site.label === "instrument" &&
          site.usages.length === 1 &&
          site.usages[0]!.owner === first,
      )!;
    for (const level of [0.4, 0.3, 0.2, 0.1]) {
      const site = local();
      expect(site).toBeDefined();
      await document.editConfiguration(
        document.view.revision,
        site.handle,
        { kind: "parameters", values: { level } },
        site.usages[0]!.handle,
      );
      expect(document.frame!.snapshot.channels[0]!.level).toBe(0.8);
      expect(document.frame!.snapshot.channels[1]!.instrument.parameters.level).toBe(0.6);
    }
    expect(document.view.files[0]!.text.match(/withParameters\(/g)).toHaveLength(1);
    expect(document.view.files[0]!.text).toContain("config('instrument', shared)");
    const revision = document.view.revision;
    await expect(
      document.editConfiguration(revision, local().handle, { kind: "parameters", values: { nonexistent: 1 } }),
    ).rejects.toBeDefined();
    expect(document.view.revision).toBe(revision);
    expect(document.view.status).toBe("ready");
    const fx = document.view.configurationSites.find(
      (site) =>
        site.scope === "reference" &&
        site.usages.length === 1 &&
        site.usages[0]!.owner === first &&
        site.kind === "effect",
    )!;
    await document.editConfiguration(
      revision,
      fx.handle,
      { kind: "host", values: { mix: 0.2, bypass: true } },
      fx.usages[0]!.handle,
    );
    expect(document.frame!.snapshot.channels[0]!.effectChain[0]).toMatchObject({
      parameters: { feedback: 0.3 },
      mix: 0.2,
      bypass: true,
    });
    expect(document.frame!.snapshot.channels[1]!.effectChain[0]!.mix).toBe(0.4);
    expect(document.frame!.snapshot.mixerChannels[0]!.inserts[0]!.mix).toBe(0.4);
    const saved = document.view.files[0]!.text;
    await document.save(document.view.revision);
    document.close();
    document = await ProjectDocument.open({ entry });
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    expect(document.frame!.snapshot.channels[0]!.instrument.parameters.level).toBe(0.1);
    expect(await readFile(entry, "utf8")).toBe(saved);
    expect(await readFile(join(pkg, "index.js"), "utf8")).toBe(factory);
    expect(saved).not.toMatch(/sessionId|__oxitone_project_|:effect:|:instrument/);
  } finally {
    document?.close();
    await rm(root, { recursive: true, force: true });
  }
});
