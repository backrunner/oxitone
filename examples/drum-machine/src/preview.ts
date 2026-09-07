import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { pluginManifestSchema } from "@oxitone/protocol";
import { createDrumSong } from "./song.js";
import { drumPanel, gainPanel } from "./plugin-panels.js";

/** Build libraries once with pnpm example:drums, then watch this authoring entry. */
export default function createProject() {
  const project = createDrumSong();
  const root = new URL("../../../", import.meta.url);
  const suffix = process.platform === "darwin" ? ".dylib" : ".so";
  for (const [library, manifest] of [
    [`target/release/liboxitone_example_drums${suffix}`, "crates/example-drums/manifest.json"],
    [`target/examples/drum-machine/plugins/reference-gain${suffix}`, "crates/render/tests/fixtures/gain.json"],
  ]) {
    project.registerPlugin({ libraryPath: fileURLToPath(new URL(library!, root)),
      manifest: pluginManifestSchema.parse(JSON.parse(readFileSync(new URL(manifest!, root), "utf8"))) },
    { allowPlugins: "any" }); // Explicit trust for these two local development fixtures.
  }
  return project.registerPluginUi(drumPanel).registerPluginUi(gainPanel);
}
