import { expect, it } from "vitest";
import { textChange } from "../src/text-change";

it("preserves common source, complete Unicode characters and line endings", () => {
  expect(textChange("chord(60, 'major')", "chord(64, 'major')")).toEqual({ start: 7, end: 8, text: "4" });
  for (const [before, after] of [
    ["", "note"],
    ["note", ""],
    ["ab", "ab"],
    ["a😀b", "a😁b"],
    ["a\r\nb", "a\nb"],
    ["a\nb", "a\r\nb"],
    ["a😀", "a😀x"],
  ]) {
    const edit = textChange(before!, after!);
    expect(before!.slice(0, edit.start) + edit.text + before!.slice(edit.end)).toBe(after);
    expect(edit.text).not.toMatch(/[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/);
  }
});
