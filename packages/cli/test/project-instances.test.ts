import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";
import { ProjectDocument } from "../src/source/index.js";

it("maps repeated shared addEffect inputs to returned instances, preserves bindings after reorder, and saves ID-free code", async () => {
  const root = await mkdtemp(join(tmpdir(), "oxitone-instance-document-")); let document: ProjectDocument | undefined;
  try {
    await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
    await symlink(fileURLToPath(new URL("../../core", import.meta.url)), join(root, "node_modules/@oxitone/core"), "dir");
    const entry = join(root, "song.ts");
    const source = `import { Project, effect, createAutomationNamespace } from '@oxitone/core';
const project = new Project();
const channel = project.addChannel();
const shared = effect('delay', { feedback: .3 });
const first = channel.addEffect(shared);
const second = channel.addEffect(shared);
first.param('feedback').automate(createAutomationNamespace().constant(.4));
channel.reorderEffects([second, first]);
export default project;
`;
    await writeFile(entry, source); document = await ProjectDocument.open({ entry });
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    const before = document.frame!.snapshot, target = before.automation[0]!.target.entityId;
    expect(before.channels[0]!.effectChain[1]!.instanceId).toBe(target);
    const local = document.view.configurationSites.find(site => site.scope === "reference" && site.expression === "shared" && site.usages.length === 1 && site.usages[0]!.handle === target)!;
    expect(local).toBeDefined();
    await document.editConfiguration(0, local.handle, { kind: "parameters", values: { feedback: .6 } }, target);
    expect(document.frame!.snapshot.channels[0]!.effectChain[1]!.parameters.feedback).toBe(.6);
    expect(document.frame!.snapshot.channels[0]!.effectChain[0]!.parameters.feedback).toBe(.3);
    expect(document.frame!.snapshot.automation).toEqual(before.automation);
    const saved = document.view.files[0]!.text;
    expect(saved).toContain("channel.reorderEffects([second, first])");
    expect(saved).not.toMatch(/instanceId|ins_[a-f0-9]|__oxitone/);
    await document.save(1); document.close(); document = await ProjectDocument.open({ entry });
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    expect(document.frame!.snapshot.channels[0]!.effectChain[1]!).toMatchObject({ instanceId: target, parameters: { feedback: .6 } });
    expect(document.frame!.snapshot.automation).toEqual(before.automation);
    expect(await readFile(entry, "utf8")).toBe(saved);
  } finally { document?.close(); await rm(root, { recursive: true, force: true }); }
});
