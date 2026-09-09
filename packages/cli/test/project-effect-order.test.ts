import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";
import { ProjectDocument } from "../src/source/index.js";

it("writes and reduces local npm chain permutations, keeps typed bindings and other uses, and rejects stale/legacy targets", async () => {
  const root = await mkdtemp(join(tmpdir(), "oxitone-effect-order-")); let document: ProjectDocument | undefined;
  try {
    await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
    await symlink(fileURLToPath(new URL("../../core", import.meta.url)), join(root, "node_modules/@oxitone/core"), "dir");
    const pkg = join(root, "node_modules/preset"); await mkdir(pkg);
    await writeFile(join(pkg, "package.json"), '{"name":"preset","version":"1.0.0","type":"module","exports":"./index.js"}');
    const factory = "import { effect } from '@oxitone/core'; export function rack() { return [effect('delay', { feedback: .2 }), effect('delay', { feedback: .6 })]; }";
    await writeFile(join(pkg, "index.js"), factory);
    const entry = join(root, "song.ts");
    const code = `import { Project, createAutomationNamespace, orderEffects as ordered } from '@oxitone/core';
import { rack } from 'preset';
const project = new Project();
const first = project.addChannel({ effectChain: rack() });
const other = project.addMixerChannel({ inserts: rack() });
first.effectInstances[0].param('feedback').automate(createAutomationNamespace().constant(.4));
export { first };
export default project;
`;
    await writeFile(entry, code); document = await ProjectDocument.open({ entry });
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    const before = document.frame!.snapshot, owner = before.channels[0]!.id;
    const order = before.channels[0]!.effectChain.map(ref => ref.instanceId!);
    const site = () => document!.view.effectOwnerSites!.find(site => site.owner === owner && site.scope === "definition")!;
    const stale = site().handle;
    await document.editEffectOrder(0, stale, owner, [...order].reverse());
    expect(document.frame!.snapshot.channels[0]!.effectChain).toEqual([...before.channels[0]!.effectChain].reverse());
    expect(document.frame!.snapshot.automation).toEqual(before.automation);
    expect(document.frame!.snapshot.mixerChannels).toEqual(before.mixerChannels);
    expect(document.view.files[0]!.text).toContain("ordered(first, [1, 0])");
    await expect(document.editEffectOrder(1, stale, owner, order)).rejects.toMatchObject({ code: "EditTargetMissing" });
    await document.editEffectOrder(1, site().handle, owner, order);
    expect(document.view.files[0]!.text).not.toContain("ordered(first,");
    await document.editEffectOrder(2, site().handle, owner, [...order].reverse());
    const saved = document.view.files[0]!.text;
    expect(saved.match(/ordered\(/g)).toHaveLength(1); expect(saved).not.toMatch(/instanceId|ins_[a-f0-9]|__oxitone/);
    expect(saved).toContain("export { first }");
    await document.save(3); document.close(); document = await ProjectDocument.open({ entry });
    expect(document.frame!.snapshot.channels[0]!.effectChain.map(ref => ref.instanceId)).toEqual([...order].reverse());
    expect(document.frame!.snapshot.automation).toEqual(before.automation);
    expect(await readFile(join(pkg, "index.js"), "utf8")).toBe(factory);
    // A legacy index lane must not silently follow a different plugin after a subsequent reorder.
    const legacy = code.replace("first.effectInstances[0].param('feedback').automate(", "first.automate('insert.0.parameter.feedback', ");
    await document.changeCode(0, entry, legacy);
    expect(document.view.status).toBe("ready");
    await expect(document.editEffectOrder(1, site().handle, owner, [...order].reverse())).rejects.toMatchObject({ code: "EditScopeConflict" });
    expect(document.view.files[0]!.text).toBe(legacy);
  } finally { document?.close(); await rm(root, { recursive: true, force: true }); }
}, 30_000);
