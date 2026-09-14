import type { Plugin } from "esbuild";
import ts from "typescript";
import { importPosition } from "../syntax/imports.js";

export function captureSpecifier(key: string): string { return `oxitone-capture:${key}`; }

export function captureImport(file: ts.SourceFile, key: string): { binding: string; at: number; text: string } {
  const names = new Set<string>();
  const collect = (node: ts.Node) => { if (ts.isIdentifier(node)) names.add(node.text); ts.forEachChild(node, collect); };
  collect(file);
  let binding = "__oxitoneCapture";
  for (let suffix = 1; names.has(binding); suffix++) binding = `__oxitoneCapture${suffix}`;
  return { binding, at: importPosition(file), text: `\nimport { capture as ${binding} } from ${JSON.stringify(captureSpecifier(key))};\n` };
}

/** A separate lexical scope keeps user bindings (including globalThis) out of the capture runtime. */
export function projectCaptureModule(key: string): Plugin {
  return { name: "oxitone-capture-module", setup(api) {
    api.onResolve({ filter: /^oxitone-capture:/ }, args => args.path === captureSpecifier(key) ? { path: key, namespace: "oxitone-capture" } : undefined);
    api.onLoad({ filter: /.*/, namespace: "oxitone-capture" }, () => ({ loader: "js", contents: `export const capture = globalThis[${JSON.stringify(key)}];` }));
  } };
}
