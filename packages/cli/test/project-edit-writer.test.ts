import { expect, it } from "vitest";
import { appendProjectEdit } from "../src/source/editing/project-edit-writer.js";
import { anchorPatternExpression } from "../src/source/editing/pattern-writer.js";
import type { ProjectEvaluation } from "../src/source/eval/project-evaluation.js";

it.each([
  { method: "configure" as const, edit: { kind: "tempo", bpm: 132 } },
  {
    method: "arrange" as const,
    edit: { kind: "pattern", action: "resize", resource: 0, clip: 0, durationBeats: 8 },
  },
])("retains authored comments when reducing $method edits", ({ method, edit }) => {
  const fileName = "song.ts";
  const expression = `project.${method}({ /* Keep this musical intention. */ ... })`.replace(
    "...",
    JSON.stringify(edit).slice(1, -1),
  );
  const text = `export default ${expression};\n`;
  const start = text.indexOf(expression);
  const before = {
    projectSites: [
      {
        label: "default export",
        fileName,
        anchor: anchorPatternExpression(fileName, text, start, start + expression.length),
      },
    ],
  } as ProjectEvaluation;
  const result = appendProjectEdit(before, new Map([[fileName, text]]), method, edit).files.get(fileName)!;
  expect(result).toContain("/* Keep this musical intention. */");
});
