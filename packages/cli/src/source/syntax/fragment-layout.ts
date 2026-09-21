import ts from "typescript";

/** String and template newlines are data, even when removing a printer's indent. */
function stringSpans(fileName: string, fragment: string): Array<[number, number]> {
  const prefix = "const __oxitone = ";
  const file = ts.createSourceFile(fileName, `${prefix}${fragment};`, ts.ScriptTarget.Latest, true);
  const spans: Array<[number, number]> = [];
  const add = (node: ts.Node) => spans.push([node.getStart(file) - prefix.length, node.end - prefix.length]);
  const visit = (node: ts.Node): void => {
    if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) add(node);
    else if (ts.isTemplateExpression(node)) {
      add(node.head);
      for (const span of node.templateSpans) add(span.literal);
    }
    ts.forEachChild(node, visit);
  };
  visit(file);
  return spans.sort((a, b) => a[0] - b[0]);
}

function layout(fileName: string, fragment: string, newline: string | undefined, add: string, remove: string): string {
  if (!fragment.includes("\n")) return fragment;
  const spans = stringSpans(fileName, fragment);
  let out = "",
    span = 0,
    start = 0;
  for (const match of fragment.matchAll(/\r?\n/g)) {
    const at = match.index;
    while (span < spans.length && at >= spans[span]![1]) span++;
    if (span < spans.length && at >= spans[span]![0]) continue;
    out += fragment.slice(start, at) + (newline ?? match[0]) + add;
    start = at + match[0].length;
    if (remove && fragment.startsWith(remove, start)) start += remove.length;
  }
  return out + fragment.slice(start);
}

/** Anchor a fragment without changing quoted or template string data. */
export function reindentEmitted(fileName: string, fragment: string, newline: string, indent: string): string {
  return layout(fileName, fragment, newline, indent, "");
}

/** Undo only the temporary declaration's continuation indent. */
export function dedentEmitted(fileName: string, fragment: string, indent: string): string {
  return layout(fileName, fragment, undefined, "", indent);
}
