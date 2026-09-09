import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { promisify } from "node:util";
import { ProjectDocument } from "../packages/cli/dist/source/index.js";

const effects = ["eq", "filter", "nonlinear-filter", "compressor", "gate", "limit", "limiter",
  "clipper", "saturator", "distortion", "multiband", "multiband-dynamics", "compactor", "delay",
  "reverb", "convolver", "resonator", "chorus", "flanger", "phaser", "tape", "frequency-shifter",
  "pitch-shifter", "bitcrush", "spreader", "utility"];
const instruments = ["wavetable", "sampler", "multisampler", "slicer"];
const args = process.argv.slice(2);
const editing = args.includes("--edit");
const theme = args.includes("--light") ? "light" : "dark";
const narrow = args.includes("--narrow");
const only = args.find(a => a.startsWith("--only="))?.slice(7).split(",");
const page = args.find(a => a.startsWith("--page="))?.slice(7);
const cases = editing ? ["filter"] : (only ?? [...effects, ...instruments]);
for (const name of cases) if (![...effects, ...instruments].includes(name)) throw new Error("Unknown builtin " + name);
if (page && (editing || cases.some(name => name !== "wavetable") || !["sound", "shaping", "modulation", "matrix"].includes(page))) throw new Error("Select a Wavetable page with --only=wavetable");
const output = resolve("target/builtin-panels", editing ? "editing" : theme + (narrow ? "-narrow" : "") + (page ? "-" + page : ""));
await mkdir(output, { recursive: true });
const root = await mkdtemp(join(tmpdir(), "oxitone-builtin-panels-"));
try {
  await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
  await symlink(resolve("packages/core"), join(root, "node_modules/@oxitone/core"), "dir");
  // A deterministic offline fixture. Capture mode always uses the native simulated sink.
  const frames = 48000;
  const wav = Buffer.alloc(44 + frames * 2);
  wav.write("RIFF", 0); wav.writeUInt32LE(wav.length - 8, 4); wav.write("WAVEfmt ", 8);
  wav.writeUInt32LE(16, 16); wav.writeUInt16LE(1, 20); wav.writeUInt16LE(1, 22);
  wav.writeUInt32LE(48000, 24); wav.writeUInt32LE(96000, 28); wav.writeUInt16LE(2, 32);
  wav.writeUInt16LE(16, 34); wav.write("data", 36); wav.writeUInt32LE(frames * 2, 40);
  for (let i = 0; i < frames; i++) wav.writeInt16LE(Math.round(8000 * Math.sin(i / 48000 * Math.PI * 440) * Math.exp(-i / 16000)), 44 + i * 2);
  await writeFile(join(root, "tone.wav"), wav);
  const hash = createHash("sha256").update(wav).digest("hex");
  for (const name of cases) {
    const instrument = instruments.includes(name);
    const entry = join(root, name + ".ts");
    const outputFile = join(output, name + ".png");
    await rm(outputFile, { force: true });
    const selected = name === "sampler" ? "sampler(sample, { rootKey: 60, amp: { attack: 0.005, decay: 0.25, sustain: 0.6, release: 0.4 } })"
      : name === "multisampler" ? "multisampler([{ sample, rootKey: 48, keyRange: [36, 59], velocityRange: [1, 64] }, { sample, rootKey: 60, keyRange: [36, 59], velocityRange: [65, 127] }, { sample, rootKey: 72, keyRange: [60, 84] }])"
      : name === "slicer" ? "slicer(sample, { slices: { grid: 8 }, triggerNote: 36 })" : "wavetable()";
    const parameters = name === "eq" ? { "band1.gainDb": 3, "band2.gainDb": -5, "band3.gainDb": 4, "band4.gainDb": 2 }
      : name === "filter" ? { cutoffHz: 1000 } : {};
    const source = [
      "import { Project, chord, wavetable, sampler, multisampler, slicer } from '@oxitone/core';",
      "const project = new Project({ name: " + JSON.stringify(name) + " });",
      "const sample = project.addSample({ assetUri: './tone.wav', sha256: '" + hash + "', format: 'wav', sampleRate: 48000, channels: 1, frames: 48000n });",
      "const keys = project.addChannel({ name: 'Keys', instrument: " + selected + " });",
      "const other = project.addChannel({ name: 'Second instance', instrument: wavetable() });",
      "const fx = { pluginId: 'oxitone." + (instrument ? "filter" : name) + "', pluginVersion: '1.0.0', parameters: " + JSON.stringify(parameters) + " };",
      "keys.addEffect(fx); other.addEffect(fx);",
      "project.addTrack('Phrase').use(keys).add(chord(60, 'major')).at({ bar: 1 });",
      "export default project;",
    ].join("\n");
    await writeFile(entry, source);
    const result = await promisify(execFile)(process.execPath, ["packages/cli/dist/index.js", "daw", entry, "--viewer", resolve("target/debug/Oxitone Preview.app")], {
      env: { ...process.env, OXITONE_PREVIEW_CAPTURE: outputFile,
        OXITONE_PREVIEW_APPEARANCE: theme, OXITONE_PREVIEW_CAPTURE_SIZE: "1440x920",
        OXITONE_PREVIEW_CAPTURE_BUILTIN: editing ? "edit" : instrument ? "instrument" : "effect",
        ...(page ? { OXITONE_PREVIEW_CAPTURE_PAGE: page } : {}),
        ...(narrow ? { OXITONE_PREVIEW_CAPTURE_PLUGIN_SIZE: "440x540" } : {}) },
      timeout: 120000, maxBuffer: 1024 * 1024,
    }).catch(async error => {
      await writeFile(outputFile + ".log", error.stderr ?? String(error));
      throw error;
    });
    await writeFile(outputFile + ".log", result.stderr);
    if (!result.stderr.includes("Preview capture saved")) throw new Error(result.stderr);
    if (editing) {
      if (!result.stderr.includes("Builtin panel editing passed")) throw new Error(result.stderr);
      const reopened = await ProjectDocument.open({ entry });
      try {
        const [first, second] = reopened.frame.snapshot.channels;
        if (!(first.effectChain[0].parameters.cutoffHz > 1000) || first.effectChain[0].mix !== 0.7 ||
          first.effectChain[0].bypass !== true || second.effectChain[0].parameters.cutoffHz !== 1000) {
          throw new Error("Panel edit did not survive source reopen");
        }
      } finally { reopened.close(); }
      await writeFile(join(output, "saved.ts.txt"), await readFile(entry, "utf8"));
    }
    console.log("PASS " + name + " · " + outputFile);
  }
} finally { await rm(root, { recursive: true, force: true }); }
