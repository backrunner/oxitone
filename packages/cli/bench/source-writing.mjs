import { cpus, platform, release } from "node:os";
import { performance } from "node:perf_hooks";
import { arp, chord } from "@oxitone/core";
import { anchorPatternExpression, materializePatternReference, writeLiteralPatternEdit, writePatternEdit } from "../dist/source/index.js";

const text = "import type { Pattern } from '@oxitone/core';\nimport { phrase } from 'arrangements';\nexport default phrase;\n";
const start = text.lastIndexOf("phrase");
const anchor = anchorPatternExpression("song.ts", text, start, start + 6);
const warmup = 3;
const iterations = 20;
const measurements = [];
let bytes = 0;
function measure(name, notes, run) {
  for (let i = 0; i < warmup; i++) run();
  const times = [];
  for (let i = 0; i < iterations; i++) {
    const start = performance.now(); const result = run(); times.push(performance.now() - start);
    bytes += result.text.length;
  }
  times.sort((a, b) => a - b);
  const percentile = (p) => times[Math.ceil(times.length * p) - 1];
  measurements.push({ name, notes, p50Ms: percentile(0.5), p95Ms: percentile(0.95), p99Ms: percentile(0.99) });
}
for (const notes of [100, 1000]) {
  const source = arp(chord(60, "major"), "upDown", 0.25).repeat(notes / 4).toSource();
  const request = { fileName: "song.ts", text, anchor, source, operations: [
    { select: { iteration: 2, note: { step: 1 } }, set: { pitch: 65 } },
  ] };
  measure("sparse-write", notes, () => writePatternEdit(request));
  measure("materialize-with-import", notes, () => materializePatternReference(request));
  const detached = materializePatternReference(request);
  measure("literal-note-write", notes, () => writeLiteralPatternEdit({ ...detached, fileName: "song.ts", operations: [
    { select: { at: { start: 2.25, pitch: 65 } }, set: { velocity: 0.3 } },
  ] }));
}
console.log(JSON.stringify({ benchmark: "source-writing", cpu: cpus()[0]?.model,
  os: `${platform()} ${release()}`, node: process.version, warmup, iterations,
  device: null, sampleRate: null, blockSize: null, callbackP95: null, callbackP99: null,
  xruns: null, cpuUtilization: null, measurements, outputCharacters: bytes }, null, 2));
