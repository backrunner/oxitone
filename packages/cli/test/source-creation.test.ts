import { execFile } from "node:child_process";
import { createRequire } from "node:module";
import { link, mkdir, mkdtemp, readFile, rm, stat, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { afterEach, expect, it } from "vitest";
import { ProjectDocument, SourceOwnership, SourceSaveStore } from "../src/source/index.js";
import { prepareImages, publishImage, writeJournal } from "../src/source/save/save-journal.js";
import { stagingPath } from "../src/source/save/save-staging.js";
import { sourceHash } from "../src/source/syntax/program.js";

const cleanup: (() => Promise<void>)[] = [];
afterEach(async () => { for (const close of cleanup.splice(0).reverse()) await close(); });
async function fixture() {
  const root = await mkdtemp(join(tmpdir(), "oxitone-source-creation-"));
  cleanup.push(() => rm(root, { recursive: true, force: true }));
  await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
  await symlink(fileURLToPath(new URL("../../core", import.meta.url)), join(root, "node_modules/@oxitone/core"), "dir");
  const entry = join(root, "song.ts"), created = join(root, "phrase.ts");
  const text = "import { Project } from '@oxitone/core'; export default new Project({ name: 'Original' });\n";
  await writeFile(entry, text);
  const document = await ProjectDocument.open({ entry }); cleanup.push(async () => { document.close(); });
  expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
  return { root, entry, created, document, text, directory: join(root, ".oxitone-source-save") };
}

it.each(["ts", "mts"])("imports and re-exports unsaved .%s modules and edits their notes before Save", async suffix => {
  const { root, entry, document } = await fixture();
  const phrase = join(root, `phrase.${suffix}`), barrel = join(root, `barrel.${suffix}`), js = suffix === "ts" ? "js" : "mjs";
  await document.createFile(document.view.revision, `phrase.${suffix}`, "import { chord } from '@oxitone/core'; export const phrase = chord(60, 'major');\n");
  await document.createFile(document.view.revision, barrel, `export { phrase as melody } from './phrase.${js}';\n`);
  await document.changeCode(document.view.revision, entry, `import { Project } from '@oxitone/core'; import { melody } from './barrel.${js}';
const project = new Project({ name: 'Draft modules' }); project.addTrack('Lead').pattern(melody).at({ bar: 1 }); export default project;`);
  expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
  await expect(stat(phrase)).rejects.toMatchObject({ code: "ENOENT" });
  const site = document.view.sites.find(site => site.fileName === phrase && site.label === "phrase" && site.scope === "definition")!;
  expect(site).toBeDefined();
  await document.edit(document.view.revision, site.handle, [{ select: { degree: 2 }, set: { pitch: 65 } }]);
  expect(document.frame!.snapshot.patterns[0]!.notes[1]!.pitch).toBe(65);
  await document.save(document.view.revision);
  expect(document.view.modified).toBe(false);
  expect(await readFile(phrase, "utf8")).toContain(".edit(");
  const revision = document.view.revision;
  await document.synchronizeDisk(); expect(document.view.revision).toBe(revision);
  const reopened = await ProjectDocument.open({ entry });
  try { expect(reopened.view.status).toBe("ready"); expect(reopened.frame!.snapshot.patterns).toEqual(document.frame!.snapshot.patterns); }
  finally { reopened.close(); }
});

it("persists Undo/Redo of new empty files, including deletion after Save and recreation", async () => {
  const { created, document } = await fixture();
  await document.createFile(0, created, "");
  expect(document.view.modified).toBe(true);
  await document.undo(1); expect(document.view.modified).toBe(false);
  await document.redo(2); await document.save(3);
  expect(await readFile(created, "utf8")).toBe("");
  await document.undo(3); expect(document.view.modified).toBe(true);
  await document.synchronizeDisk(); expect(document.view.files.some(file => file.path === created)).toBe(false);
  await document.save(4); await expect(stat(created)).rejects.toMatchObject({ code: "ENOENT" });
  expect(document.view.modified).toBe(false);
  await document.redo(4); await document.save(5);
  expect(await readFile(created, "utf8")).toBe("");
});

it("uses the same .js-to-TypeScript resolution before and after saving a module beside existing JavaScript", async () => {
  const { root, entry, document } = await fixture();
  await writeFile(join(root, "phrase.js"), "export const name = 'JavaScript';");
  await document.createFile(0, "phrase.ts", "export const name = 'TypeScript';");
  await document.changeCode(1, entry, "import { Project } from '@oxitone/core'; import { name } from './phrase.js'; export default new Project({ name });");
  expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
  expect(document.frame!.snapshot.name).toBe("TypeScript");
  await document.save(2);
  const reopened = await ProjectDocument.open({ entry });
  try { expect(reopened.frame!.snapshot.name).toBe("TypeScript"); }
  finally { reopened.close(); }
});

it("restores JavaScript fallback when a saved TypeScript shadow is undone", async () => {
  const { root, entry, document } = await fixture();
  await writeFile(join(root, "phrase.js"), "export const name = 'JavaScript';");
  await document.changeCode(0, entry, "import { Project } from '@oxitone/core'; import { name } from './phrase.js'; export default new Project({ name });");
  await document.createFile(1, "phrase.ts", "export const name = 'TypeScript';"); await document.save(2);
  await document.undo(2);
  expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
  expect(document.frame!.snapshot.name).toBe("JavaScript");
  await document.save(3);
  const reopened = await ProjectDocument.open({ entry });
  try { expect(reopened.frame!.snapshot.name).toBe("JavaScript"); }
  finally { reopened.close(); }
});

it("preserves extensionless directory index precedence across Save", async () => {
  const { root, entry, document } = await fixture();
  await mkdir(join(root, "phrases"));
  await writeFile(join(root, "phrases/index.js"), "export const name = 'JavaScript';");
  await document.createFile(0, "phrases/index.ts", "export const name = 'TypeScript';");
  await document.changeCode(1, entry, "import { Project } from '@oxitone/core'; import { name } from './phrases'; export default new Project({ name });");
  expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
  expect(document.frame!.snapshot.name).toBe("TypeScript");
  await document.save(2);
  const reopened = await ProjectDocument.open({ entry });
  try { expect(reopened.frame!.snapshot.name).toBe("TypeScript"); }
  finally { reopened.close(); }
});

it("rejects existing files and stale/oversized creation without changing the revision or ownership", async () => {
  const { root, created, entry, text, document } = await fixture();
  const before = document.view;
  await expect(document.createFile(0, created, "x".repeat(8 * 1024 * 1024 + 1))).rejects.toMatchObject({ code: "BudgetExceeded" });
  expect(document.view).toEqual(before);
  const pending = document.createFile(0, created, "");
  const stale = expect(pending).rejects.toMatchObject({ code: "SourceChanged" });
  await document.changeCode(0, entry, text + "// concurrent\n"); await stale;
  expect(document.view.files.some(file => file.path === created)).toBe(false);
  await writeFile(created, "");
  await expect(document.createFile(1, created, "// new")).rejects.toMatchObject({ code: "EditNotRepresentable" });
  await mkdir(join(root, "nested")); await writeFile(join(root, "nested/package.json"), "{}");
  await expect(document.createFile(1, join(root, "nested/new.ts"), "")).rejects.toMatchObject({ code: "EditNotRepresentable" });
  await rm(created); await document.createFile(1, created, ""); expect(document.view.revision).toBe(2);
});

it("keeps an externally created empty file intact and exposes its conflict with a new draft", async () => {
  const { created, document } = await fixture();
  await document.createFile(0, created, "export const n = 1;\n"); await writeFile(created, "");
  await expect(document.save(1)).rejects.toMatchObject({ code: "SourceChanged" });
  expect(await readFile(created, "utf8")).toBe("");
  await document.synchronizeDisk(); expect(document.view.status).toBe("conflict");
  await document.resolveDiskConflict(document.view.revision, created, document.view.conflicts[0]!.diskHash, "keep-draft");
  await document.save(document.view.revision); expect(await readFile(created, "utf8")).toContain("n = 1");
});

it.each(["prepared", "committed"] as const)("recovers %s creation with a fresh owner, including interrupted hard-link publication", async phase => {
  const { root, entry, created, directory } = await fixture();
  const ownership = await SourceOwnership.open([root], [entry]); await ownership.enroll(created);
  const images = await prepareImages([{ path: created, baselineHash: null, text: "" }], ownership);
  await writeJournal(directory, { version: 2, phase, files: images });
  const temporary = stagingPath(images[0]!.realPath, images[0]!.stagingId!);
  await writeFile(temporary, ""); await link(temporary, created);
  await SourceSaveStore.open(root, await SourceOwnership.open([root], []));
  await expect(stat(temporary)).rejects.toMatchObject({ code: "ENOENT" });
  if (phase === "prepared") await expect(stat(created)).rejects.toMatchObject({ code: "ENOENT" });
  else { expect(await readFile(created, "utf8")).toBe(""); expect((await stat(created)).nlink).toBe(1); }
});

it("preserves existing empty files in recovery and loads the recovered preimage", async () => {
  const { root, entry, created, directory, text } = await fixture();
  await writeFile(created, "");
  const ownership = await SourceOwnership.open([root], [entry, created]);
  const images = await prepareImages([{ path: created, baselineHash: sourceHash(""), text: "// filled" },
    { path: entry, baselineHash: sourceHash(text), text: text.replace("Original", "Partial") }], ownership);
  await writeJournal(directory, { version: 2, phase: "prepared", files: images });
  for (const image of images) await publishImage(image, image.after, image.beforeHash);
  const reopened = await ProjectDocument.open({ entry });
  try { expect(await readFile(created, "utf8")).toBe(""); expect(reopened.frame!.snapshot.name).toBe("Original"); expect(reopened.view.modified).toBe(false); }
  finally { reopened.close(); }
});

it("recovers a real killed creation before discovering and executing project buffers", async () => {
  const { root, entry, created, text } = await fixture();
  const files = [{ path: created, baselineHash: null, text: "// new" }, { path: entry, baselineHash: sourceHash(text), text: text.replace("Original", "Partial") }];
  const worker = join(root, "crash.mjs");
  await writeFile(worker, `import { SourceOwnership, SourceSaveStore } from ${JSON.stringify(new URL("../src/source/index.ts", import.meta.url).href)};
const files = ${JSON.stringify(files)};
const ownership = await SourceOwnership.open([${JSON.stringify(root)}], [files[1].path]); await ownership.enroll(files[0].path);
const store = await SourceSaveStore.open(${JSON.stringify(root)}, ownership); let calls = 0;
await store.save({ files, reads: [], assertCurrent() { if (++calls === 4) process.kill(process.pid, 'SIGKILL'); } });`);
  await expect(promisify(execFile)(process.execPath, ["--import", createRequire(import.meta.url).resolve("tsx"), worker])).rejects.toMatchObject({ signal: "SIGKILL" });
  expect(await readFile(created, "utf8")).toBe("// new");
  const reopened = await ProjectDocument.open({ entry });
  try { expect(reopened.frame!.snapshot.name).toBe("Original"); expect(reopened.view.files.some(file => file.path === created)).toBe(false); }
  finally { reopened.close(); }
  await expect(stat(created)).rejects.toMatchObject({ code: "ENOENT" });
});
