import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { describe, expect, it } from "vitest";
import { arp, chord, Pattern } from "@oxitone/core";
import { ErrorCode, type NoteEdit } from "@oxitone/protocol";
import { anchorPatternExpression, writePatternEdit } from "../src/source/index.js";
import { bundleProject } from "../src/bundle.js";

function request(text: string, expression: string, pattern: Pattern, operations: NoteEdit[]) {
  const start = text.indexOf(expression);
  return {
    text,
    fileName: "song.ts",
    source: pattern.toSource(),
    operations,
    anchor: anchorPatternExpression("song.ts", text, start, start + expression.length),
  };
}

describe("Pattern source writer", () => {
  it("wraps exactly the selected expression and preserves surrounding bytes", () => {
    const expression = "arp(harmony, 'upDown', 0.25)";
    const text = `// keep this comment\r\nconst phrase = ${expression}; // keep this too\r\n`;
    const result = writePatternEdit(
      request(text, expression, arp(chord(60, "major"), "upDown", 0.25), [{ select: { step: 1 }, remove: true }]),
    );
    expect(result.text.slice(0, result.anchor.start)).toBe(text.slice(0, text.indexOf(expression)));
    expect(result.text.slice(result.anchor.end)).toBe("; // keep this too\r\n");
    expect(result.anchor.expression).toContain(`${expression}.edit([`);
    expect(result.text).not.toMatch(/(?<!\r)\n/);
    expect(result.text).not.toMatch(/"id"|UUID|sourceHash/);
  });

  it("returns byte-identical text for no-op edits", () => {
    const text = "export default chord(60, 'major');\n";
    expect(writePatternEdit(request(text, "chord(60, 'major')", chord(60, "major"), [])).text).toBe(text);
  });

  it("rejects stale anchors, partial expressions, declaration names and invalid drafts", () => {
    const initial = request("const p = chord(60, 'major');", "chord(60, 'major')", chord(60, "major"), []);
    expect(() => writePatternEdit({ ...initial, text: `// formatted\n${initial.text}` })).toThrowError(
      expect.objectContaining({ code: ErrorCode.SourceChanged }),
    );
    expect(() => anchorPatternExpression("song.ts", initial.text, 0, 5)).toThrowError(
      expect.objectContaining({ code: ErrorCode.EditNotRepresentable }),
    );
    expect(() => anchorPatternExpression("song.ts", initial.text, 6, 7)).toThrowError(
      expect.objectContaining({ code: ErrorCode.EditNotRepresentable }),
    );
    expect(() => anchorPatternExpression("song.ts", "const p = chord(;", 10, 15)).toThrowError(
      expect.objectContaining({ code: ErrorCode.DraftInvalid }),
    );
  });

  it("updates one literal edit list across repeated drags", () => {
    let current = writePatternEdit(
      request("export default arp([60,64], 'up', 1);", "arp([60,64], 'up', 1)", arp([60, 64], "up", 1), [
        { select: { step: 1 }, set: { pitch: 65 } },
      ]),
    );
    for (let i = 0; i < 20; i++)
      current = writePatternEdit({
        ...current,
        fileName: "song.ts",
        operations: [{ select: { step: 1 }, set: { pitch: 66 + (i % 2) } }],
      });
    expect(current.text.match(/\.edit\(/g)).toHaveLength(1);
    expect(current.text).toContain("pitch: 67");
    expect(Pattern.fromSource(current.source).notes[1]?.pitch).toBe(67);
  });

  it("preserves comments inside an existing list without endlessly wrapping later drags", () => {
    const expression = "arp([60,64], 'up', 1).edit([/* human variation */ {select:{step:1},set:{pitch:65}}])";
    let current = writePatternEdit(
      request(
        `export default ${expression};`,
        expression,
        arp([60, 64], "up", 1).edit([{ select: { step: 1 }, set: { pitch: 65 } }]),
        [{ select: { step: 0 }, set: { pitch: 61 } }],
      ),
    );
    for (let i = 0; i < 3; i++)
      current = writePatternEdit({
        ...current,
        fileName: "song.ts",
        operations: [{ select: { step: 0 }, set: { pitch: 62 + i } }],
      });
    expect(current.text).toContain("/* human variation */");
    expect(current.text.match(/\.edit\(/g)).toHaveLength(2);
  });

  it("writes a shared reference locally and rebuilds the saved TS in a fresh module", async () => {
    const cache = fileURLToPath(new URL("../node_modules/.cache/", import.meta.url));
    await mkdir(cache, { recursive: true });
    const directory = await mkdtemp(join(cache, "source-writer-"));
    try {
      const entry = join(directory, "song.ts");
      const definition = join(directory, "harmony.ts");
      const definitions =
        "import { chord, arp } from '@oxitone/core';\nexport const phrase = arp(chord(60, 'major'), 'upDown', 0.25).repeat(3);\n";
      await writeFile(definition, definitions);
      const text = "import { phrase } from './harmony.js';\nexport const shared = phrase;\nexport default phrase;\n";
      const start = text.lastIndexOf("phrase");
      const base = arp(chord(60, "major"), "upDown", 0.25).repeat(3);
      const operations: NoteEdit[] = [
        { select: { iteration: 1, note: { step: 2 } }, set: { pitch: 70 } },
        { select: { iteration: 2, note: { step: 1 } }, remove: true },
      ];
      const result = writePatternEdit({
        fileName: entry,
        text,
        anchor: anchorPatternExpression(entry, text, start, start + 6),
        source: base.toSource(),
        operations,
      });
      await writeFile(entry, result.text);
      const output = join(directory, "fresh.mjs");
      await bundleProject(entry, output);
      const loaded = (await import(pathToFileURL(output).href)) as { default: Pattern; shared: Pattern };
      expect(loaded.default.outputs).toEqual(base.edit(operations).outputs);
      expect(loaded.default.toSource()).toEqual(result.source);
      expect(loaded.shared.outputs).toEqual(base.outputs);
      expect(await readFile(definition, "utf8")).toBe(definitions);
      // Drop the bundle and rebuild from saved TS alone; no source map or editor cache is read.
      await rm(output);
      const second = join(directory, "fresh-again.mjs");
      await bundleProject(entry, second);
      const reopened = (await import(pathToFileURL(second).href)) as { default: Pattern };
      expect(reopened.default.outputs).toEqual(loaded.default.outputs);
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });
});
