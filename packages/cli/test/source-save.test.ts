import { chmod, mkdir, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, expect, it } from "vitest";
import { execFile } from "node:child_process";
import { createRequire } from "node:module";
import { promisify } from "node:util";
import { SourceOwnership, SourceSaveStore, captureSourceReads } from "../src/source/index.js";
import { prepareImages, writeJournal } from "../src/source/save/save-journal.js";
import { sourceHash } from "../src/source/syntax/program.js";

const roots: string[] = [];
afterEach(async () => { for (const root of roots.splice(0)) await rm(root, { recursive: true, force: true }); });
async function fixture() {
  const root = await mkdtemp(join(tmpdir(), "oxitone-save-test-")); roots.push(root);
  const a = join(root, "a.ts"), b = join(root, "b.ts");
  await writeFile(a, "export const a = 1;\n"); await writeFile(b, "export const b = 1;\n");
  const ownership = await SourceOwnership.open([root], [a, b]);
  const store = await SourceSaveStore.open(root, ownership);
  const files = await Promise.all([a, b].map(async path => ({ path, baselineHash: sourceHash(await readFile(path, "utf8")), text: "export default 2;\n" })));
  return { root, a, b, ownership, store, files, directory: join(root, ".oxitone-source-save") };
}

it("publishes multiple enrolled TS files, preserves modes and detects stale dependencies before publication", async () => {
  const { a, b, store, files, root } = await fixture(); await chmod(a, 0o640);
  const dependency = join(root, "lock.json"); await writeFile(dependency, "1");
  const reads = await captureSourceReads([a, b, dependency]);
  await writeFile(dependency, "2");
  await expect(store.save({ files, reads })).rejects.toMatchObject({ code: "SourceChanged" });
  expect(await readFile(a, "utf8")).toContain("a = 1");
  await store.save({ files, reads: await captureSourceReads([a, b, dependency]) });
  expect(await readFile(a, "utf8")).toBe(files[0]!.text);
  expect(await readFile(b, "utf8")).toBe(files[1]!.text);
  expect((await stat(a)).mode & 0o777).toBe(0o640);
});

it("rolls back an interrupted partial publication before a loader can execute the source", async () => {
  const { root, a, b, directory, ownership, files } = await fixture();
  const images = await prepareImages(files, ownership);
  await writeJournal(directory, { version: 2, phase: "prepared", files: images });
  await writeFile(a, images[0]!.after!);
  await SourceSaveStore.open(root, ownership);
  expect(await readFile(a, "utf8")).toBe(images[0]!.before);
  expect(await readFile(b, "utf8")).toBe(images[1]!.before);
  await expect(stat(join(directory, "journal.json"))).rejects.toMatchObject({ code: "ENOENT" });
});

it("finishes a committed journal and preserves third-party edits as a recovery conflict", async () => {
  const { root, a, b, directory, ownership, files } = await fixture();
  const images = await prepareImages(files, ownership);
  await writeJournal(directory, { version: 2, phase: "committed", files: images });
  await writeFile(a, "// a third image\n");
  await expect(SourceSaveStore.open(root, ownership)).rejects.toMatchObject({ code: "SourceChanged" });
  expect(await readFile(a, "utf8")).toBe("// a third image\n");
  expect(await readFile(b, "utf8")).toBe(images[1]!.before);
  expect(await readFile(join(directory, "journal.json"), "utf8")).toContain("committed");
  await writeFile(a, images[0]!.before!);
  await SourceSaveStore.open(root, ownership);
  expect(await readFile(a, "utf8")).toBe(images[0]!.after);
  expect(await readFile(b, "utf8")).toBe(images[1]!.after);
});

it("rolls back a rejected revision during a multi-file save and excludes a second writer", async () => {
  const { a, b, directory, store, files } = await fixture();
  let calls = 0;
  await expect(store.save({ files, reads: [], assertCurrent() { if (++calls === 4) throw new Error("superseded"); } })).rejects.toThrow("superseded");
  expect(await readFile(a, "utf8")).toContain("a = 1");
  expect(await readFile(b, "utf8")).toContain("b = 1");
  const lock = join(directory, "lock"); await mkdir(lock);
  await writeFile(join(lock, "owner.json"), JSON.stringify({ pid: process.pid }));
  await expect(store.save({ files, reads: [] })).rejects.toMatchObject({ code: "SourceChanged" });
});

it("recovers a real SIGKILL between two renames and reclaims the dead process lock", async () => {
  const { root, a, b, ownership, files } = await fixture();
  const worker = join(root, "crash.mjs");
  const sourceModule = new URL("../src/source/index.ts", import.meta.url).href;
  await writeFile(worker, `import { SourceOwnership, SourceSaveStore } from ${JSON.stringify(sourceModule)};
const root = ${JSON.stringify(root)};
const files = ${JSON.stringify(files)};
const ownership = await SourceOwnership.open([root], files.map(file => file.path));
const store = await SourceSaveStore.open(root, ownership);
let calls = 0;
await store.save({ files, reads: [], assertCurrent() { if (++calls === 4) process.kill(process.pid, 'SIGKILL'); } });
`);
  const require = createRequire(import.meta.url);
  await expect(promisify(execFile)(process.execPath, ["--import", require.resolve("tsx"), worker])).rejects.toMatchObject({ signal: "SIGKILL" });
  expect(await readFile(a, "utf8")).toBe(files[0]!.text);
  expect(await readFile(b, "utf8")).toContain("b = 1");
  await SourceSaveStore.open(root, ownership);
  expect(await readFile(a, "utf8")).toContain("a = 1");
  expect(await readFile(b, "utf8")).toContain("b = 1");
});

it("refuses a save journal in dependencies and preserves non-UTF8 source bytes", async () => {
  const { root, a, ownership, store, files } = await fixture();
  const dependency = join(root, "node_modules/pkg"); await mkdir(dependency, { recursive: true });
  await expect(SourceSaveStore.open(dependency, ownership)).rejects.toMatchObject({ code: "EditNotRepresentable" });
  await expect(stat(join(dependency, ".oxitone-source-save"))).rejects.toMatchObject({ code: "ENOENT" });
  const bytes = Buffer.from([0xff, 0xfe]); await writeFile(a, bytes);
  await expect(store.save({ files, reads: [] })).rejects.toMatchObject({ code: "DraftInvalid" });
  expect(await readFile(a)).toEqual(bytes);
});

it("rejects an obsolete journal without rewriting source or discarding recovery images", async () => {
  const { root, a, directory, ownership, files } = await fixture();
  const images = await prepareImages(files, ownership);
  const journal = JSON.stringify({ version: 1, phase: "prepared", files: images });
  await writeFile(join(directory, "journal.json"), journal);
  await writeFile(a, images[0]!.after!);
  await expect(SourceSaveStore.open(root, ownership)).rejects.toMatchObject({ code: "SourceChanged" });
  expect(await readFile(a, "utf8")).toBe(images[0]!.after);
  expect(await readFile(join(directory, "journal.json"), "utf8")).toBe(journal);
});
