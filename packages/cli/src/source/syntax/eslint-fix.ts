import { createRequire } from "node:module";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { applyEmittedFixes } from "./emitted-fixes.js";

/**
 * Best-effort `eslint --fix` over an emitted candidate. The project's own
 * eslint installation (and therefore its flat config and plugins) wins; the
 * workspace's eslint is only a fallback for projects that share it. A file
 * eslint cannot parse, ignores, or would already fix before the edit is left
 * untouched. Each fix is additionally restricted to newly emitted text;
 * edits can make previously clean imports or other source newly fixable.
 */

type ESLint = import("eslint").ESLint;
type LintResult = import("eslint").ESLint.LintResult;

const instances = new Map<string, Promise<ESLint | null>>();

async function create(projectRoot: string): Promise<ESLint | null> {
  try {
    const entry = createRequire(join(projectRoot, "package.json")).resolve("eslint");
    const module = (await import(pathToFileURL(entry).href)) as { ESLint?: new (o: object) => ESLint };
    if (module.ESLint) return new module.ESLint({ cwd: projectRoot, fix: false });
  } catch {
    /* The project does not install eslint. */
  }
  try {
    const module = await import("eslint");
    return new module.ESLint({ cwd: projectRoot, fix: false });
  } catch {
    return null;
  }
}

function eslintFor(projectRoot: string): Promise<ESLint | null> {
  let instance = instances.get(projectRoot);
  if (instance === undefined) {
    instance = create(projectRoot);
    instances.set(projectRoot, instance);
  }
  return instance;
}

/** Drop resolved instances when the workspace may have changed (new roots, tests). */
export function resetSourceLintCache(): void {
  instances.clear();
}

async function lint(eslint: ESLint, fileName: string, text: string): Promise<LintResult | undefined> {
  try {
    return (await eslint.lintText(text, { filePath: fileName }))[0];
  } catch {
    return undefined;
  }
}

export async function lintSourceCandidate(
  projectRoot: string,
  fileName: string,
  before: string,
  candidate: string,
): Promise<string> {
  if (candidate === before) return candidate;
  const eslint = await eslintFor(projectRoot);
  if (!eslint) return candidate;
  const original = await lint(eslint, fileName, before);
  if (original === undefined) {
    instances.set(projectRoot, Promise.resolve(null));
    return candidate;
  }
  if (original.messages.some((message) => message.fix || message.fatal)) return candidate;
  let result = candidate;
  for (let pass = 0; pass < 10; pass++) {
    const checked = await lint(eslint, fileName, result);
    if (!checked || checked.messages.some((message) => message.fatal)) return candidate;
    if (!checked.messages.some((message) => message.fix)) break;
    const next = applyEmittedFixes(before, result, checked.messages);
    if (next === result) break;
    result = next;
  }
  return result;
}
