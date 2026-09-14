import { lstat, realpath, stat } from "node:fs/promises";
import { basename, dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { ErrorCode, OxitoneError } from "@oxitone/protocol";

const excluded = new Set([
  "node_modules",
  ".pnpm",
  ".yarn",
  ".git",
  "dist",
  "target",
  "build",
  "native-generated",
  ".oxitone-source-save",
]);
const segments = (path: string) => path.split(/[\\/]/).map((part) => part.toLowerCase());
const inside = (root: string, path: string) => {
  const rel = relative(root, path);
  return rel !== "" && rel !== ".." && !rel.startsWith(`..${sep}`) && !isAbsolute(rel);
};

export interface OwnedSource {
  readonly path: string;
  readonly realPath: string;
}

function reject(path: string, reason: string): never {
  throw new OxitoneError(ErrorCode.EditNotRepresentable, reason, { details: { path, reason: "SourceReadOnly" } });
}

/** Explicit project file ownership, not an OS security sandbox. Does not grant writes to dependencies. */
export class SourceOwnership {
  private readonly files = new Map<string, OwnedSource>();
  private readonly created = new Set<string>();
  private constructor(private readonly roots: readonly OwnedSource[]) {}

  static async open(sourceRoots: readonly string[], files: readonly string[]): Promise<SourceOwnership> {
    if (sourceRoots.length === 0)
      throw new OxitoneError(ErrorCode.InvalidProject, "at least one source root is required");
    const roots: OwnedSource[] = [];
    for (const input of sourceRoots) {
      const path = resolve(input);
      const realPath = await realpath(path);
      if (segments(path).some((part) => excluded.has(part)) || segments(realPath).some((part) => excluded.has(part)))
        reject(path, "dependency or generated directory cannot be a source root");
      if (!(await stat(realPath)).isDirectory()) reject(path, "source root must be a directory");
      roots.push({ path, realPath });
    }
    const ownership = new SourceOwnership(roots);
    for (const input of files) {
      const file = await ownership.inspect(input);
      if ([...ownership.files.values()].some((other) => other.realPath === file.realPath))
        reject(file.path, "duplicate aliases of one source file are not editable");
      ownership.files.set(file.path, Object.freeze(file));
    }
    return ownership;
  }

  private async inspect(input: string): Promise<OwnedSource> {
    const path = resolve(input);
    if (!/\.(?:ts|mts)$/.test(path) || /\.d\.(?:ts|mts)$/.test(path))
      reject(path, "only TypeScript ESM authoring files are editable");
    if (segments(path).some((part) => excluded.has(part))) reject(path, "dependency and generated paths are read-only");
    const realPath = await realpath(path);
    if (segments(realPath).some((part) => excluded.has(part)))
      reject(path, "source resolves into a dependency or generated directory");
    const root = this.roots
      .filter((candidate) => inside(candidate.path, path) && inside(candidate.realPath, realPath))
      .sort((a, b) => b.path.length - a.path.length)[0];
    if (!root) reject(path, "source is outside the enrolled project roots");
    if ((await realpath(root.path)) !== root.realPath) reject(path, "source root was relocated");
    const info = await stat(realPath);
    if (!info.isFile() || info.nlink !== 1) reject(path, "source must be an ordinary file without hard-link aliases");
    // A nested package must be separately enrolled as a source root, even under a monorepo root.
    for (let parent = dirname(realPath); parent !== root.realPath; parent = dirname(parent)) {
      try {
        await lstat(resolve(parent, "package.json"));
        reject(path, "nested package is not an enrolled source root");
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
      }
    }
    return { path, realPath };
  }

  /** Recheck the actual path before accepting a candidate and again before eventual publication. */
  async assertRoot(input: string): Promise<string> {
    const path = resolve(input);
    const root = this.roots.find((candidate) => candidate.path === path);
    if (!root || (await realpath(path)) !== root.realPath) reject(path, "save root is not an enrolled project root");
    return root.realPath;
  }

  /** Recheck the actual path before accepting a candidate and again before eventual publication. */
  async assertWritable(input: string): Promise<OwnedSource> {
    const path = resolve(input);
    const owned = this.files.get(path);
    if (!owned) reject(path, "file is not enrolled in this document session");
    if (this.created.has(path)) {
      const current = await this.assertCreationTarget(path);
      if (current.realPath !== owned.realPath) reject(path, "source parent changed since enrollment");
      return { ...owned };
    }
    const current = await this.inspect(path);
    if (current.realPath !== owned.realPath) reject(path, "source link target changed since enrollment");
    return { ...owned };
  }

  /** Enroll a validated new path so an unsaved module can already participate in evaluation. */
  async enroll(input: string, assertCurrent: () => void = () => {}): Promise<OwnedSource> {
    const owned = await this.assertCreatable(input);
    assertCurrent();
    if ([...this.files.values()].some((file) => file.path !== owned.path && file.realPath === owned.realPath))
      reject(input, "duplicate source aliases are not editable");
    this.files.set(owned.path, Object.freeze(owned));
    this.created.add(owned.path);
    return { ...owned };
  }

  /** Validate a new authoring path without enrolling a dependency or symlink. */
  async assertCreatable(input: string): Promise<OwnedSource> {
    const owned = await this.assertCreationTarget(input);
    if (this.files.has(owned.path) && !this.created.has(owned.path))
      reject(input, "source already belongs to this document");
    try {
      await lstat(owned.path);
      reject(input, "source file already exists");
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
    }
    return owned;
  }

  /** Recovery validates existing sources against the enrolled roots before discovery reads text. */
  async assertRecoverySource(input: string): Promise<OwnedSource> {
    return this.inspect(input);
  }

  /** Revalidate a creation/deletion journal target, including after a service restart. */
  async assertCreationTarget(input: string): Promise<OwnedSource> {
    const owned = await this.assertCreationParent(input);
    try {
      const info = await lstat(owned.path);
      if (!info.isFile() || info.nlink !== 1)
        reject(owned.path, "created source must be an ordinary file without links");
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
    }
    return owned;
  }

  /** Validate a staging directory without following a possibly interrupted publication link. */
  async assertCreationParent(input: string): Promise<OwnedSource> {
    const path = resolve(input);
    if (!/\.(?:ts|mts)$/.test(path) || /\.d\.(?:ts|mts)$/.test(path))
      reject(path, "only TypeScript ESM authoring files are editable");
    if (segments(path).some((part) => excluded.has(part))) reject(path, "dependency and generated paths are read-only");
    const root = this.roots
      .filter((candidate) => inside(candidate.path, path))
      .sort((a, b) => b.path.length - a.path.length)[0];
    if (!root || (await realpath(root.path)) !== root.realPath)
      reject(path, "source is outside the enrolled project roots");
    const parent = resolve(dirname(path));
    const parentReal = await realpath(parent);
    if (
      segments(parentReal).some((part) => excluded.has(part)) ||
      (parentReal !== root.realPath && !inside(root.realPath, parentReal))
    )
      reject(path, "new source parent is outside the enrolled project root");
    for (let directory = parentReal; directory !== root.realPath; directory = dirname(directory)) {
      try {
        await lstat(join(directory, "package.json"));
        reject(path, "nested package is not an enrolled source root");
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
      }
    }
    return { path, realPath: join(parentReal, basename(path)) };
  }
}
