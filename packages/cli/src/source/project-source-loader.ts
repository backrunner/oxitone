import { realpath, stat } from "node:fs/promises";
import { basename, dirname, extname, join, resolve } from "node:path";
import { ErrorCode, OxitoneError } from "@oxitone/protocol";
import type { ProjectSourceLoader } from "../bundle.js";
import type { SourceOwnership } from "./ownership.js";
import { captureMissingSource, captureSourceReads, type SourceRead } from "./read-set.js";
import { instrumentProjectSource, type SourceSite } from "./project-instrument.js";
import { readSourceText } from "./project-files.js";
import { sourceHash } from "./syntax.js";

/** Enrolled text overlays and negative read evidence for modules not saved yet. */
export async function projectSourceLoader(files: ReadonlyMap<string, string>, ownership: SourceOwnership,
  key: string, sites: SourceSite[], reads: Map<string, SourceRead>, signal: AbortSignal,
  removed: readonly string[]): Promise<ProjectSourceLoader> {
  const owned = new Map<string, { path: string; text: string }>();
  const removedPaths = new Set<string>();
  for (const path of removed) removedPaths.add((await ownership.assertWritable(path)).realPath);
  for (const [path, text] of files) {
    const source = await ownership.assertWritable(path);
    owned.set(source.realPath, { path, text });
  }
  return {
    async resolveLocal(specifier, directory, diskPath) {
      const target = resolve(directory, specifier), suffix = extname(target);
      const extensions = [".tsx", ".ts", ".jsx", ".js", ".css", ".json"];
      const candidates = suffix === ".js" ? [".ts", ".tsx", ".js", ".jsx"].map(ext => target.slice(0, -3) + ext)
        : suffix === ".mjs" ? [target.slice(0, -4) + ".mts", target]
        : suffix ? [target] : [target, ...extensions.map(ext => target + ext)];
      const indexes = suffix ? [] : extensions.map(ext => join(target, "index" + ext));
      // Directory package entry resolution comes after file resolution and before index fallback.
      if (diskPath && !candidates.includes(diskPath) && !indexes.includes(diskPath)) candidates.push(diskPath);
      candidates.push(...indexes);
      for (const path of candidates) {
        let canonical: string;
        try { canonical = join(await realpath(dirname(path)), basename(path)); }
        catch (error) { if ((error as NodeJS.ErrnoException).code === "ENOENT") continue; throw error; }
        if (removedPaths.has(canonical)) continue;
        if (canonical === diskPath) return undefined;
        if (owned.has(canonical)) return canonical;
        if (diskPath && removedPaths.has(diskPath)) {
          try { if ((await stat(path)).isFile()) return await realpath(path); }
          catch (error) { if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error; }
        }
      }
      return undefined;
    },
    async read(path) {
      signal.throwIfAborted();
      if (removedPaths.has(path)) throw new OxitoneError(ErrorCode.EditTargetMissing, "import refers to a source removed in the draft");
      let evidence: SourceRead;
      try { [evidence] = await captureSourceReads([path]) as [SourceRead]; }
      catch (error) {
        if ((error as NodeJS.ErrnoException).code !== "ENOENT" || !owned.has(path)) throw error;
        evidence = await captureMissingSource(path);
      }
      reads.set(path, evidence);
      const file = owned.get(evidence.realPath);
      if (evidence.sha256 !== null) {
        const disk = await readSourceText(path);
        if (sourceHash(disk) !== evidence.sha256) throw new OxitoneError(ErrorCode.SourceChanged, "source changed during project build");
        if (!file) return disk;
      }
      if (!file) throw new OxitoneError(ErrorCode.EditTargetMissing, "missing source is not enrolled");
      const instrumented = instrumentProjectSource(file.path, file.text, key, sites.length);
      sites.push(...instrumented.sites); return instrumented.text;
    },
  };
}
