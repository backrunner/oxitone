import { mkdir, mkdtemp, readFile, rename, rm, symlink, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, expect, it } from "vitest";
import { ProjectDocument } from "../src/source/index.js";
import { watchProjectDocument } from "../src/source/document-watch.js";
import { runEvaluationProcess } from "../src/source/evaluation-process.js";
import { until } from "./preview-helpers.js";

const cleanup: (() => Promise<void>)[] = [];
afterEach(async () => { for (const close of cleanup.splice(0).reverse()) await close(); });
const source = "import { Project, chord } from '@oxitone/core'; const phrase = chord(60, 'major'); const project = new Project(); project.addTrack('Lead').pattern(phrase).at({ bar: 1 }); export default project;\n";
async function fixture(text = source) {
  const root = await mkdtemp("/tmp/oxitone-project-session-");
  cleanup.push(() => rm(root, { recursive: true, force: true }));
  await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
  await symlink(fileURLToPath(new URL("../../core", import.meta.url)), join(root, "node_modules/@oxitone/core"), "dir");
  const entry = join(root, "song.ts"); await writeFile(entry, text);
  return { root, entry };
}
async function open(entry: string) {
  const document = await ProjectDocument.open({ entry }); cleanup.push(async () => document.close());
  expect(document.view.status, document.view.diagnostic?.message).toBe("ready"); return document;
}

it("captures async local factories with their original this, module URL, exports and call count", async () => {
  const text = "import { Project } from '@oxitone/core'; import { make } from './phrase.js'; export default async function () { await Promise.resolve(); const project = new Project(); project.addTrack('Lead').pattern(make()).at({ bar: 1 }); return project; }";
  const { root, entry } = await fixture(text), path = join(root, "phrase.ts");
  await writeFile(path, "import { chord } from '@oxitone/core'; const factory = { pitch: 60, make() { return chord(this.pitch, 'major'); } }; let calls = 0; export function make() { if (++calls !== 1) throw Error('ran twice'); if (!import.meta.url.endsWith('/phrase.ts')) throw Error('asset base lost'); const selected = factory.make(); return selected; }");
  const document = await open(entry);
  const site = document.view.sites.find(site => site.label === "selected")!;
  await document.edit(0, site.handle, [{ select: { degree: 1 }, set: { pitch: 61 } }]);
  await document.save(1);
  expect(await readFile(entry, "utf8")).toBe(text);
  expect(await readFile(path, "utf8")).toContain("factory.make()");
  const reopened = await open(entry); expect(reopened.frame!.snapshot.patterns[0]!.notes[0]!.pitch).toBe(61);
});

it("rejects a repeatedly executed boundary and contains failing subscribers", async () => {
  const { entry } = await fixture("import { Project, chord } from '@oxitone/core'; function make() { const selected = chord(60, 'major'); return selected; } const project = new Project(); const track = project.addTrack('Lead'); for (let bar = 1; bar < 3; bar++) track.pattern(make()).at({ bar }); export default project;");
  const document = await open(entry);
  document.subscribe(() => { throw new Error("broken observer"); });
  expect(document.frame!.snapshot.patternClips).toHaveLength(2);
  // Repeated executions are excluded from editable sites by the full-project projection.
  expect(document.view.sites.some(site => site.label === "selected")).toBe(false);
  const before = document.view;
  await expect(document.edit(0, "unavailable-repeated-boundary", [{ select: { degree: 1 }, set: { pitch: 70 } }])).rejects.toMatchObject({ code: "EditTargetMissing" });
  expect(document.view).toEqual(before);
  await document.changeCode(0, entry, source);
  expect(document.view.status).toBe("ready");
});

