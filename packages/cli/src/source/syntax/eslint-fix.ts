import { createRequire } from "node:module";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

/**
 * Best-effort `eslint --fix` over an emitted candidate. The project's own
 * eslint installation (and therefore its flat config and plugins) wins; the
 * workspace's eslint is only a fallback for projects that share it. A file
 * eslint cannot parse, ignores, or would already fix before the edit is left
 * untouched, so fixes stay confined to the emitted span.
 */

type ESLint = import("eslint").ESLint;
type LintResult = import("eslint").ESLint.LintResult;

const instances = new Map<string, Promise<ESLint | null>>();

async function create(projectRoot: string): Promise<ESLint | null> {
  try {
    const entry = createRequire(join(projectRoot, "package.json")).resolve("eslint");
    const module = (await import(pathToFileURL(entry).href)) as { ESLint?: new (o: object) => ESLint };
    if (module.ESLint) return new module.ESLint({ cwd: projectRoot, fix: true });
  } catch {
    /* The project does not install eslint. */
  }
  try {
    const module = await import("eslint");
    return new module.ESLint({ cwd: projectRoot, fix: true });
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
  if (original.output !== undefined || original.messages.some((message) => message.fatal)) return candidate;
  return (await lint(eslint, fileName, candidate))?.output ?? candidate;
}
