import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { beatToWire, encodeProjectSnapshot } from "@oxitone/protocol";
import { compile, createEngine, dispose, exportMidi, getPluginDiagnostics, inspectSample, registerPlugin, renderWav } from "oxitone";
import { hashFile } from "../verify.js";
import { lofi, createLofiSong } from "./lofi.js";
import { dubstep, createDubstepSong } from "./melodic-dubstep.js";
import { outputRoot } from "./piano.js";
import { drumRegistration } from "./shared.js";
import { inspectSections } from "./verify.js";
import piano from "./piano-assets.json" with { type: "json" };

await mkdir(outputRoot, { recursive: true });
const reports = [];
for (const [song, create] of [[lofi, createLofiSong], [dubstep, createDubstepSong]] as const) {
  const engine = createEngine({ allowPlugins: "any" });
  try {
    const plugin = registerPlugin(engine, drumRegistration());
    const project = create(), snapshot = project.snapshot();
    compile(engine, snapshot);
    const started = performance.now();
    const result = renderWav(engine, snapshot, { path: join(outputRoot, `${song.slug}.wav`),
      end: { beat: beatToWire(song.bars * 4) }, tailSeconds: 3, bitDepth: 24, dither: "tpdf" });
    const renderSeconds = (performance.now() - started) / 1000;
    const file = result.files[0]!, expectedSeconds = song.bars * 4 * 60 / song.bpm + 3;
    assert(Math.abs(file.durationSeconds - expectedSeconds) < 0.01, "Incorrect song duration");
    assert(file.truePeakDbfs < -0.5 && file.peakDbfs > -24, "Unexpected song peak");
    assert(file.integratedLufs > -32 && file.integratedLufs < -6, "Unexpected song loudness");
    const wav = inspectSample(file.path);
    assert.equal(wav.sampleRate, 48000); assert.equal(wav.channels, 2);
    assert(Math.abs(Number(wav.frames) / wav.sampleRate - expectedSeconds) < 0.01);
    const sectionLevels = inspectSections(file.path, song.sections, song.bpm, song.bars);
    assert(sectionLevels.every(s => s.rmsDbfs > -52), "Silent arrangement section");
    const diagnostics = getPluginDiagnostics(engine);
    assert(diagnostics.every(d => d.faults === 0), "Native plugin fault");
    const midi = exportMidi(engine, snapshot, { path: join(outputRoot, `${song.slug}.mid`) });
    await writeFile(join(outputRoot, `${song.slug}.snapshot.json`), encodeProjectSnapshot(snapshot));
    reports.push({ ...song, ...file, sha256: hashFile(file.path), renderSeconds,
      realtimeMultiple: expectedSeconds / renderSeconds, tracks: snapshot.tracks.length,
      notes: snapshot.patterns.reduce((sum, pattern) => sum + pattern.notes.length, 0),
      sectionLevels, midi, plugin, diagnostics });
    console.log(`${song.title}: ${file.durationSeconds.toFixed(2)} s, ${file.integratedLufs.toFixed(2)} LUFS, ${file.truePeakDbfs.toFixed(2)} dBTP`);
  } finally { dispose(engine); }
}
await writeFile(join(outputRoot, "report.json"), `${JSON.stringify({ sampleRate: 48000, blockSize: 128,
  piano: { source: piano.source, commit: piano.commit, license: piano.license, regions: piano.files.length,
    keyRange: [39, 91], velocityLayers: [[1, 50], [51, 88], [89, 127]] }, songs: reports }, null, 2)}\n`);
