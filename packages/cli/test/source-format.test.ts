import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import prettier from "prettier";
import { afterAll, describe, expect, it } from "vitest";
import { ProjectDocument } from "../src/source/index.js";
import {
  formatSourceArguments,
  formatSourceExpression,
  formatSourceStatement,
  reindentEmitted,
  resetSourceFormatCache,
} from "../src/source/syntax/format.js";
import { lintSourceCandidate, resetSourceLintCache } from "../src/source/syntax/eslint-fix.js";

const roots: string[] = [];
const documents: ProjectDocument[] = [];

afterAll(async () => {
  await Promise.all(documents.map((document) => document.close()));
  await Promise.all(roots.map((root) => rm(root, { recursive: true, force: true })));
  resetSourceFormatCache();
  resetSourceLintCache();
});

async function workspace(files: Record<string, string>): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), "oxitone-format-"));
  roots.push(root);
  for (const [name, text] of Object.entries(files)) {
    if (name.includes("/")) await mkdir(join(root, name.slice(0, name.lastIndexOf("/"))), { recursive: true });
    await writeFile(join(root, name), text);
  }
  return root;
}

describe("source emission formatting", () => {
  it("prints emitted expressions with prettier defaults for an unstyled file", () => {
    const text = "const x = 1;\n";
    expect(formatSourceExpression("song.ts", text, '(project).configure({"kind":"tempo","bpm":132})')).toBe(
      'project.configure({ kind: "tempo", bpm: 132 })',
    );
    expect(formatSourceArguments("song.ts", text, '[{"set":{"pitch":61}}]')).toBe("[{ set: { pitch: 61 } }]");
    expect(formatSourceStatement("song.ts", text, 'import {Pattern} from "@oxitone/core"')).toBe(
      'import { Pattern } from "@oxitone/core";',
    );
  });

  it("keeps an unconfigured file's own quote and indent conventions", () => {
    const text = "const p = chord(60, 'major');\n\tconst q = arp(p, 'up', 1);\n";
    expect(formatSourceExpression("song.ts", text, '(p).edit([{"select":{"step":1},"remove":true}])')).toBe(
      "p.edit([{ select: { step: 1 }, remove: true }])",
    );
    expect(formatSourceExpression("song.ts", text, '(project).configure({"kind":"tempo","bpm":132})')).toBe(
      "project.configure({ kind: 'tempo', bpm: 132 })",
    );
  });

  it("wraps long chains at member boundaries with relative indentation", () => {
    const text = "export default project;\n";
    const out = formatSourceExpression(
      "song.ts",
      text,
      `(project).configure(${JSON.stringify({ kind: "channel", index: 0, values: { level: 0.5, pan: -0.3, sends: { reverb: 0.2, delay: 0.4 } } })})`,
    );
    expect(out).toContain("\n");
    expect(out).toMatch(/\}\)$/);
    for (const line of out.split("\n").slice(1, -1)) expect(line).toMatch(/^ {2}\S/);
  });

  it("lets a project prettier config override the file's own style", async () => {
    const root = await workspace({
      ".prettierrc": JSON.stringify({ singleQuote: false, semi: false, printWidth: 200 }),
    });
    resetSourceFormatCache();
    const text = "const p = chord(60, 'major');\n";
    expect(formatSourceExpression(join(root, "song.ts"), text, '(p).edit([{"select":{"step":1},"remove":true}])')).toBe(
      "p.edit([{ select: { step: 1 }, remove: true }])",
    );
    expect(formatSourceStatement(join(root, "song.ts"), text, 'import {Pattern} from "@oxitone/core"')).toBe(
      'import { Pattern } from "@oxitone/core"',
    );
  });

  it("reindents continuation lines without rewriting template literal bytes", () => {
    const fragment = "sample(`line1\nline2 ${x}\nline3`).edit([\n  { select: { step: 1 } },\n])";
    const out = reindentEmitted("song.ts", fragment, "\n", "    ");
    expect(out).toContain("`line1\nline2 ${x}\nline3`");
    expect(out).toContain(".edit([\n      {");
  });
});

