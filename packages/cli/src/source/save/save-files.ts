import { randomUUID } from "node:crypto";
import { link, lstat, mkdir, open, readFile, rename, rm } from "node:fs/promises";
import { dirname, join } from "node:path";
import { ErrorCode, OxitoneError } from "@oxitone/protocol";

export function saveConflict(message: string): never {
  throw new OxitoneError(ErrorCode.SourceChanged, message);
}
export async function syncDirectory(path: string): Promise<void> {
  const handle = await open(path, "r");
  try {
    await handle.sync();
  } finally {
    await handle.close();
  }
}
export async function ordinaryDirectory(path: string): Promise<void> {
  await mkdir(path, { recursive: true, mode: 0o700 });
  if (!(await lstat(path)).isDirectory()) saveConflict("save directory must not be a symbolic link");
}
export async function durableReplace(path: string, text: string, mode = 0o600): Promise<void> {
  const temporary = join(dirname(path), `.oxitone-save-${randomUUID()}.tmp`);
  try {
    const handle = await open(temporary, "wx", mode);
    try {
      await handle.writeFile(text);
      await handle.chmod(mode);
      await handle.sync();
    } finally {
      await handle.close();
    }
    await rename(temporary, path);
    await syncDirectory(dirname(path));
  } finally {
    await rm(temporary, { force: true });
  }
}

/** Publish a complete file without overwriting a concurrently created target. */
export async function durableCreate(path: string, text: string, mode: number, temporary: string): Promise<void> {
  let created = false;
  try {
    const handle = await open(temporary, "wx", mode);
    created = true;
    try {
      await handle.writeFile(text);
      await handle.chmod(mode);
      await handle.sync();
    } finally {
      await handle.close();
    }
    try {
      await link(temporary, path);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "EEXIST") saveConflict("new source appeared during Save");
      throw error;
    }
  } finally {
    if (created) {
      await rm(temporary, { force: true });
      await syncDirectory(dirname(path));
    }
  }
}

/** Cooperative writer lock. Dead owners may be recovered once; ambiguous ownership fails closed. */
export async function withSaveLock<T>(directory: string, action: () => Promise<T>): Promise<T> {
  const lock = join(directory, "lock");
  let acquired = false;
  try {
    try {
      await mkdir(lock, { mode: 0o700 });
      acquired = true;
      await durableReplace(join(lock, "owner.json"), JSON.stringify({ pid: process.pid }));
      await syncDirectory(directory);
    } catch (error) {
      if (acquired || (error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
      if (!(await lstat(lock)).isDirectory()) saveConflict("save lock is not a directory");
      let pid: number;
      try {
        pid = (JSON.parse(await readFile(join(lock, "owner.json"), "utf8")) as { pid: number }).pid;
      } catch {
        saveConflict("save lock owner is unknown; preserve recovery files for inspection");
      }
      if (!Number.isSafeInteger(pid) || pid <= 0) saveConflict("invalid save lock owner");
      try {
        process.kill(pid, 0);
        saveConflict("another document service holds the project save lock");
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== "ESRCH") throw error;
      }
      // The claim stays inside the old lock so a second opener cannot steal a recovery in progress.
      try {
        await mkdir(join(lock, "recovering"));
      } catch {
        saveConflict("source recovery is already claimed; preserve the journal");
      }
      acquired = true;
      await durableReplace(join(lock, "owner.json"), JSON.stringify({ pid: process.pid }));
    }
    return await action();
  } finally {
    if (acquired) {
      await rm(lock, { recursive: true, force: true });
      await syncDirectory(directory);
    }
  }
}
