import { expect, it } from "vitest";
import { mergeThreeWay } from "../src/source/three-way-merge.js";

it("merges disjoint line edits and identical edits", () => {
  const base = "one\ntwo\nthree\n";
  expect(mergeThreeWay(base, "ONE\ntwo\nthree\n", "one\ntwo\nTHREE\n")).toBe("ONE\ntwo\nTHREE\n");
  expect(mergeThreeWay(base, "one\nTWO\nthree\n", "one\nTWO\nthree\n")).toBe("one\nTWO\nthree\n");
});

it("rejects overlapping edits and preserves explicit insertion conflicts", () => {
  const base = "one\ntwo\nthree\n";
  expect(mergeThreeWay(base, "ONE\ntwo\nthree\n", "OTHER\ntwo\nthree\n")).toBeUndefined();
  expect(mergeThreeWay(base, "one\nnew\ntwo\nthree\n", "one\nother\ntwo\nthree\n")).toBeUndefined();
});
