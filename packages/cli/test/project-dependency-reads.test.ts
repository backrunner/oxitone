import { mkdir, mkdtemp, readFile, rm, stat, symlink, utimes, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, expect, it } from "vitest";
import { ProjectDocument } from "../src/source/index.js";
import { ProjectDocument as BuiltProjectDocument } from "../dist/source/index.js";

const cleanup: (() => Promise<void>)[] = [];
afterEach(async () => { for (const close of cleanup.splice(0).reverse()) await close(); });
const workers = [{ name: "source", Document: ProjectDocument }, { name: "built", Document: BuiltProjectDocument }];
async function fixture(Document: typeof ProjectDocument | typeof BuiltProjectDocument) {
  const root = await mkdtemp(join(tmpdir(), "oxitone-dependency-reads-"));
  cleanup.push(() => rm(root, { recursive: true, force: true }));
  await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
  await symlink(fileURLToPath(new URL("../../core", import.meta.url)), join(root, "node_modules/@oxitone/core"), "dir");
  const pkg = join(root, "node_modules/preset"); await mkdir(pkg);
  await writeFile(join(pkg, "package.json"), '{"name":"preset","version":"1.0.0","type":"module","exports":"./index.ts"}');
  const entry = join(root, "song.ts"), marker = join(root, "calls");
  await writeFile(entry, "import { Project } from '@oxitone/core'; import { rack } from 'preset'; const project = new Project(); const channel = project.addChannel({ effectChain: rack() }); export default project;");
  await writeFile(join(pkg, "index.ts"), `import { effect } from '@oxitone/core'; import { appendFileSync } from 'node:fs'; import { Setting } from './setting.js';
let calls = 0;
export function rack() { if (++calls !== 1) throw Error('factory executed twice'); appendFileSync(${JSON.stringify(marker)}, 'x'); return [effect('delay', { feedback: new Setting().feedback }), effect('delay')]; }`);
  const setting = join(pkg, "setting.ts");
  await writeFile(setting, "export class Setting { constructor(public feedback: number = .2) {} }");
  const document = await Document.open({ entry }); cleanup.push(async () => document.close());
  return { root, entry, marker, document, setting };
}

it.each(workers)("$name worker executes npm TypeScript factories afresh and observes dependency byte changes", async ({ Document }) => {
  const { document, marker, setting } = await fixture(Document);
  expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
  const before = document.frame!.snapshot.channels[0]!;
  const reorder = () => {
    const view = document.view, channel = document.frame!.snapshot.channels[0]!;
    const site = view.effectOwnerSites!.find(site => site.owner === channel.id && site.scope === "definition")!;
    return document.editEffectOrder(view.revision, site.handle, channel.id, channel.effectChain.map(effect => effect.instanceId!).reverse());
  };
  await reorder();
  expect(document.frame!.snapshot.channels[0]!.effectChain).toEqual([...before.effectChain].reverse());
  await document.undo(document.view.revision);
  expect(await readFile(marker, "utf8")).toBe("xxx");
  const original = await readFile(setting, "utf8"), previous = await stat(setting);
  await writeFile(setting, original.replace(".2", ".3")); await utimes(setting, previous.atime, previous.mtime);
  await expect(reorder()).rejects.toMatchObject({ code: "SourceChanged" });
  expect(await readFile(marker, "utf8")).toBe("xxx");
  await document.synchronizeDisk();
  expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
  expect(document.frame!.snapshot.channels[0]!.effectChain[0]!.parameters.feedback).toBe(.3);
  expect(await readFile(marker, "utf8")).toBe("xxxx");
});

it.each(workers)("$name worker rejects a new package lock introduced after its directory was scanned", async ({ Document }) => {
  const { root, entry, document } = await fixture(Document);
  expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
  await document.changeCode(document.view.revision, entry, `${await readFile(entry, "utf8")}
(await import('node:fs')).writeFileSync(${JSON.stringify(join(root, "package-lock.json"))}, '{}');`);
  expect(document.view.status).toBe("invalid");
  expect(document.view.diagnostic?.code).toBe("SourceChanged");
});
