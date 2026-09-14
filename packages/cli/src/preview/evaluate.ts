/** Runs in a fresh process on every build. Audio generation never occurs here. */
import { writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { projectSnapshotSchema, PREVIEW_MAX_FRAME_BYTES } from "@oxitone/protocol";

try {
  const entry = resolve(process.argv[2]!);
  const source = resolve(process.argv[3] ?? entry);
  const module = await import(pathToFileURL(entry).href);
  const exported = module.default ?? module.createProject ?? module.project;
  const value = await (typeof exported === "function" ? exported() : exported);
  const project = value?.project ?? value;
  if (!project || typeof project.snapshot !== "function")
    throw new Error("Preview entry must export a Project or a factory returning one");
  const snapshot = projectSnapshotSchema.parse(project.snapshot());
  const result = JSON.stringify({
    snapshot,
    assetBaseDir: resolve(
      value?.assetBaseDir ?? project.assetBaseDir ?? module.__oxitoneSourceDirectory ?? dirname(source),
    ),
    plugins: project.registeredPlugins ?? [],
    allowPlugins: project.pluginPolicy,
    pluginUis: project.registeredPluginUis ?? [],
  });
  if (Buffer.byteLength(result) > PREVIEW_MAX_FRAME_BYTES) throw new Error("Preview project exceeds 64 MiB");
  writeFileSync(3, result);
} catch (error) {
  console.error(error instanceof Error ? error.stack : String(error));
  process.exitCode = 1;
}
