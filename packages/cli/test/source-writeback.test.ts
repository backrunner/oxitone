import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";
import { ProjectDocument } from "../src/source/index.js";

it("preserves authored text and project data through repeated DAW edits, Undo, Save and fresh evaluation", async () => {
  const root = await mkdtemp(join(tmpdir(), "oxitone-writeback-"));
  let document: ProjectDocument | undefined;
  const template = "`first\n  second\n    third`";
  const comment = "/* Keep this tempo choice documented. */";
  const source = `import { Project } from '@oxitone/core';
function createProjectWithAVeryLongFunctionName(name: string) {
  return new Project({ name });
}
export default createProjectWithAVeryLongFunctionName(${template}).configure({ kind: 'tempo', ${comment} bpm: 120 });
`;
  try {
    await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
    await symlink(
      fileURLToPath(new URL("../../core", import.meta.url)),
      join(root, "node_modules/@oxitone/core"),
      "dir",
    );
    await writeFile(join(root, ".prettierrc"), JSON.stringify({ printWidth: 40 }));
    const entry = join(root, "song.ts");
    await writeFile(entry, source);
    document = await ProjectDocument.open({ entry });
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    await document.configure(document.view.revision, { kind: "tempo", bpm: 132 });
    await document.configure(document.view.revision, { kind: "tempo", bpm: 140 });
    const draft = document.view.files.find((file) => file.path === entry)!.text;
    expect(draft).toContain(comment);
    expect(draft).toContain(template);
    expect(draft.match(/\.configure\(/g)).toHaveLength(2);
    expect(await readFile(entry, "utf8")).toBe(source);
    await document.undo(document.view.revision);
    await document.redo(document.view.revision);
    expect(document.view.files.find((file) => file.path === entry)!.text).toBe(draft);
    const snapshot = document.frame!.snapshot;
    await document.save(document.view.revision);
    expect(await readFile(entry, "utf8")).toBe(draft);
    document.close();
    document = await ProjectDocument.open({ entry });
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    expect({ ...document.frame!.snapshot, revision: "0" }).toEqual({ ...snapshot, revision: "0" });
    expect(document.view.modified).toBe(false);
  } finally {
    document?.close();
    await rm(root, { recursive: true, force: true });
  }
});