describe("candidate eslint polish", () => {
  it("leaves projects without an eslint config untouched", async () => {
    const root = await workspace({ "song.ts": "export default project;\n" });
    const before = "export default project;\n";
    const candidate = 'export default project.configure({ kind: "tempo", bpm: 132, });\n';
    expect(await lintSourceCandidate(root, join(root, "song.ts"), before, candidate)).toBe(candidate);
  });

  it("applies the project's eslint --fix to emitted code only", async () => {
    const root = await workspace({
      "eslint.config.mjs": 'export default [{ files: ["**/*.ts"], rules: { "comma-dangle": ["error", "never"] } }];\n',
      "song.ts": "export default project\n",
    });
    const before = "export default project\n";
    const candidate =
      'export default project.configure({\n  kind: "channel",\n  index: 0,\n  values: { level: 0.5 },\n})\n';
    const polished = await lintSourceCandidate(root, join(root, "song.ts"), before, candidate);
    expect(polished).toContain("values: { level: 0.5 }\n})");
    expect(polished).not.toContain(",\n})");
  });

  it("never rewrites unrelated violations that predated the edit", async () => {
    const root = await workspace({
      "eslint.config.mjs": 'export default [{ files: ["**/*.ts"], rules: { "comma-dangle": ["error", "never"] } }];\n',
      "song.ts": "const stale = { a: 1, };\nexport default project\n",
    });
    const before = "const stale = { a: 1, };\nexport default project\n";
    const candidate = `${before.slice(0, -1)}.configure({ kind: "tempo", bpm: 132, })\n`;
    expect(await lintSourceCandidate(root, join(root, "song.ts"), before, candidate)).toBe(candidate);
  });
});

describe("document transactions", () => {
  it("saves project edits that stay prettier-clean under the project config", async () => {
    const root = await workspace({
      ".prettierrc": JSON.stringify({ singleQuote: true }),
      "song.ts": "",
    });
    await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
    await symlink(
      fileURLToPath(new URL("../../core", import.meta.url)),
      join(root, "node_modules/@oxitone/core"),
      "dir",
    );
    const entry = join(root, "song.ts");
    await writeFile(
      entry,
      await prettier.format(
        [
          "import { Project } from '@oxitone/core';",
          "",
          "export default function make() {",
          "  const project = new Project();",
          "  project.addChannel({ name: 'Lead' });",
          "  return project;",
          "}",
          "",
        ].join("\n"),
        { parser: "typescript", singleQuote: true },
      ),
    );
    const document = await ProjectDocument.open({ entry });
    documents.push(document);
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    await document.configure(document.view.revision, {
      kind: "channel",
      index: 0,
      values: { level: 0.5, pan: -0.3 },
    });
    await document.save(document.view.revision);
    const saved = await readFile(entry, "utf8");
    expect(saved).toContain(".configure({");
    const config = await prettier.resolveConfig(entry);
    expect(await prettier.check(saved, { ...config, filepath: entry })).toBe(true);
  });

  it("runs the project's eslint --fix on emitted code inside a transaction", async () => {
    const root = await workspace({
      "eslint.config.mjs": 'export default [{ files: ["**/*.ts"], rules: { "comma-dangle": ["error", "never"] } }];\n',
    });
    await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
    await symlink(
      fileURLToPath(new URL("../../core", import.meta.url)),
      join(root, "node_modules/@oxitone/core"),
      "dir",
    );
    const entry = join(root, "song.ts");
    await writeFile(
      entry,
      "import { Project } from '@oxitone/core';\nexport default function make() {\n  const project = new Project();\n  project.addChannel({ name: 'Lead' });\n  return project;\n}\n",
    );
    const document = await ProjectDocument.open({ entry });
    documents.push(document);
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    await document.configure(document.view.revision, {
      kind: "channel",
      index: 0,
      values: { level: 0.5, pan: -0.3 },
    });
    const text = document.view.files.find((file) => file.path === entry)!.text;
    expect(text).toContain(".configure({\n");
    expect(text).toContain("pan: -0.3 }\n  });");
  });
});
