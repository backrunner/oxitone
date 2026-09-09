import { createHash } from "node:crypto";
import { existsSync, readFileSync, realpathSync } from "node:fs";
import * as moduleHooks from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import type { SourceRead } from "./read-set.js";

export function trackModuleReads(bundle: string): { reads: Map<string, SourceRead>; close(): void } {
  if (typeof moduleHooks.registerHooks !== "function") throw new Error("source evaluation requires Node 22.15+ or 24+");
  const reads = new Map<string, SourceRead>();
  const directories = new Set<string>();
  function add(read: SourceRead): void {
    if (reads.size >= 4096) throw new Error("source dependency budget exceeded");
    reads.set(read.path, read);
  }
  function record(path: string): void {
    if (reads.has(path)) return;
    add({ path, realPath: realpathSync(path), sha256: createHash("sha256").update(readFileSync(path)).digest("hex") });
  }
  function manifests(path: string): void {
    for (let parent = dirname(path); ; parent = dirname(parent)) {
      if (directories.has(parent)) break;
      directories.add(parent);
      const realParent = realpathSync(parent);
      for (const name of ["package.json", "pnpm-lock.yaml", "package-lock.json", "yarn.lock"]) {
        const candidate = join(parent, name);
        if (existsSync(candidate)) record(candidate);
        // Absence is evidence too: a package/lock appearing after this scan must reject the candidate.
        // The parent removes its private bundle directory after evaluation.
        else if (parent !== dirname(bundle)) add({ path: candidate, realPath: join(realParent, name), sha256: null });
      }
      if (parent === dirname(parent)) break;
    }
  }
  const hooks = moduleHooks.registerHooks({
    resolve(specifier, context, next) {
      if (context.parentURL?.startsWith("file:")) manifests(fileURLToPath(context.parentURL));
      const result = next(specifier, context);
      if (result.url.startsWith("file:")) manifests(fileURLToPath(result.url));
      return result;
    },
    load(url, context, next) {
      if (url.startsWith("file:") && fileURLToPath(url) !== bundle) record(fileURLToPath(url));
      return next(url, context);
    },
  });
  return { reads, close: () => hooks.deregister() };
}
