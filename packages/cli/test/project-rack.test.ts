import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { expect, it } from "vitest";
import { ProjectDocument } from "../src/source/index.js";

it("reviews and detaches a real npm C-plugin serial rack, keeps bindings and side effects, then edits one local effect and reopens", async () => {
  const root = await mkdtemp(join(tmpdir(), "oxitone-rack-document-"));
  let document: ProjectDocument | undefined;
  try {
    await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
    await symlink(
      fileURLToPath(new URL("../../core", import.meta.url)),
      join(root, "node_modules/@oxitone/core"),
      "dir",
    );
    const pkg = join(root, "node_modules/rack-fixture");
    await mkdir(pkg);
    await writeFile(
      join(pkg, "package.json"),
      '{"name":"rack-fixture","version":"1.0.0","type":"module","exports":"./index.js"}',
    );
    const repo = fileURLToPath(new URL("../../../", import.meta.url)),
      fixtures = join(repo, "crates/render/tests/fixtures");
    const manifest = JSON.parse(await readFile(join(fixtures, "gain.json"), "utf8"));
    const binary = join(pkg, process.platform === "darwin" ? "gain.dylib" : "gain.so");
    await promisify(execFile)("cc", [
      "-std=c11",
      "-shared",
      "-fPIC",
      "-O2",
      "-I",
      join(repo, "include"),
      join(fixtures, "gain.c"),
      "-o",
      binary,
    ]);
    const binaryHash = createHash("sha256")
      .update(await readFile(binary))
      .digest("hex");
    const factory = `export const registration = ${JSON.stringify({ libraryPath: binary, expectedHash: binaryHash, manifest })};
export let calls = 0; export function rack() { calls++; return [
  { pluginId: 'fixture.gain', pluginVersion: '1.0.0', parameters: { gain: .7 }, mix: .8 },
  { pluginId: 'fixture.gain', pluginVersion: '1.0.0', parameters: { gain: .4 }, bypass: true }
]; }`;
    await writeFile(join(pkg, "index.js"), factory);
    const entry = join(root, "song.ts");
    const source =
      "import { Project, createAutomationNamespace } from '@oxitone/core';\nimport { registration, rack, calls } from 'rack-fixture';\nconst project = new Project(); project.registerPlugin(registration, { allowPlugins: 'any' });\nconst shared = rack();\nconst first = project.addChannel({ effectChain: rack() });\nconst second = project.addChannel({ effectChain: shared });\nconst bus = project.addMixerChannel({ inserts: shared });\nfirst.effectInstances[0].param('gain').automate(createAutomationNamespace().sine({ periodBeats: 4 }));\nproject.addTrack('Factory calls: ' + calls);\nexport default project;\n";
    await writeFile(entry, source);
    document = await ProjectDocument.open({ entry });
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    const before = document.frame!.snapshot,
      first = before.channels[0]!.id;
    const site = () =>
      document!.view.rackSites.find(
        (site) => site.scope === "reference" && site.usages.length === 1 && site.usages[0]!.owner === first,
      )!;
    await document.planMaterializeRack(0, site().handle, first);
    const review = document.view.rackMaterialization!;
    expect(review).toMatchObject({ effects: 2, affectedOwners: [first], retainsOriginalEvaluation: true });
    expect(document.view.revision).toBe(0);
    expect(document.view.files[0]!.text).toBe(source);
    expect(await readFile(entry, "utf8")).toBe(source);
    document.cancelMaterialize(0, review.planId);
    await expect(document.confirmMaterialize(0, review.planId)).rejects.toMatchObject({ code: "SourceChanged" });
    await document.planMaterializeRack(0, site().handle, first);
    await document.confirmMaterialize(0, document.view.rackMaterialization!.planId);
    expect(document.frame!.snapshot.automation).toEqual(before.automation);
    expect(document.frame!.snapshot.tracks[0]!.name).toBe("Factory calls: 2");
    expect(document.frame!.snapshot.channels).toEqual(before.channels);
    const local = () =>
      document!.view.configurationSites.find(
        (site) =>
          site.scope === "reference" &&
          site.usages.length === 1 &&
          site.usages[0]!.owner === first &&
          site.usages[0]!.index === 1,
      )!;
    for (const gain of [0.5, 0.6, 0.9])
      await document.editConfiguration(
        document.view.revision,
        local().handle,
        { kind: "parameters", values: { gain } },
        local().usages[0]!.handle,
      );
    expect(document.frame!.snapshot.channels[0]!.effectChain[1]!.parameters.gain).toBe(0.9);
    expect(document.frame!.snapshot.channels[0]!.effectChain[0]).toEqual(before.channels[0]!.effectChain[0]);
    expect(document.frame!.snapshot.channels[1]).toEqual(before.channels[1]);
    expect(document.frame!.snapshot.mixerChannels).toEqual(before.mixerChannels);
    expect(document.view.files[0]!.text).not.toContain("withParameters(");
    expect(document.view.files[0]!.text).not.toMatch(/instanceId|ins_[a-f0-9]/);
    const revision = document.view.revision;
    await expect(
      document.editConfiguration(revision, local().handle, { kind: "parameters", values: { gain: 3 } }),
    ).rejects.toBeDefined();
    expect(document.view.revision).toBe(revision);
    await document.save(revision);
    document.close();
    document = await ProjectDocument.open({ entry });
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    expect(document.frame!.snapshot.channels[0]!.effectChain[1]!.parameters.gain).toBe(0.9);
    expect(document.frame!.snapshot.automation).toEqual(before.automation);
    expect(await readFile(join(pkg, "index.js"), "utf8")).toBe(factory);
    expect(
      createHash("sha256")
        .update(await readFile(binary))
        .digest("hex"),
    ).toBe(binaryHash);
  } finally {
    document?.close();
    await rm(root, { recursive: true, force: true });
  }
}, 30_000);
