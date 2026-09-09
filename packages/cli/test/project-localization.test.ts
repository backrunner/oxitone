import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";
import { ProjectDocument } from "../src/source/index.js";

it("localizes an npm chord/arp factory at one placement, preserves evaluation and imports, and rejects dependency drift after review", async () => {
  const root = await mkdtemp(join(tmpdir(), "oxitone-project-localize-"));
  let document: ProjectDocument | undefined;
  try {
    await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
    await symlink(fileURLToPath(new URL("../../core", import.meta.url)), join(root, "node_modules/@oxitone/core"), "dir");
    const pkg = join(root, "node_modules/arrangement-fixture"); await mkdir(pkg);
    const dependency = join(pkg, "index.js"), manifest = join(pkg, "package.json");
    const factory = "import { chord, arp } from '@oxitone/core'; export let calls = 0; export function phrase() { calls++; return arp(chord(60, 'major'), 'upDown', 0.25).repeat(2); }";
    const metadata = '{"name":"arrangement-fixture","version":"1.0.0","type":"module","exports":"./index.js"}';
    await writeFile(manifest, metadata); await writeFile(dependency, factory);
    const entry = join(root, "song.ts");
    const source = "import type { Pattern } from '@oxitone/core'; import { Project } from '@oxitone/core';\nimport { phrase as chorus, calls } from 'arrangement-fixture';\nexport default async function make(Pattern = 123) { const project = new Project(); const track = project.addTrack('Lead'); track.pattern(chorus()).at({ bar: 1 }); track.pattern(chorus()).at({ bar: 2 }); project.addTrack(String(calls)); return project; }\n";
    await writeFile(entry, source); document = await ProjectDocument.open({ entry });
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    const site = document.view.sites.find(site => site.expression === "chorus()" && site.placements.length === 1)!;
    const clip = site.placements[0]!;
    await document.edit(0, site.handle, [{ select: { iteration: 1, note: { step: 2 } }, set: { start: 20, duration: .5 } }], clip);
    expect(document.frame!.snapshot.patterns.some(p => p.lengthBeats.numerator === 21)).toBe(true);
    expect(await readFile(dependency, "utf8")).toBe(factory);
    await document.undo(document.view.revision);
    const initialRevision = document.view.revision;
    const initialSite = document.view.sites.find(site => site.expression === "chorus()" && site.placements[0] === clip)!;
    await document.planMaterialize(initialRevision, initialSite.handle, [{ select: { iteration: 1, note: { step: 2 } }, remove: true }], clip);
    const stale = document.view.materialization!;
    expect(stale.afterText).toContain("Pattern as LocalPattern1");
    expect(stale.retainsOriginalEvaluation).toBe(true); expect(stale.afterNotes).toBe(7);
    expect(await readFile(entry, "utf8")).toBe(source);
    await writeFile(dependency, factory + "\n");
    await expect(document.confirmMaterialize(initialRevision, stale.planId)).rejects.toMatchObject({ code: "SourceChanged" });
    await document.synchronizeDisk();
    expect(document.view.materialization).toBeUndefined();
    const current = document.view.sites.find(site => site.expression === "chorus()" && site.placements[0] === clip)!;
    await document.planMaterialize(document.view.revision, current.handle, [{ select: { iteration: 1, note: { step: 2 } }, remove: true }], clip);
    const plan = document.view.materialization!;
    document.cancelMaterialize(document.view.revision, plan.planId);
    await expect(document.confirmMaterialize(document.view.revision, plan.planId)).rejects.toMatchObject({ code: "SourceChanged" });
    await document.planMaterialize(document.view.revision, current.handle, [{ select: { iteration: 1, note: { step: 2 } }, remove: true }], clip);
    await document.confirmMaterialize(document.view.revision, document.view.materialization!.planId);
    await document.save(document.view.revision);
    const snapshot = document.frame!.snapshot;
    expect(snapshot.tracks[1]!.name).toBe("2"); // Both factories still run exactly once.
    expect(snapshot.patterns.map(pattern => pattern.notes.length).sort()).toEqual([7, 8]);
    expect(await readFile(dependency, "utf8")).toBe(factory + "\n"); expect(await readFile(manifest, "utf8")).toBe(metadata);
    document.close(); document = await ProjectDocument.open({ entry });
    expect(document.view.status).toBe("ready"); expect(document.frame!.snapshot.patterns.map(pattern => pattern.notes.length).sort()).toEqual([7, 8]);
    expect(await readFile(entry, "utf8")).not.toMatch(/sessionId|requestId|__oxitone_project_|UUID/);
  } finally { document?.close(); await rm(root, { recursive: true, force: true }); }
});

it("keeps shared exports while localizing one reference, then runs the detached notes without the removed dependency", async () => {
  const root = await mkdtemp(join(tmpdir(), "oxitone-project-detached-"));
  let document: ProjectDocument | undefined;
  try {
    await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
    await symlink(fileURLToPath(new URL("../../core", import.meta.url)), join(root, "node_modules/@oxitone/core"), "dir");
    const pkg = join(root, "node_modules/arrangement-fixture"); await mkdir(pkg);
    const factory = "import { chord, arp } from '@oxitone/core'; export function makePhrase() { return arp(chord(60, 'major'), 'upDown', 0.25).repeat(2); }";
    await writeFile(join(pkg, "package.json"), '{"name":"arrangement-fixture","type":"module","exports":"./index.js"}');
    await writeFile(join(pkg, "index.js"), factory);
    const entry = join(root, "song.ts");
    const text = "import type { Pattern } from '@oxitone/core';\nimport { Project } from '@oxitone/core';\nimport { makePhrase as chorus } from 'arrangement-fixture';\nconst phrase = chorus();\nexport const shared = phrase;\nconst project = new Project(); const track = project.addTrack('Lead');\ntrack.pattern(phrase).at({ bar: 1 });\ntrack.pattern(shared).at({ bar: 2 });\nexport default project;\n";
    await writeFile(entry, text); document = await ProjectDocument.open({ entry });
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    const site = document.view.sites.find(site => site.scope === "reference" && site.expression === "phrase" && site.placements.length === 1)!;
    await document.planMaterialize(0, site.handle, [{ select: { iteration: 1, note: { step: 1 } }, remove: true }], site.placements[0]);
    const review = document.view.materialization!;
    expect(review.retainsOriginalEvaluation).toBe(false); expect(review.afterNotes).toBe(7);
    expect(review.afterText).toContain("Pattern as LocalPattern1");
    expect(review.afterText).toContain("export const shared = phrase;");
    await document.confirmMaterialize(0, review.planId); await document.save(1);
    expect(document.frame!.snapshot.patterns.map(pattern => pattern.notes.length).sort()).toEqual([7, 8]);
    expect(await readFile(join(pkg, "index.js"), "utf8")).toBe(factory);
    // Removing unused imports/generators is an explicit subsequent author edit.
    const independent = review.afterText.replace("import { makePhrase as chorus } from 'arrangement-fixture';\n", "")
      .replace("const phrase = chorus();\n", "").replace("export const shared = phrase;\n", "")
      .replace("track.pattern(shared).at({ bar: 2 });\n", "");
    await rm(pkg, { recursive: true });
    await document.changeCode(1, entry, independent);
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    await document.save(2); document.close(); document = await ProjectDocument.open({ entry });
    expect(document.view.status).toBe("ready");
    expect(document.frame!.snapshot.patterns).toHaveLength(1);
    expect(document.frame!.snapshot.patterns[0]!.notes).toHaveLength(7);
  } finally { document?.close(); await rm(root, { recursive: true, force: true }); }
});
