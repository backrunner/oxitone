// Interactive native editor validation. No audio device is opened.
import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { editVst3Plugin } from "../packages/vst3/dist/index.js";

const bundlePath = process.env.OXITONE_VST3_FIXTURE;
if (!bundlePath) throw new Error("Set OXITONE_VST3_FIXTURE to VestiGain.vst3");
const result = await editVst3Plugin(
  { bundlePath: resolve(bundlePath), classId: "56455354494741494e30303030303031", allowPlugins: "any" },
  { parameters: { 0: 0.75, 1: 0 } },
  { hostPath: resolve("target/release/oxitone-vst3-host"), timeoutMs: 180_000 },
);
if (process.argv.includes("--cancel")) {
  assert.equal(result, null);
} else {
  assert.ok(result?.configuration);
  assert.equal(result.classId, "56455354494741494e30303030303031");
  assert.ok(Object.keys(result.configuration.parameters).length > 0);
  await writeFile(resolve("target/vst3-editor.json"), `${JSON.stringify(result, null, 2)}\n`);
}
console.log(`Native VST3 editor ${result ? "Apply" : "Cancel"} passed; no audio device used`);
