import { mkdir, writeFile } from "node:fs/promises";
import { cpus, release } from "node:os";
import { dirname, resolve } from "node:path";
import { inspectVst3Plugin } from "../packages/vst3/dist/index.js";
import { verifyLiveDaw } from "./vst3-daw-instance.mjs";

if (!process.env.OXITONE_VST3_FIXTURE) throw new Error("Set OXITONE_VST3_FIXTURE to VestiGain.vst3");
const source = {
  bundlePath: resolve(process.env.OXITONE_VST3_FIXTURE),
  classId: "56455354494741494e30303030303031",
  allowPlugins: "any",
};
const host = { hostPath: resolve(process.env.OXITONE_VST3_HOST_PATH ?? "target/release/oxitone-vst3-host") };
const metadata = await inspectVst3Plugin(source, host);
const results = await verifyLiveDaw(source, metadata, host, {
  parameterId: 0,
  initial: 0.875,
  changed: 0.625,
  playback: true,
  editor: process.argv.includes("--editor"),
  gain: 10 ** (-18 / 20),
});
const report = resolve(process.env.OXITONE_VST3_LIVE_DAW_REPORT ?? "target/vst3-live-daw.json");
await mkdir(dirname(report), { recursive: true });
await writeFile(
  report,
  `${JSON.stringify({ date: new Date().toISOString(), cpu: cpus()[0].model, os: `${process.platform} ${release()}`, profile: "debug Preview, release helper/addon", sampleRate: 48000, blockSize: 128, device: "simulated stereo sink", callbackP95: null, callbackP99: null, guiVisualAcceptance: false, results }, null, 2)}\n`,
);
console.log(`VST3 live DAW conformance passed: ${report}`);
