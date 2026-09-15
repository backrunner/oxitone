import { describe, expect, it } from "vitest";
import { arp, chord, Pattern } from "@oxitone/core";
import { anchorPatternExpression, materializePatternReference, writeLiteralPatternEdit } from "../src/source/index.js";

const base = arp(chord(60, "major"), "upDown", 0.25);
function request(text: string, pattern = base) {
  const start = text.lastIndexOf("phrase");
  return {
    fileName: "song.ts",
    text,
    source: pattern.toSource(),
    anchor: anchorPatternExpression("song.ts", text, start, start + 6),
    operations: [{ select: { step: 1 }, set: { pitch: 65 } }],
  };
}

describe("Pattern materialization", () => {
  it("reuses named and namespace imports, preserving exports and unrelated text", () => {
    for (const [header, constructor] of [
      ["import { Pattern as Notes } from '@oxitone/core';", "Notes"],
      ["import * as music from '@oxitone/core';", "music.Pattern"],
    ]) {
      const text = `${header}\nimport { phrase } from 'arrangements';\nexport const other = phrase;\nexport default phrase; // kept\n`;
      const result = materializePatternReference(request(text));
      expect(result.text).toContain(`export default new ${constructor}({`);
      expect(result.text.startsWith(text.slice(0, text.lastIndexOf("phrase")))).toBe(true);
      expect(result.text.endsWith("; // kept\n")).toBe(true);
      expect(result.source.nodes.map((node) => node.kind)).toEqual(["literal"]);
      expect(Pattern.fromSource(result.source).notes).toEqual(base.edit(request(text).operations).notes);
      expect(result.summary).toMatchObject({ afterNotes: 4, scope: "selected-reference", losesGeneratorLink: true });
    }
  });

  it("adds a runtime import for type-only imports and avoids shadowed aliases", () => {
    for (const text of [
      "import type { Pattern } from '@oxitone/core';\nimport { phrase } from 'arrangements';\nexport default phrase;",
      "import { type Pattern } from '@oxitone/core';\nimport { phrase } from 'arrangements';\nexport default phrase;",
      "import { Pattern as Notes } from '@oxitone/core';\nimport { phrase } from 'arrangements';\nexport function make(Notes: unknown) { return phrase; }",
      "import * as music from '@oxitone/core';\nimport { phrase } from 'arrangements';\nexport function make(music: unknown) { return phrase; }",
    ]) {
      const result = materializePatternReference(request(text));
      expect(result.text.match(/from '@oxitone\/core'/g)).toHaveLength(2);
      expect(result.text).toContain(text.slice(0, text.indexOf("\n")));
      expect(result.anchor.expression).not.toMatch(/new Notes\(|new music\.Pattern\(/);
    }
  });

  it("retains CRLF, shebang and directive prologues when inserting imports", () => {
    const text =
      "#!/usr/bin/env node\r\n'use strict'; // directive\r\nimport { phrase } from 'arrangements';\r\nexport default phrase;\r\n";
    const result = materializePatternReference(request(text));
    expect(result.text.startsWith("#!/usr/bin/env node\r\n'use strict'; // directive\r\n")).toBe(true);
    expect(result.text).not.toMatch(/(?<!\r)\n/);
    expect(result.text).not.toMatch(/pat_|UUID|sourceHash/);
  });

  it("edits materialized Note literals directly without accumulating wrappers", () => {
    let result = materializePatternReference(request("import { phrase } from 'arrangements';\nexport default phrase;"));
    for (let i = 0; i < 4; i++) {
      const changed = writeLiteralPatternEdit({
        ...result,
        fileName: "song.ts",
        operations: [{ select: { at: { start: 0.25, pitch: 65 + i } }, set: { pitch: 66 + i } }],
      });
      expect(changed).toBeDefined();
      result = { ...result, ...changed! };
    }
    expect(result.text).not.toContain(".edit(");
    expect(Pattern.fromSource(result.source).notes[1]?.pitch).toBe(69);
  });

  it("rejects stale references, probabilistic notes and source windows", () => {
    const initial = request("import { phrase } from 'arrangements'; export default phrase;");
    expect(() => materializePatternReference({ ...initial, text: `${initial.text}\n` })).toThrowError(
      expect.objectContaining({ code: "SourceChanged" }),
    );
    const chance = base.edit([{ select: { step: 0 }, set: { chance: 0.5 } }]);
    expect(() => materializePatternReference(request(initial.text, chance))).toThrow("probabilistic");
    expect(() => materializePatternReference(request(initial.text, base.slice(0, 0.5)))).toThrow("windows");
  });

  it("does not remove arbitrary calls, accessors or declaration bindings", () => {
    const expression = "makePhrase()";
    const text = `export default ${expression};`;
    const result = materializePatternReference({
      ...request("const phrase = 1; export default phrase;"),
      text,
      anchor: anchorPatternExpression("song.ts", text, 15, 15 + expression.length),
    });
    expect(result.text).toMatch(/\(makePhrase\(\),\s*new Pattern\(/);
    expect(result.summary.retainsOriginalEvaluation).toBe(true);
    const edited = writeLiteralPatternEdit({
      ...result,
      fileName: "song.ts",
      operations: [{ select: { at: { start: 0, pitch: 60 } }, set: { pitch: 61 } }],
    })!;
    expect(edited.text).not.toContain(".edit(");
    expect(edited.text.match(/makePhrase\(\)/g)).toHaveLength(1);
    for (const [text, start, end] of [
      ["let phrase = 1; phrase = 2;", 16, 22],
      ["const value = { phrase };", 16, 22],
      ["import { phrase } from 'x';", 9, 15],
    ] as const)
      expect(() => anchorPatternExpression("song.ts", text, start, end)).toThrow();
  });
});
