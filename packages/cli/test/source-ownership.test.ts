import { link, mkdir, mkdtemp, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { SourceOwnership } from "../src/source/index.js";
import { projectSourceFiles, readSourceText } from "../src/source/files/project-files.js";

describe("project source ownership", () => {
  it("requires explicit files and rejects dependencies, build output and declarations", async () => {
    const root = await mkdtemp(join(tmpdir(), "oxitone-owned-"));
    try {
      const entry = join(root, "song.ts");
      await writeFile(entry, "export default 1;");
      const ownership = await SourceOwnership.open([root], [entry]);
      expect((await ownership.assertWritable(entry)).path).toBe(entry);
      const second = join(root, "second.ts"); await writeFile(second, "");
      await expect(ownership.assertWritable(second)).rejects.toMatchObject({ details: { reason: "SourceReadOnly" } });
      for (const directory of ["node_modules/pkg", ".pnpm/pkg", ".yarn/cache", "dist", "target"]) {
        const dir = join(root, directory); await mkdir(dir, { recursive: true });
        const file = join(dir, "music.ts"); await writeFile(file, "");
        await expect(SourceOwnership.open([root], [file])).rejects.toMatchObject({ code: "EditNotRepresentable" });
      }
      const declaration = join(root, "music.d.ts"); await writeFile(declaration, "");
      await expect(SourceOwnership.open([root], [declaration])).rejects.toThrow();
      await expect(SourceOwnership.open([join(root, "node_modules/pkg")], [])).rejects.toThrow();
    } finally { await rm(root, { recursive: true, force: true }); }
  });

  it("rechecks symbolic links and rejects hard links to other content", async () => {
    const root = await mkdtemp(join(tmpdir(), "oxitone-links-"));
    const external = await mkdtemp(join(tmpdir(), "oxitone-external-"));
    try {
      const original = join(root, "original.ts"); const outside = join(external, "outside.ts");
      await writeFile(original, "export default 1;"); await writeFile(outside, "export default 2;");
      const alias = join(root, "alias.ts"); await symlink(original, alias);
      const ownership = await SourceOwnership.open([root], [alias]);
      await rm(alias); await symlink(outside, alias);
      await expect(ownership.assertWritable(alias)).rejects.toMatchObject({ code: "EditNotRepresentable" });
      const hard = join(root, "hard.ts"); await link(outside, hard);
      await expect(SourceOwnership.open([root], [hard])).rejects.toThrow();
      const dependency = join(root, "node_modules/pkg"); await mkdir(dependency, { recursive: true });
      const internal = join(dependency, "notes.ts"); await writeFile(internal, "");
      const disguised = join(root, "notes.ts"); await symlink(internal, disguised);
      await expect(SourceOwnership.open([root], [disguised])).rejects.toThrow();
    } finally { await rm(root, { recursive: true, force: true }); await rm(external, { recursive: true, force: true }); }
  });

  it("requires separate enrollment of nested workspace package sources", async () => {
    const root = await mkdtemp(join(tmpdir(), "oxitone-workspace-"));
    try {
      const pkg = join(root, "packages/arrangements"); await mkdir(pkg, { recursive: true });
      await writeFile(join(pkg, "package.json"), '{"name":"arrangements"}');
      const source = join(pkg, "index.ts"); await writeFile(source, "export const phrase = 1;");
      await expect(SourceOwnership.open([root], [source])).rejects.toThrow("nested package");
      const ownership = await SourceOwnership.open([root, pkg], [source]);
      expect((await ownership.assertWritable(source)).path).toBe(source);
      const linked = join(root, "linked.ts"); await symlink(source, linked);
      // A logical path outside the nested package cannot bypass its real module ownership.
      await expect(SourceOwnership.open([root], [linked])).rejects.toThrow("nested package");
    } finally { await rm(root, { recursive: true, force: true }); }
  });

  it("rejects oversized files before building an in-memory project map", async () => {
    const root = await mkdtemp(join(tmpdir(), "oxitone-source-budget-"));
    try {
      const oversized = join(root, "oversized.ts");
      await writeFile(oversized, Buffer.alloc(8 * 1024 * 1024 + 1, 0x78));
      await expect(readSourceText(oversized)).rejects.toMatchObject({ code: "BudgetExceeded" });
      await expect(projectSourceFiles([root])).rejects.toMatchObject({ code: "BudgetExceeded" });
    } finally { await rm(root, { recursive: true, force: true }); }
  });

  it("enforces the aggregate source budget while reading files sequentially", async () => {
    const root = await mkdtemp(join(tmpdir(), "oxitone-source-total-budget-"));
    try {
      for (let index = 0; index < 5; index++) await writeFile(join(root, `part-${index}.ts`), Buffer.alloc(7 * 1024 * 1024, 0x78));
      await expect(projectSourceFiles([root])).rejects.toMatchObject({ code: "BudgetExceeded" });
    } finally { await rm(root, { recursive: true, force: true }); }
  });
});
