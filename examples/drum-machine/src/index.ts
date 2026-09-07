import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { Project } from "@oxitone/core";
import { beatToWire, encodeProjectSnapshot } from "@oxitone/protocol";
import { compile, createEngine, dispose, exportMidi, getPluginDiagnostics, registerPlugin, renderWav } from "oxitone";
import { buildPlugins } from "./plugins.js";
import { createDrumSong } from "./song.js";
import { hashFile, verifyPlugins } from "./verify.js";

const output = resolve(process.argv[2] ?? fileURLToPath(new URL("../../../target/examples/drum-machine", import.meta.url)));
await mkdir(join(output, "verification"), { recursive: true });
const engine = createEngine({ allowPlugins: "any" });
try {
  const plugins = (await buildPlugins(join(output, "plugins"))).map((options) => registerPlugin(engine, options));
  const verification = verifyPlugins(engine, join(output, "verification"));
  const project = createDrumSong();
  const snapshot = project.snapshot();
  compile(engine, snapshot); // Clear verification host events before rendering the song.
  const options = { end: { beat: beatToWire(64) }, tailSeconds: 2, bitDepth: 24 as const, dither: "tpdf" as const };
  const mix = renderWav(engine, snapshot, { ...options, path: join(output, "midnight-circuit.wav") });
  assert(mix.files[0]!.truePeakDbfs < -1 && mix.files[0]!.peakDbfs > -30, "Unexpected song level");
  const midi = exportMidi(engine, snapshot, { path: join(output, "midnight-circuit.mid") });
  await writeFile(join(output, "midnight-circuit.snapshot.json"), encodeProjectSnapshot(snapshot));
  await project.save(join(output, "project"));
  const restored = await Project.load(join(output, "project"));
  renderWav(engine, restored.snapshot(), { ...options, path: join(output, "midnight-circuit-restored.wav") });
  const sha256 = hashFile(mix.files[0]!.path);
  assert.equal(hashFile(join(output, "midnight-circuit-restored.wav")), sha256, "Restored song changed audio");
  const drums = structuredClone(snapshot);
  for (const track of drums.tracks.slice(1)) track.enabled = false;
  const drumSolo = renderWav(engine, drums, { ...options, path: join(output, "drums-only.wav") });
  const diagnostics = getPluginDiagnostics(engine);
  assert(diagnostics.every((plugin) => plugin.faults === 0));
  const report = { title: "Midnight Circuit / 午夜回路", bpm: 112, bars: 16, sampleRate: 48000,
    blockSize: 128, plugins, verification, mix, drumSolo, midi, sha256, restoredIdentical: true, diagnostics };
  await writeFile(join(output, "report.json"), `${JSON.stringify(report, null, 2)}\n`);
  console.log(JSON.stringify(report, null, 2));
} finally {
  dispose(engine);
}
