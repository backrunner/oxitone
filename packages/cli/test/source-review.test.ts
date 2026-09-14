import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";
import { ProjectDocument } from "../src/source/document/project-document.js";
import { materializePatternReference } from "../src/source/editing/materialize.js";
import { anchorPatternExpression } from "../src/source/editing/pattern-writer.js";
import { chord } from "@oxitone/core";

it("keeps a same-line directive prologue while inserting the required runtime import", () => {
  const expression = "chord(60, 'major')";
  const text = `"use strict"; import { chord } from '@oxitone/core'; const phrase = ${expression}; export { phrase };`;
  const start = text.indexOf(expression);
  const result = materializePatternReference({ fileName: "/tmp/song.ts", text, anchor: anchorPatternExpression("/tmp/song.ts", text, start, start + expression.length), source: chord(60, "major").toSource(), operations: [] });
  expect(result.text).toMatch(/^"use strict";\s+import \{ Pattern \}/);
  expect(result.text).toContain("export { phrase }");
});

it("captures and writes a project whose author shadows globalThis without changing its bindings", async () => {
  const root = await mkdtemp("/tmp/oxitone-source-review-");
  let document: ProjectDocument | undefined;
  try {
    await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
    await symlink(fileURLToPath(new URL("../../core", import.meta.url)), join(root, "node_modules/@oxitone/core"), "dir");
    const entry = join(root, "song.ts");
    const text = "import { Project, chord } from '@oxitone/core'; const globalThis = { pitch: 60 }; const phrase = chord(globalThis.pitch, 'major'); const project = new Project(); project.addTrack('Lead').add(phrase).at({ bar: 1 }); export default project;";
    await writeFile(entry, text); document = await ProjectDocument.open({ entry });
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    const site = document.view.sites.find(site => site.label === "phrase")!;
    await document.edit(0, site.handle, [{ select: { degree: 1 }, set: { pitch: 61 } }]);
    expect(document.frame!.snapshot.patterns[0]!.notes[0]!.pitch).toBe(61);
    await document.save(1);
    const saved = await readFile(entry, "utf8");
    expect(saved).toContain("globalThis.pitch");
    expect(saved).not.toContain("oxitone-capture:");
    expect(saved).not.toContain("__oxitoneCapture");
    document.close(); document = await ProjectDocument.open({ entry });
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    await document.edit(0, document.view.sites.find(site => site.label === "phrase")!.handle, [{ select: { degree: 2 }, set: { pitch: 66 } }]);
    await document.save(1);
    expect(await readFile(entry, "utf8")).not.toContain("__oxitoneCapture");
  } finally { document?.close(); await rm(root, { recursive: true, force: true }); }
});

it("materializes a helper-only local module using the SDK package actually installed by the project", async () => {
  const root = await mkdtemp("/tmp/oxitone-import-review-");
  let document: ProjectDocument | undefined;
  try {
    await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
    await symlink(fileURLToPath(new URL("../../core", import.meta.url)), join(root, "node_modules/@oxitone/core"), "dir");
    const pkg = join(root, "node_modules/arrangement-fixture"); await mkdir(pkg);
    await writeFile(join(pkg, "package.json"), '{"type":"module","exports":"./index.js"}');
    const dependency = "import { chord } from '@oxitone/core'; export const make = () => chord(60, 'major');";
    await writeFile(join(pkg, "index.js"), dependency);
    const local = join(root, "phrase.ts"), entry = join(root, "song.ts");
    await writeFile(local, "import { make } from 'arrangement-fixture'; export const phrase = make();");
    const source = "import { Project } from '@oxitone/core'; import { phrase } from './phrase.js'; const project = new Project(); project.addTrack('Lead').add(phrase).at({ bar: 1 }); export default project;";
    await writeFile(entry, source);
    document = await ProjectDocument.open({ entry });
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    const site = document.view.sites.find(site => site.label === "phrase" && site.fileName === local)!;
    await document.planMaterialize(0, site.handle, [{ select: { degree: 2 }, remove: true }]);
    expect(document.view.materialization!.afterText).toContain("from '@oxitone/core'");
    await document.confirmMaterialize(0, document.view.materialization!.planId); await document.save(1);
    expect(await readFile(entry, "utf8")).toBe(source);
    expect(await readFile(join(pkg, "index.js"), "utf8")).toBe(dependency);
    document.close(); document = await ProjectDocument.open({ entry });
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    expect(document.frame!.snapshot.patterns[0]!.notes).toHaveLength(2);
  } finally { document?.close(); await rm(root, { recursive: true, force: true }); }
});
