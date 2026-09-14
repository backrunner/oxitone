import { createHash, randomUUID } from "node:crypto";
import { link, lstat, mkdir, open, readFile, realpath, rename, unlink } from "node:fs/promises";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";
import {
  canonicalEncode,
  checkProtocolVersion,
  ErrorCode,
  OxitoneError,
  PROJECT_FORMAT_VERSION,
  projectFileSchema,
  projectSnapshotSchema,
  type ProjectSnapshot,
} from "@oxitone/protocol";
import { parseAuthoring } from "../authoring-validation.js";

export interface SaveProjectOptions {
  assetBaseDir?: string | undefined;
}
export interface LoadedProject {
  snapshot: ProjectSnapshot;
  assetBaseDir: string;
}
const FILE = "oxitone.project.json";
const hash = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex");

function failure(error: unknown, path: string): never {
  if (error instanceof OxitoneError) throw error;
  if (error instanceof SyntaxError)
    throw new OxitoneError(ErrorCode.InvalidProject, "malformed project JSON", { details: { path } });
  throw new OxitoneError(
    ErrorCode.AssetUnavailable,
    error instanceof Error ? error.message : "project file operation failed",
    { details: { path } },
  );
}

function directoryPath(directory: string): string {
  if (typeof directory !== "string" || directory.length === 0 || directory.includes("\0")) {
    throw new OxitoneError(ErrorCode.InvalidProject, "project directory must be a nonempty local path");
  }
  return resolve(directory);
}

function checkInside(root: string, path: string): void {
  const local = relative(root, path);
  if (local === ".." || local.startsWith(`..${sep}`) || isAbsolute(local)) {
    throw new OxitoneError(ErrorCode.InvalidProject, "asset resolves outside project directory");
  }
}

/** Temp + fsync + rename, followed by directory fsync. Never exposes partial manifest bytes. */
async function atomicWrite(path: string, text: string): Promise<void> {
  const temp = `${path}.${randomUUID()}.tmp`;
  try {
    const file = await open(temp, "wx");
    try {
      await file.writeFile(text, "utf8");
      await file.sync();
    } finally {
      await file.close();
    }
    await rename(temp, path);
    const dir = await open(dirname(path), "r");
    try {
      await dir.sync();
    } finally {
      await dir.close();
    }
  } finally {
    await unlink(temp).catch(() => {});
  }
}

function checkResourceReferences(snapshot: ProjectSnapshot): void {
  // Resources in the current ABI are Sample entity IDs, never arbitrary paths.
  const ids = new Set(snapshot.samples.map((sample) => sample.id));
  const plugins = snapshot.channels
    .flatMap((c) => [c.instrument, ...c.effectChain])
    .concat(snapshot.mixerChannels.flatMap((bus) => bus.inserts));
  for (const plugin of plugins)
    for (const id of Object.values(plugin.resources ?? {})) {
      if (!ids.has(id)) throw new OxitoneError(ErrorCode.InvalidProject, `unknown sample resource: ${id}`);
    }
}

/** Save a detached snapshot plus immutable content-addressed source assets. No audio decoding in JS. */
export async function saveProject(
  snapshot: ProjectSnapshot,
  directory: string,
  options: SaveProjectOptions = {},
): Promise<void> {
  const root = directoryPath(directory);
  try {
    const portable = parseAuthoring(projectSnapshotSchema, snapshot, "project");
    checkProtocolVersion(portable.protocolVersion);
    const sourceBase = options.assetBaseDir === undefined ? process.cwd() : directoryPath(options.assetBaseDir);
    checkResourceReferences(portable);
    await mkdir(root, { recursive: true });
    await mkdir(resolve(root, "assets"), { recursive: true });
    checkInside(await realpath(root), await realpath(resolve(root, "assets")));
    for (const sample of portable.samples) {
      const source = resolve(sourceBase, sample.assetUri);
      const bytes = await readFile(source);
      if (hash(bytes) !== sample.sha256)
        throw new OxitoneError(ErrorCode.AssetUnavailable, "sample content hash does not match descriptor", {
          details: { path: source },
        });
      const uri = `assets/${sample.sha256}.${sample.format}`;
      const target = resolve(root, uri);
      const temp = `${target}.${randomUUID()}.tmp`;
      try {
        // Publish without overwriting an existing content-addressed asset.
        const file = await open(temp, "wx");
        try {
          await file.writeFile(bytes);
          await file.sync();
        } finally {
          await file.close();
        }
        try {
          await link(temp, target);
        } catch (error) {
          if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
          if ((await lstat(target)).isSymbolicLink())
            throw new OxitoneError(ErrorCode.InvalidProject, "cached asset must not be a symbolic link");
          if (hash(await readFile(target)) !== sample.sha256)
            throw new OxitoneError(ErrorCode.AssetUnavailable, "existing cached asset has an unexpected hash", {
              details: { path: target },
            });
        }
      } finally {
        await unlink(temp).catch(() => {});
      }
      sample.assetUri = uri;
    }
    const assetDir = await open(resolve(root, "assets"), "r");
    try {
      await assetDir.sync();
    } finally {
      await assetDir.close();
    }
    await atomicWrite(
      resolve(root, FILE),
      canonicalEncode(
        projectFileSchema.parse({
          ...portable,
          formatVersion: PROJECT_FORMAT_VERSION,
          projectId: portable.id,
        }),
      ),
    );
  } catch (error) {
    failure(error, root);
  }
}

/** Read a portable project, validate versions/asset hashes, and retain relative asset paths. */
export async function loadProject(directory: string): Promise<LoadedProject> {
  const root = directoryPath(directory);
  try {
    const raw: unknown = JSON.parse(await readFile(resolve(root, FILE), "utf8"));
    if (
      typeof raw !== "object" ||
      raw === null ||
      !("formatVersion" in raw) ||
      raw.formatVersion !== PROJECT_FORMAT_VERSION
    ) {
      throw new OxitoneError(ErrorCode.ProtocolVersionUnsupported, "unsupported project formatVersion");
    }
    if (!("protocolVersion" in raw)) throw new OxitoneError(ErrorCode.InvalidProject, "missing protocolVersion");
    checkProtocolVersion(String(raw.protocolVersion));
    const parsed = projectFileSchema.safeParse(raw);
    if (!parsed.success)
      throw new OxitoneError(ErrorCode.InvalidProject, "invalid project manifest", { details: { path: FILE } });
    const snapshot = projectSnapshotSchema.parse(parsed.data);
    checkResourceReferences(snapshot);
    const actualRoot = await realpath(root);
    for (const sample of snapshot.samples) {
      if (isAbsolute(sample.assetUri) || sample.assetUri.includes("\\") || sample.assetUri.split("/").includes("..")) {
        throw new OxitoneError(ErrorCode.InvalidProject, "portable assets must use relative paths inside the project");
      }
      const asset = await realpath(resolve(root, sample.assetUri));
      checkInside(actualRoot, asset);
      if (hash(await readFile(asset)) !== sample.sha256)
        throw new OxitoneError(ErrorCode.AssetUnavailable, "sample content hash does not match manifest", {
          details: { path: asset },
        });
    }
    return { snapshot, assetBaseDir: root };
  } catch (error) {
    failure(error, root);
  }
}
