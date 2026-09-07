import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { beatToWire, encodeProjectSnapshot } from "@oxitone/protocol";
import { compile, createEngine, dispose, exportMidi, getPluginDiagnostics, inspectSample, registerPlugin, renderWav } from "oxitone";
import { hashFile } from "../verify.js";
import { dubstep, createDubstepSong } from "./melodic-dubstep.js";
import { outputRoot } from "./paths.js";
import { drumRegistration, type Section } from "./shared.js";
import { inspectSections } from "./verify.js";
import { midiSnapshot } from "./midi-export.js";
import { inspectImpact } from "./impact-analysis.js";

await mkdir(outputRoot, { recursive: true });
const reports = [];
for (const [song, create] of [[dubstep, createDubstepSong]] as const) {
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
    assert(file.truePeakDbfs < -0.8 && file.peakDbfs > -6, "Unexpected song peak");
    assert(file.integratedLufs > -18 && file.integratedLufs < -9, "Unexpected song loudness");
    const wav = inspectSample(file.path);
    assert.equal(wav.sampleRate, 48000); assert.equal(wav.channels, 2);
    assert(Math.abs(Number(wav.frames) / wav.sampleRate - expectedSeconds) < 0.01);
    const sectionLevels = inspectSections(file.path, song.sections, song.bpm, song.bars);
    assert(sectionLevels.every(s => s.rmsDbfs > -52), "Silent arrangement section");
    assert(sectionLevels.every(s => Math.abs(s.dc) < 0.002 && s.stereoCorrelation > 0), "DC or stereo cancellation regression");
    for (const index of [2, 5]) {
      const drop = sectionLevels[index]!;
      assert(drop.crestDb > 6, "Drop transients over-compressed");
      assert(drop.lowSideToMidDb < -20, "Drop bass lost its mono foundation");
      assert(drop.rmsDbfs > sectionLevels[index - 1]!.rmsDbfs + 2, "Build no longer lifts into drop");
      assert(drop.rmsDbfs < sectionLevels[index - 1]!.rmsDbfs + 8, "Build is too quiet relative to drop");
    }
    const phrases: Section[] = Array.from({ length: song.bars / 4 }, (_, i) => [`Phrase ${i + 1}`, i * 4]);
    const phraseLevels = inspectSections(file.path, phrases, song.bpm, song.bars);
    const impacts = inspectImpact(file.path, song.bpm);
    assert(impacts.every(i => i.arrivalLiftDb! > 12), "Pre-drop gap no longer clears the downbeat");
    assert(impacts.every(i => i.snareMedianMidLiftDb > 0), "Snare sustain is buried in the drop mix");
    const diagnostics = getPluginDiagnostics(engine);
    assert(diagnostics.every(d => d.faults === 0), "Native plugin fault");
    const midi = exportMidi(engine, midiSnapshot(snapshot), { path: join(outputRoot, `${song.slug}.mid`) });
    await writeFile(join(outputRoot, `${song.slug}.snapshot.json`), encodeProjectSnapshot(snapshot));
    reports.push({ ...song, ...file, sha256: hashFile(file.path), renderSeconds,
      realtimeMultiple: expectedSeconds / renderSeconds, tracks: snapshot.tracks.length,
      notes: snapshot.patterns.reduce((sum, pattern) => sum + pattern.notes.length, 0),
      sectionLevels, phraseLevels, impacts, midi, plugin, diagnostics });
    console.log(`${song.title}: ${file.durationSeconds.toFixed(2)} s, ${file.integratedLufs.toFixed(2)} LUFS, ${file.truePeakDbfs.toFixed(2)} dBTP`);
  } finally { dispose(engine); }
}
await writeFile(join(outputRoot, "report.json"), `${JSON.stringify({ sampleRate: 48000, blockSize: 128,
  instruments: "Oxitone synthesis / Circuit electronic drums / Salamander Grand Piano v3 (Alexander Holm, CC BY 3.0)", songs: reports }, null, 2)}\n`);
