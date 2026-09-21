import ts from "typescript";
import { expect, it } from "vitest";
import { readLiteral } from "../src/source/syntax/literals.js";

function parse(expression: string) {
  const file = ts.createSourceFile("song.ts", `const value = ${expression};`, ts.ScriptTarget.Latest, true);
  return (file.statements[0] as ts.VariableStatement).declarationList.declarations[0]!.initializer!;
}

it("reads emitted parameter keys as literal strings without interpreting dotted paths", () => {
  expect(readLiteral(parse('{ ["oscA.level"]: 0.4, "noise.level": 0.1 }'))).toEqual({
    "oscA.level": 0.4,
    "noise.level": 0.1,
  });
});

it.each(["{ [getKey()]: 1 }", "{ [name]: 1 }", '{ ["__proto__"]: null }', '{ a: 1, ["a"]: 2 }'])(
  "rejects ambiguous or evaluated property keys: %s",
  (expression) => expect(() => readLiteral(parse(expression))).toThrow(),
);
