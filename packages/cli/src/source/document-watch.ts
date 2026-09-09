import { watch, type FSWatcher } from "node:fs";
import { dirname, resolve } from "node:path";
interface WatchedDocument {
  readonly view: { readonly status: string; readonly saving?: boolean };
  readonly dependencyPaths: readonly string[];
  subscribe(listener: () => void): () => void;
  synchronizeDisk(): Promise<unknown>;
}

/** Watch parent directories so atomic editor saves and package file replacements retain their subscriptions. */
export function watchProjectDocument(document: WatchedDocument, fileName: string, onError: (error: unknown) => void): () => void {
  const watchers = new Map<string, { watcher: FSWatcher; files: Set<string> }>();
  let timer: NodeJS.Timeout | undefined;
  let pending = false;
  let active = false;
  let closed = false;
  const schedule = () => {
    if (closed || active || !pending || document.view.saving || document.view.status === "building") return;
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = undefined;
      if (closed || document.view.saving || document.view.status === "building") return;
      pending = false; active = true;
      void document.synchronizeDisk().catch(onError).finally(() => { active = false; schedule(); });
    }, 120);
  };
  const unsubscribe = document.subscribe(() => {
    const view = document.view;
    if (closed || view.status === "closed") return;
    const directories = new Map<string, Set<string>>();
    for (const input of [fileName, ...document.dependencyPaths]) {
      const path = resolve(input), directory = dirname(path);
      if (!directories.has(directory)) directories.set(directory, new Set());
      directories.get(directory)!.add(path);
    }
    for (const [directory, current] of watchers) if (!directories.has(directory)) { current.watcher.close(); watchers.delete(directory); }
    for (const [directory, files] of directories) {
      const current = watchers.get(directory);
      if (current) { current.files = files; continue; }
      try {
        const watcher = watch(directory, (_, name) => {
          if (name !== null && !watchers.get(directory)?.files.has(resolve(directory, String(name)))) return;
          pending = true; schedule();
        });
        watcher.on("error", onError);
        watchers.set(directory, { watcher, files });
      } catch (error) { onError(error); }
    }
    schedule();
  });
  return () => {
    closed = true; unsubscribe(); if (timer) clearTimeout(timer);
    for (const { watcher } of watchers.values()) watcher.close(); watchers.clear();
  };
}
