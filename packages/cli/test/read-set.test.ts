import { mkdtemp, rm, stat, symlink, unlink, utimes, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, expect, it } from "vitest";
import { captureMissingSource, captureSourceReads, checkSourceReads } from "../src/source/files/read-set.js";

const roots: string[] = [];
afterEach(async () => {
  for (const root of roots.splice(0)) await rm(root, { recursive: true, force: true });
});
async function fixture() {
  const root = await mkdtemp(join(tmpdir(), "oxitone-read-set-"));
  roots.push(root);
  const paths = Array.from({ length: 40 }, (_, i) => join(root, `${i}.ts`));
  await Promise.all(paths.map((path) => writeFile(path, "export default 60;\n")));
  return { root, paths };
}

it("checks every batch and rejects same-size byte changes even with the original mtime", async () => {
  const { paths } = await fixture(),
    path = paths[35]!;
  const reads = await captureSourceReads(paths),
    before = await stat(path);
  await expect(checkSourceReads(reads)).resolves.toBeUndefined();
  await writeFile(path, "export default 61;\n");
  await utimes(path, before.atime, before.mtime);
  expect((await stat(path)).size).toBe(before.size);
  await expect(checkSourceReads(reads)).rejects.toMatchObject({ code: "SourceChanged", details: { path } });
});

it("rejects a retargeted symlink with identical bytes and an appearing missing dependency", async () => {
  const { root, paths } = await fixture(),
    link = join(root, "linked.ts"),
    missing = join(root, "package.json");
  await symlink(paths[0]!, link);
  const reads = await captureSourceReads([link]),
    negative = await captureMissingSource(missing);
  await expect(checkSourceReads([...reads, negative])).resolves.toBeUndefined();
  await unlink(link);
  await symlink(paths[1]!, link);
  await expect(checkSourceReads(reads)).rejects.toMatchObject({ code: "SourceChanged", details: { path: link } });
  await writeFile(missing, "{}");
  await expect(checkSourceReads([negative])).rejects.toMatchObject({
    code: "SourceChanged",
    details: { path: missing },
  });
});

it("reports concurrent failures in input order and enforces the read-set budget", async () => {
  const { paths } = await fixture(),
    reads = await captureSourceReads(paths);
  await writeFile(paths[0]!, "changed");
  await unlink(paths[1]!);
  await expect(checkSourceReads(reads)).rejects.toMatchObject({ code: "SourceChanged", details: { path: paths[0] } });
  await expect(checkSourceReads(Array.from({ length: 4097 }, () => reads[0]!))).rejects.toMatchObject({
    code: "BudgetExceeded",
  });
  await expect(captureSourceReads(Array.from({ length: 4097 }, () => paths[0]!))).rejects.toMatchObject({
    code: "BudgetExceeded",
  });
});
