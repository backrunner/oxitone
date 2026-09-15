import ts from "typescript";
import type { Options } from "prettier";
import synchronizedPrettier from "@prettier/sync";

/**
 * Emitted edits conform to the project's own formatting: the prettier
 * configuration resolved for the file wins, an unconfigured file keeps its
 * own quote/indent conventions, and prettier defaults fill the rest.
 */

const configCache = new Map<string, Options | null>();

function projectConfig(fileName: string): Options | null {
  if (!configCache.has(fileName)) {
    let config: Options | null = null;
    try {
      config = synchronizedPrettier.resolveConfig(fileName, { editorconfig: true });
    } catch {
      /* Unreadable or invalid config: fall back to inferred style. */
    }
    configCache.set(fileName, config);
  }
  return configCache.get(fileName)!;
}

/** Drop resolved styles when the workspace may have changed (new roots, tests). */
export function resetSourceFormatCache(): void {
  configCache.clear();
}

const gcd = (a: number, b: number): number => (b === 0 ? a : gcd(b, a % b));

function inferStyle(text: string): Options {
  let single = 0,
    double = 0,
    tabs = 0,
    spaced = 0,
    indent = 0,
    semis = 0,
    bare = 0;
  for (const match of text.matchAll(/(['"])(?:\\.|(?!\1).)*\1/gs)) {
    if (match[1] === "'") single++;
    else double++;
  }
  for (const line of text.split("\n")) {
    const leading = line.match(/^[ \t]+/)?.[0];
    if (leading !== undefined) {
      if (leading.includes("\t")) tabs++;
      else {
        spaced++;
        indent = indent === 0 ? leading.length : gcd(indent, leading.length);
      }
    }
    const trimmed = line.trimEnd();
    if (trimmed.endsWith(";")) semis++;
    else if (/[\w)\]"'`]$/.test(trimmed) && !/[({[,:]$/.test(trimmed)) bare++;
  }
  return {
    ...(single > double ? { singleQuote: true } : {}),
    ...(tabs > spaced ? { useTabs: true } : indent > 0 ? { tabWidth: indent } : {}),
    ...(bare > semis ? { semi: false } : {}),
  };
}

export function sourceFormatOptions(fileName: string, text: string): Options {
  return { ...inferStyle(text), ...(projectConfig(fileName) ?? {}), filepath: fileName };
}

function print(fileName: string, text: string, program: string): string | undefined {
  try {
    return synchronizedPrettier.format(program, { ...sourceFormatOptions(fileName, text), parser: "typescript" });
  } catch {
    return undefined;
  }
}

/**
 * Format one complete expression at base indent zero. The expression is wrapped
 * in a declaration so object literals and comma expressions stay expressions;
 * when the declaration itself wraps, its continuation indent is removed again.
 */
export function formatSourceExpression(fileName: string, text: string, expression: string): string {
  const options = sourceFormatOptions(fileName, text);
  let out: string;
  try {
    out = synchronizedPrettier.format(`const __oxitone = ${expression};`, { ...options, parser: "typescript" });
  } catch {
    return expression;
  }
  const at = out.indexOf("=");
  if (at < 0) return expression;
  let body = out.slice(at + 1);
  if (body.startsWith("\n")) {
    const unit = options.useTabs ? "\t" : " ".repeat(options.tabWidth ?? 2);
    body = body
      .split("\n")
      .map((line) => (line.startsWith(unit) ? line.slice(unit.length) : line))
      .join("\n");
  }
  body = body.trim().replace(/;+$/, "");
  return body || expression;
}

/** Format a call argument list (one or more arguments), keeping prettier's separators. */
export function formatSourceArguments(fileName: string, text: string, args: string): string {
  const out = print(fileName, text, `__oxitone(${args});`);
  if (out === undefined || !out.startsWith("__oxitone(")) return args;
  const close = out.lastIndexOf(")");
  if (close < "__oxitone(".length) return args;
  return out.slice("__oxitone(".length, close);
}

/** Format a complete statement such as an import or an expression statement. */
export function formatSourceStatement(fileName: string, text: string, statement: string): string {
  return print(fileName, text, statement)?.trim() ?? statement;
}

/**
 * Anchor an emitted fragment at its splice indentation. Newlines inside
 * template literals are string data and keep their original bytes.
 */
export function reindentEmitted(fileName: string, fragment: string, newline: string, indent: string): string {
  if (!fragment.includes("\n")) return fragment;
  const spans: Array<[number, number]> = [];
  const base = "const __oxitone = ".length;
  try {
    const file = ts.createSourceFile(fileName, `const __oxitone = ${fragment};`, ts.ScriptTarget.Latest, true);
    const visit = (node: ts.Node): void => {
      if (ts.isNoSubstitutionTemplateLiteral(node)) {
        spans.push([node.getStart(file) - base, node.end - base]);
      } else if (ts.isTemplateExpression(node)) {
        spans.push([node.head.getStart(file) - base, node.head.end - base]);
        for (const span of node.templateSpans) {
          spans.push([span.literal.getStart(file) - base, span.literal.end - base]);
        }
      }
      ts.forEachChild(node, visit);
    };
    visit(file);
  } catch {
    /* An unparseable fragment is spliced as-is; validation rejects it later. */
  }
  spans.sort((a, b) => a[0] - b[0]);
  let out = "",
    span = 0;
  for (let i = 0; i < fragment.length; i++) {
    while (span < spans.length && i >= spans[span]![1]) span++;
    const inside = span < spans.length && i >= spans[span]![0] && i < spans[span]![1];
    if (fragment[i] === "\n" && !inside) {
      out += (fragment[i - 1] === "\r" ? "\n" : newline) + indent;
    } else out += fragment[i];
  }
  return out;
}
