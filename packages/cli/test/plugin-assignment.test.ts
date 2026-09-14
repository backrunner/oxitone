import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { expect, it } from "vitest";
import { ProjectDocument } from "../src/source/index.js";

it("registers an unused npm effect in local TS, preserves package bytes, follows instance selection and reopens", async () => {
  const root = await mkdtemp(join(tmpdir(), "oxitone-assign-package-"));
  let document: ProjectDocument | undefined;
  try {
    await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
    await symlink(
      fileURLToPath(new URL("../../core", import.meta.url)),
      join(root, "node_modules/@oxitone/core"),
      "dir",
    );
    const repo = fileURLToPath(new URL("../../../", import.meta.url));
    const original = await readFile(join(repo, "crates/render/tests/fixtures/gain.c"), "utf8");
    const manifest = JSON.parse(await readFile(join(repo, "crates/render/tests/fixtures/gain.json"), "utf8"));
    const pkg = join(root, "store/package");
    await mkdir(pkg, { recursive: true });
    await symlink(pkg, join(root, "node_modules/gain-pack"), "dir");
    const library = process.platform === "darwin" ? "gain.dylib" : "gain.so";
    const compile = async (source: string, binary: string) => {
      const input = binary + ".c";
      await writeFile(input, source);
      await promisify(execFile)("cc", [
        "-std=c11",
        "-shared",
        "-fPIC",
        "-O2",
        "-I",
        join(repo, "include"),
        input,
        "-o",
        binary,
      ]);
      return createHash("sha256")
        .update(await readFile(binary))
        .digest("hex");
    };
    const initialBinary = join(root, "initial.dylib");
    const initialHash = await compile(original.replaceAll("fixture.gain", "fixture.initial"), initialBinary);
    const binary = join(pkg, library),
      sha256 = await compile(original, binary);
    await writeFile(join(root, "package.json"), '{"dependencies":{"gain-pack":"1.0.0"}}');
    const packageJson =
      '{"name":"gain-pack","version":"1.0.0","type":"module","exports":"./index.js","oxitone":{"plugins":"plugins.json"}}';
    await writeFile(join(pkg, "package.json"), packageJson);
    const packageSource = "throw new Error('Package entry must never execute');";
    await writeFile(join(pkg, "index.js"), packageSource);
    const metadata = JSON.stringify({
      formatVersion: 1,
      plugins: [
        {
          displayName: "Gain",
          vendor: "Fixture",
          manifest,
          platforms: { [`${process.platform}-${process.arch}`]: { library, sha256 } },
        },
      ],
    });
    await writeFile(join(pkg, "plugins.json"), metadata);
    const entry = join(root, "song.ts");
    await writeFile(
      entry,
      `import { Project, effect, createAutomationNamespace } from '@oxitone/core';
const project = new Project();
project.registerPlugin(${JSON.stringify({ libraryPath: initialBinary, expectedHash: initialHash, manifest: { ...manifest, pluginId: "fixture.initial" } })}, { allowPlugins: 'any' });
const channel = project.addChannel();
const bound = channel.addEffect(effect('delay'));
bound.param('feedback').automate(createAutomationNamespace().constant(.4));
export default project;
`,
    );
    document = await ProjectDocument.open({ entry });
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    const owner = document.frame!.snapshot.channels[0]!.id;
    const plugin = document.view.plugins.find((p) => p.pluginId === "fixture.gain")!;
    await document.assignPlugin(0, { plugin: plugin.handle, owner, target: "channelInsert" });
    const effects = () => document!.frame!.snapshot.channels[0]!.effectChain;
    expect(effects()[1]!.pluginId).toBe("fixture.gain");
    const use = effects()[1]!.instanceId;
    const source = document.view.configurationSites.find((s) => s.usages.some((u) => u.handle === use))!;
    expect(source, "external assignment remains editable in the instance editor").toBeDefined();
    await document.editConfiguration(
      document.view.revision,
      source.handle,
      { kind: "parameters", values: { gain: 0.7 } },
      use,
    );
    expect(effects()[1]!.parameters.gain).toBe(0.7);
    const binding = document.frame!.snapshot.automation[0]!.target.entityId;
    const snapshot = document.frame!.snapshot,
      text = document.view.files[0]!.text;
    expect(text).toContain('import.meta.dirname + "/node_modules/gain-pack/');
    expect(text).not.toContain("/store/package");
    expect(text).not.toContain("instanceId");
    await expect(
      document.assignPlugin(document.view.revision, {
        plugin: plugin.handle,
        owner,
        target: "channelInsert",
        instance: binding,
      }),
    ).rejects.toThrow();
    expect(document.frame!.snapshot).toEqual(snapshot);
    expect(document.view.files[0]!.text).toBe(text);
    await document.save(document.view.revision);
    document.close();
    document = await ProjectDocument.open({ entry });
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    expect(effects()).toEqual(snapshot.channels[0]!.effectChain);
    expect(document.frame!.plugins).toHaveLength(2);
    expect(await readFile(join(pkg, "index.js"), "utf8")).toBe(packageSource);
    expect(await readFile(join(pkg, "package.json"), "utf8")).toBe(packageJson);
    expect(await readFile(join(pkg, "plugins.json"), "utf8")).toBe(metadata);
    expect(
      createHash("sha256")
        .update(await readFile(binary))
        .digest("hex"),
    ).toBe(sha256);
  } finally {
    document?.close();
    await rm(root, { recursive: true, force: true });
  }
});