it("supersedes pending source and DAW evaluations without publishing them after close", async () => {
  const { root, entry } = await fixture(), marker = join(root, "entered");
  const document = await open(entry);
  const entered = async () => until(() => document.view.status === "building");
  const slow = document.changeCode(0, entry, source + "await new Promise(resolve => setTimeout(resolve, 5000));");
  const rejected = expect(slow).rejects.toMatchObject({ code: "SourceChanged" }); await entered();
  await document.changeCode(1, entry, source.replace("chord(60", "chord(72")); await rejected;
  expect(document.frame!.snapshot.patterns[0]!.notes[0]!.pitch).toBe(72);
  // This worker reaches a real barrier only when the proposed musical edit is executed.
  const gated = source + `if (phrase.notes[0].pitch === 61) { (await import('node:fs')).writeFileSync(${JSON.stringify(marker)}, 'entered'); await new Promise(resolve => setTimeout(resolve, 5000)); }`;
  await document.changeCode(2, entry, gated);
  const site = document.view.sites.find(site => site.label === "phrase")!;
  const candidate = document.edit(3, site.handle, [{ select: { degree: 1 }, set: { pitch: 61 } }]);
  const cancelled = expect(candidate).rejects.toMatchObject({ code: "SourceChanged" });
  for (let count = 0; ; count++) {
    try { await readFile(marker); break; } catch (error) { if (count > 250 || (error as NodeJS.ErrnoException).code !== "ENOENT") throw error; }
    await new Promise(resolve => setTimeout(resolve, 20));
  }
  await document.changeCode(3, entry, source.replace("chord(60", "chord(74")); await cancelled;
  expect(document.frame!.snapshot.patterns[0]!.notes[0]!.pitch).toBe(74);
  const closing = document.changeCode(4, entry, source + "await new Promise(resolve => setTimeout(resolve, 5000));");
  const closed = expect(closing).rejects.toMatchObject({ code: "SourceChanged" }); document.close(); await closed;
  expect(document.view.status).toBe("closed");
});

it("terminates runaway project workers and honors cancellation", async () => {
  const { root } = await fixture(), bundle = join(root, "runaway.mjs");
  await writeFile(bundle, "export default () => { while (true) {} };");
  await expect(runEvaluationProcess(bundle, "timeout", root, new AbortController().signal, 1000, "project-worker")).rejects.toMatchObject({ code: "BudgetExceeded" });
  const controller = new AbortController();
  const pending = runEvaluationProcess(bundle, "cancel", root, controller.signal, 10_000, "project-worker");
  const rejected = expect(pending).rejects.toMatchObject({ code: "SourceChanged" }); controller.abort(); await rejected;
});

it("watches atomic saves, resolves dirty conflicts and rebuilds changed imported source", async () => {
  const { root, entry } = await fixture();
  const phrase = join(root, "phrase.ts");
  await writeFile(phrase, "import { chord } from '@oxitone/core'; export const phrase = chord(60, 'major');");
  const text = source.replace("const phrase = chord(60, 'major');", "import { phrase } from './phrase.js';");
  await writeFile(entry, text); const document = await open(entry);
  const errors: unknown[] = [], stop = watchProjectDocument(document, entry, error => errors.push(error));
  cleanup.push(async () => stop());
  const external = text + "// atomic save\n";
  await writeFile(entry + ".tmp", external); await rename(entry + ".tmp", entry);
  await until(() => document.view.status === "ready" && document.view.files.find(file => file.path === entry)?.text === external);
  await document.changeCode(document.view.revision, entry, external + "// local\n");
  await writeFile(entry, text);
  await until(() => document.view.status === "conflict");
  const conflict = document.view.conflicts[0]!;
  await document.resolveDiskConflict(document.view.revision, entry, conflict.diskHash, "keep-draft");
  await document.save(document.view.revision);
  expect(await readFile(entry, "utf8")).toContain("// local");
  await writeFile(phrase, (await readFile(phrase, "utf8")).replace("chord(60", "chord(70"));
  await until(() => document.view.status === "ready" && document.frame!.snapshot.patterns[0]!.notes[0]!.pitch === 70);
  expect(errors).toEqual([]);
});
