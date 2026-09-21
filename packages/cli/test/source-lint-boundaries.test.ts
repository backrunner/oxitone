import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterAll, expect, it } from "vitest";
import { lintSourceCandidate, resetSourceLintCache } from "../src/source/syntax/eslint-fix.js";

const roots: string[] = [];
afterAll(async () => {
  resetSourceLintCache();
  await Promise.all(roots.map((root) => rm(root, { recursive: true, force: true })));
});

it.each(["", 'import { Pattern } from "pattern";\n', "\uFEFF"])(
  "retains existing imports across materialization with prefix %j",
  async (prefix) => {
    const root = await mkdtemp(join(tmpdir(), "oxitone-lint-boundary-"));
    roots.push(root);
    // Model an unused-import fixer: the original import was used, so the old
    // whole-file before-lint guard cannot detect this newly applicable fix.
    await writeFile(
      join(root, "eslint.config.mjs"),
      `
const unusedImport = {
  meta: { fixable: "code", schema: [] },
  create(context) {
    return { ImportDeclaration(node) {
      if (node.source.value !== "shared-pattern") return;
      const references = context.sourceCode.getText().match(/shared/g) ?? [];
      if (references.length !== 2) return;
      context.report({ node, message: "Unused import", fix: fixer => fixer.remove(node) });
    } };
  },
};
export default [{ files: ["**/*.ts"], plugins: { local: { rules: { "unused-import": unusedImport } } },
  rules: { "local/unused-import": "error", "comma-dangle": ["error", "never"] } }];
`,
    );
    const originalImport = 'import { shared } from "shared-pattern";\n';
    const before = `${prefix === "\uFEFF" ? prefix : ""}${originalImport}export default shared;\n`;
    const candidate = `${prefix}${originalImport}export default new Pattern({ lengthBeats: 4, notes: [], });\n`;
    const result = await lintSourceCandidate(root, join(root, "song.ts"), before, candidate);
    expect(result).toContain(originalImport);
    expect(result).toContain("notes: [] }");
    expect(result).toBe(candidate.replace("notes: [],", "notes: []"));
  },
);
