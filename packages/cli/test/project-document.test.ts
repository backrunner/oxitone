import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, expect, it } from "vitest";
import { ProjectDocument } from "../src/source/index.js";

const cleanup: (() => Promise<void>)[] = [];
afterEach(async () => {
  for (const close of cleanup.splice(0).reverse()) await close();
});
async function project(entryText?: string) {
  const root = await mkdtemp(join(tmpdir(), "oxitone-full-document-"));
  cleanup.push(() => rm(root, { force: true, recursive: true }));
  await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
  await symlink(fileURLToPath(new URL("../../core", import.meta.url)), join(root, "node_modules/@oxitone/core"), "dir");
  const entry = join(root, "song.ts"),
    phrase = join(root, "phrase.ts");
  await writeFile(phrase, "import { chord } from '@oxitone/core'; export const phrase = chord(60, 'major');\n");
  await writeFile(
    entry,
    entryText ??
      "import { Project } from '@oxitone/core'; import { phrase } from './phrase.js';\nconst project = new Project({ name: 'Song' }); const track = project.addTrack('Lead'); track.pattern(phrase).at({ bar: 1 }); track.pattern(phrase).at({ bar: 2 }); export default project;\n",
  );
  const document = await ProjectDocument.open({ entry });
  cleanup.push(async () => {
    document.close();
  });
  expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
  return { root, entry, phrase, document };
}
it("automatically maps real project Patterns across modules, edits shared definitions and saves all dirty TS", async () => {
  const { document, entry, phrase } = await project();
  const site = document.view.sites.find((site) => site.label === "phrase" && site.scope === "definition")!;
  expect(site.references).toBe(2);
  expect(site.outputs).toHaveLength(3);
  const initial = await readFile(phrase, "utf8");
  await document.edit(0, site.handle, [{ select: { degree: 2 }, set: { pitch: 65 } }]);
  expect(document.frame?.snapshot.patterns[0]?.notes[1]?.pitch).toBe(65);
  expect(await readFile(phrase, "utf8")).toBe(initial);
  await document.changeCode(1, entry, (await readFile(entry, "utf8")).replace("'Song'", "'Edited'"));
  expect(document.frame?.snapshot.name).toBe("Edited");
  await document.save(2);
  expect(await readFile(phrase, "utf8")).toContain(".edit(");
  const reopened = await ProjectDocument.open({ entry });
  try {
    expect(reopened.frame?.snapshot.patterns[0]?.notes[1]?.pitch).toBe(65);
    expect(reopened.frame?.snapshot.name).toBe("Edited");
  } finally {
    reopened.close();
  }
  await document.undo(2);
  await document.undo(3);
  expect(document.frame?.snapshot.patterns[0]?.notes[1]?.pitch).toBe(64);
});
it("rejects a use-site edit that affects only one shared placement when the requested scope is the definition", async () => {
  const { document } = await project();
  const site = document.view.sites.find((site) => site.label === "track.pattern")!;
  const before = document.view.files;
  await expect(document.edit(0, site.handle, [{ select: { degree: 1 }, set: { pitch: 61 } }])).rejects.toMatchObject({
    code: "EditScopeConflict",
  });
  expect(document.view.files).toEqual(before);
  expect(document.view.revision).toBe(0);
});
it("rejects changed non-note project fields and keeps invalid code with the last accepted graph", async () => {
  const { document, entry } = await project(
    "import { Project, chord } from '@oxitone/core'; const phrase = chord(60, 'major'); const project = new Project({ name: String(phrase.notes[0].pitch) }); project.addTrack('Lead').pattern(phrase).at({ bar: 1 }); export default project;",
  );
  const site = document.view.sites.find((site) => site.label === "phrase")!;
  await expect(document.edit(0, site.handle, [{ select: { degree: 1 }, set: { pitch: 61 } }])).rejects.toMatchObject({
    code: "EditScopeConflict",
  });
  const frame = document.frame;
  await document.changeCode(0, entry, "export default (");
  expect(document.view.status).toBe("invalid");
  expect(document.frame).toEqual(frame);
  await expect(document.save(1)).rejects.toMatchObject({ code: "DraftInvalid" });
  await document.undo(1);
  expect(document.view.status).toBe("ready");
});

