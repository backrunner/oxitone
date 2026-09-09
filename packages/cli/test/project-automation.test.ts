import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";
import { ProjectDocument } from "../src/source/index.js";
import { documentRequestSchema } from "@oxitone/protocol";

it("writes shared or single-lane source ranges, reduces repeated drawing and reopens the actual saved project", async () => {
  const root = await mkdtemp(join(tmpdir(), "oxitone-automation-document-")); let document: ProjectDocument | undefined;
  try {
    await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
    await symlink(fileURLToPath(new URL("../../core", import.meta.url)), join(root, "node_modules/@oxitone/core"), "dir");
    const entry = join(root, "song.ts");
    const source = "import { Project, createAutomationNamespace } from '@oxitone/core';\nconst project = new Project(); const a = createAutomationNamespace(); const shared = a.sine({ periodBeats: 4 });\nconst first = project.addChannel(); const second = project.addChannel(); first.automate('level', shared); second.automate('level', shared); export default project;\n";
    await writeFile(entry, source); document = await ProjectDocument.open({ entry });
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    const definition = document.view.automationSites.find(site => site.label === "shared" && site.scope === "definition")!;
    expect(definition.lanes).toHaveLength(2);
    const points = [{ beat: 0, value: 0.2 }, { beat: 1, value: 0.8 }];
    await document.editAutomationRange(0, definition.handle, { start: 2, end: 3, points });
    expect(document.frame!.snapshot.automation.every(lane => lane.source.kind === "replaceRange")).toBe(true);
    await document.undo(1);
    let site = document.view.automationSites.find(site => site.scope === "reference" && site.label === "first.automate")!;
    const lane = site.lanes[0]!;
    const other = document.frame!.snapshot.automation.find(item => item.id !== lane)!;
    for (let i = 0; i < 4; i++) {
      await document.editAutomationRange(document.view.revision, site.handle, { start: 2, end: 3, points: [{ beat: 0, value: 0.2 + i / 10 }] }, lane);
      site = document.view.automationSites.find(site => site.scope === "reference" && site.label === "first.automate")!;
      expect(document.frame!.snapshot.automation.find(item => item.id !== lane)).toEqual(other);
    }
    const shaped = documentRequestSchema.parse({ documentProtocolVersion: "2.0", sessionId: document.view.sessionId,
      requestId: "stream/gpui/1", baseRevision: document.view.revision, operation: { kind: "automationRange", site: site.handle, lane,
        edit: { start: 2, end: 3, points: [
          { beat: 0, value: 0.2, curve: { kind: "bezier", out: [1 / 3, 0.6], in: [2 / 3, 0.9] } },
          { beat: 0.5, value: 0.8, curve: { kind: "step" } }, { beat: 1, value: 0.5 },
        ] } } });
    if (shaped.operation.kind !== "automationRange") throw new Error("unexpected operation");
    await document.editAutomationRange(document.view.revision, site.handle, shaped.operation.edit, lane);
    const shapedSource = document.frame!.snapshot.automation.find(item => item.id === lane)!.source;
    expect(shapedSource).toMatchObject({ kind: "replaceRange", replacement: { kind: "curve", points: [
      { value: 0.2, curve: { kind: "bezier", out: [1 / 3, 0.6], in: [2 / 3, 0.9] } },
      { value: 0.8, curve: { kind: "step" } }, { value: 0.5 },
    ] } });
    expect(document.frame!.snapshot.automation.find(item => item.id !== lane)).toEqual(other);
    await document.undo(document.view.revision);
    expect(document.frame!.snapshot.automation[0]!.source).toMatchObject({ kind: "replaceRange", replacement: { points: [{ value: 0.5 }] } });
    await document.redo(document.view.revision);
    expect(document.frame!.snapshot.automation[0]!.source).toEqual(shapedSource);
    const text = document.view.files.find(file => file.path === entry)!.text;
    expect(text.match(/replaceRange\(/g)).toHaveLength(1); expect(text).toContain("a.sine({ periodBeats: 4 })");
    expect(await readFile(entry, "utf8")).toBe(source);
    await document.save(document.view.revision);
    document.close(); document = await ProjectDocument.open({ entry });
    expect(document.view.status).toBe("ready");
    expect(document.frame!.snapshot.automation[0]!.source).toEqual(shapedSource);
    expect(document.frame!.snapshot.automation[1]!.source).toEqual(other.source);
  } finally { document?.close(); await rm(root, { recursive: true, force: true }); }
});

it("edits a unique playlist automation clip in clip-local time and rejects shared clips", async () => {
  const root = await mkdtemp(join(tmpdir(), "oxitone-automation-clip-document-")); let document: ProjectDocument | undefined;
  try {
    await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
    await symlink(fileURLToPath(new URL("../../core", import.meta.url)), join(root, "node_modules/@oxitone/core"), "dir");
    const entry = join(root, "song.ts");
    const source = "import { Project, createAutomationNamespace } from '@oxitone/core';\nconst project = new Project(); const channel = project.addChannel(); const track = project.addTrack('Automation'); const lane = project.addAutomationLane({ entityId: channel.id, parameterId: 'level' }, createAutomationNamespace().line(0.1, 0.9, 4), { playback: 'playlist' }); project.createAutomationClip(lane, track, 8, 4); export default project;\n";
    await writeFile(entry, source); document = await ProjectDocument.open({ entry });
    const site = document.view.automationSites.find(item => item.clips.length === 1)!;
    const clip = site.clips[0]!;
    await document.editAutomationRange(document.view.revision, site.handle, { start: 9, end: 10, points: [{ beat: 0, value: 0.2 }, { beat: 1, value: 0.8 }] }, undefined, clip);
    const lane = document.frame!.snapshot.automation[0]!;
    expect(lane.source).toMatchObject({ kind: "replaceRange", startBeat: { numerator: 1 }, endBeat: { numerator: 2 } });
    expect(document.view.automationSites[0]!.clips).toEqual([clip]);
  } finally { document?.close(); await rm(root, { recursive: true, force: true }); }
});
