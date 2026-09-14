import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";
import { ProjectDocument } from "../src/source/index.js";

it("edits independent Pattern parts and moves reusable automation through Save, Undo and reopen", async () => {
  const root = await mkdtemp(join(tmpdir(), "oxitone-playlist-document-"));
  let document: ProjectDocument | undefined;
  try {
    await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
    await symlink(
      fileURLToPath(new URL("../../core", import.meta.url)),
      join(root, "node_modules/@oxitone/core"),
      "dir",
    );
    const entry = join(root, "song.ts");
    await writeFile(
      entry,
      [
        "import { Project, Pattern, chord, createAutomationNamespace } from '@oxitone/core';",
        "const project = new Project(); const a = project.addChannel(); const b = project.addChannel();",
        "const lead = chord(60, 'major', { lengthBeats: 4 }); const bass = chord(36, 'minor', { lengthBeats: 4 });",
        "const phrase = new Pattern({ lengthBeats: 4, parts: [{ channelId: a.id, pattern: lead }, { channelId: b.id, pattern: bass }] });",
        "project.addTrack('A').add(phrase).at({ bar: 1 }); project.addTrack('B').add(phrase).at({ bar: 3 });",
        "const motion = createAutomationNamespace().ramp({ periodBeats: 4 }); a.automate('level', motion);",
        "export default project;",
      ].join("\n"),
    );
    document = await ProjectDocument.open({ entry });
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    const site = document.view.sites.find((s) => s.label === "lead" && s.scope === "definition")!;
    expect(site.references).toBe(2);
    await document.edit(0, site.handle, [{ select: { degree: 1 }, set: { pitch: 61 } }]);
    expect(document.frame!.snapshot.patterns.find((p) => p.notes.some((n) => n.pitch === 36))!.notes[0]!.pitch).toBe(
      36,
    );
    const pattern = document.frame!.snapshot.patterns.find((p) => p.parts)!;
    const part = document.frame!.snapshot.patterns.find((p) => p.id === pattern.parts![0]!.patternId)!;
    expect(part.notes.some((n) => n.pitch === 61)).toBe(true);
    const lead = document.view.sites.find((s) => s.label === "lead" && s.scope === "definition")!;
    await document.edit(document.view.revision, lead.handle, [
      { select: { degree: 1 }, set: { start: 12, duration: 1 } },
    ]);
    expect(document.frame!.snapshot.patterns.find((p) => p.parts)!.lengthBeats.numerator).toBe(13);
    expect(
      document.frame!.snapshot.patterns.find((p) => p.notes.some((n) => n.pitch === 36))!.lengthBeats.numerator,
    ).toBe(4);
    await document.undo(document.view.revision);
    expect(document.frame!.snapshot.patterns.find((p) => p.parts)!.lengthBeats.numerator).toBe(4);
    await document.redo(document.view.revision);
    await document.arrange(document.view.revision, {
      action: "place",
      kind: "automation",
      resource: 0,
      track: 0,
      startBeat: 2,
      durationBeats: 4,
    });
    await document.arrange(document.view.revision, {
      action: "place",
      kind: "automation",
      resource: 0,
      track: 1,
      startBeat: 8,
      durationBeats: 4,
    });
    await document.arrange(document.view.revision, {
      action: "move",
      kind: "automation",
      resource: 0,
      clip: 0,
      track: 1,
      startBeat: 5,
    });
    expect(document.frame!.snapshot.automationClips!.some((c) => c.startBeat.numerator === 5)).toBe(true);
    await document.undo(document.view.revision);
    expect(document.frame!.snapshot.automationClips!.some((c) => c.startBeat.numerator === 2)).toBe(true);
    await document.redo(document.view.revision);
    await document.save(document.view.revision);
    const expected = document.frame!.snapshot;
    document.close();
    document = await ProjectDocument.open({ entry });
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    expect(document.frame!.snapshot.automationClips).toEqual(expected.automationClips);
    expect(document.frame!.snapshot.patterns).toEqual(expected.patterns);
    expect(await readFile(entry, "utf8")).not.toMatch(/instanceId|requestId|patternId|channelIds/);
    await document.arrange(document.view.revision, { action: "remove", kind: "automation", resource: 0, clip: 0 });
    await document.arrange(document.view.revision, { action: "remove", kind: "automation", resource: 0, clip: 0 });
    expect(document.frame!.snapshot.automation[0]!.playback).toBe("playlist");
    expect(document.frame!.snapshot.automationClips).toBeUndefined();
    await document.save(document.view.revision);
    document.close();
    document = await ProjectDocument.open({ entry });
    expect(document.frame!.snapshot.automation[0]!.playback).toBe("playlist");
  } finally {
    document?.close();
    await rm(root, { recursive: true, force: true });
  }
});