it("edits only the selected placement and reviews materialization before any buffer or disk changes", async () => {
  const { document, entry, phrase } = await project();
  const site = document.view.sites.find((site) => site.scope === "reference" && site.placements.length === 1)!;
  const placement = site.placements[0]!;
  await document.edit(0, site.handle, [{ select: { degree: 1 }, set: { pitch: 61 } }], placement);
  const snapshot = document.frame!.snapshot;
  expect(
    snapshot.patterns.find(
      (pattern) => pattern.id === snapshot.patternClips.find((clip) => clip.id === placement)!.patternId,
    )!.notes[0]!.pitch,
  ).toBe(61);
  expect(
    snapshot.patterns.find(
      (pattern) => pattern.id === snapshot.patternClips.find((clip) => clip.id !== placement)!.patternId,
    )!.notes[0]!.pitch,
  ).toBe(60);
  await document.undo(1);
  const current = document.view.sites.find((site) => site.scope === "reference" && site.placements[0] === placement)!;
  const before = document.view.files;
  await document.planMaterialize(2, current.handle, [{ select: { degree: 2 }, remove: true }], placement);
  const review = document.view.materialization!;
  expect(review).toMatchObject({ beforeNotes: 3, afterNotes: 2, affectedClips: [placement] });
  expect(review.afterText).toContain("new Pattern(");
  expect(document.view.files).toEqual(before);
  expect(await readFile(entry, "utf8")).toBe(review.beforeText);
  await document.confirmMaterialize(2, review.planId);
  await document.save(3);
  expect(await readFile(entry, "utf8")).toBe(review.afterText);
  expect(await readFile(phrase, "utf8")).toBe(before.find((file) => file.path === phrase)!.text);
  await expect(document.confirmMaterialize(3, review.planId)).rejects.toMatchObject({ code: "SourceChanged" });
  const reopened = await ProjectDocument.open({ entry });
  try {
    expect(reopened.frame!.snapshot.patterns.map((pattern) => pattern.notes.length).sort()).toEqual([2, 3]);
  } finally {
    reopened.close();
  }
});

it("keeps conflicts across code changes and Undo, rejects stale disk resolution and resolves the exact displayed disk", async () => {
  const { document, entry } = await project();
  const original = await readFile(entry, "utf8");
  await document.changeCode(0, entry, original.replace("'Song'", "'Draft'"));
  const external = original.replace("'Song'", "'Disk'");
  await writeFile(entry, external);
  await document.synchronizeDisk();
  expect(document.view.status).toBe("conflict");
  const conflict = document.view.conflicts[0]!;
  await document.changeCode(document.view.revision, entry, original.replace("'Song'", "'New draft'"));
  expect(document.view.status).toBe("conflict");
  await document.undo(document.view.revision);
  expect(document.view.status).toBe("conflict");
  await expect(document.save(document.view.revision)).rejects.toMatchObject({ code: "DraftInvalid" });
  await writeFile(entry, external + "\n");
  await expect(
    document.resolveDiskConflict(document.view.revision, entry, conflict.diskHash, "keep-draft"),
  ).rejects.toMatchObject({ code: "SourceChanged" });
  await document.synchronizeDisk();
  await document.resolveDiskConflict(document.view.revision, entry, document.view.conflicts[0]!.diskHash, "keep-draft");
  expect(document.view.status).toBe("ready");
  expect(document.frame!.snapshot.name).toBe("Draft");
  await document.save(document.view.revision);
  expect(await readFile(entry, "utf8")).toContain("'Draft'");
});

it("automatically merges disjoint draft and disk edits and rejects overlapping ones", async () => {
  const { document, entry } = await project();
  const original = await readFile(entry, "utf8");
  const draft = original.replace("'Song'", "'Draft'");
  const disk = `${original}\n// external annotation\n`;
  await document.changeCode(0, entry, draft);
  await writeFile(entry, disk);
  await document.synchronizeDisk();
  await document.resolveDiskConflict(document.view.revision, entry, document.view.conflicts[0]!.diskHash, "merge");
  expect(document.view.status).toBe("ready");
  expect(document.view.files.find((file) => file.path === entry)?.text).toContain("'Draft'");
  expect(document.view.files.find((file) => file.path === entry)?.text).toContain("external annotation");

  const overlapping = original.replace("'Song'", "'Draft'");
  await document.changeCode(document.view.revision, entry, overlapping);
  await writeFile(entry, original.replace("'Song'", "'Other'"));
  await document.synchronizeDisk();
  await expect(
    document.resolveDiskConflict(document.view.revision, entry, document.view.conflicts[0]!.diskHash, "merge"),
  ).rejects.toMatchObject({ code: "EditScopeConflict" });
  expect(document.view.status).toBe("conflict");
});

it("creates a new owned TypeScript source file through the journal and reopens it", async () => {
  const { document, root } = await project();
  const created = join(root, "helpers.ts");
  await document.createFile(document.view.revision, created, "export const helper = 42;\n");
  expect(document.view.files.some((file) => file.path === created)).toBe(true);
  await document.save(document.view.revision);
  expect(await readFile(created, "utf8")).toBe("export const helper = 42;\n");
  const reopened = await ProjectDocument.open({ entry: join(root, "song.ts") });
  try {
    expect(reopened.view.files.some((file) => file.path === created)).toBe(true);
  } finally {
    reopened.close();
  }
});

it("rejects oversized code before mutating the draft, history or revision", async () => {
  const { document, entry } = await project();
  const before = document.view;
  await expect(document.changeCode(0, entry, "x".repeat(8 * 1024 * 1024 + 1))).rejects.toMatchObject({
    code: "BudgetExceeded",
  });
  expect(document.view).toEqual(before);
  await document.undo(0);
  expect(document.view).toEqual(before);
});
