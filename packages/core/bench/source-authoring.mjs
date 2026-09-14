import { cpus, platform, release } from "node:os";
import { performance } from "node:perf_hooks";
import { arp, chord, Pattern } from "../dist/index.js";

// Pure authoring benchmark: no engine, audio device, DSP callback or PCM.
const warmup = 3;
const iterations = 20;
const measurements = [];
let checksum = 0;
function measure(name, notes, run) {
  for (let i = 0; i < warmup; i++) run();
  const times = [];
  const before = process.memoryUsage().heapUsed;
  for (let i = 0; i < iterations; i++) {
    const start = performance.now();
    checksum += run().notes.length;
    times.push(performance.now() - start);
  }
  times.sort((a, b) => a - b);
  const percentile = (p) => times[Math.min(times.length - 1, Math.ceil(times.length * p) - 1)];
  measurements.push({
    name,
    notes,
    p50Ms: percentile(0.5),
    p95Ms: percentile(0.95),
    p99Ms: percentile(0.99),
    heapDeltaBytes: process.memoryUsage().heapUsed - before,
  });
}
for (const count of [1_000, 100_000]) {
  const base = arp(chord(60, "major"), "upDown", 0.25).repeat(count / 4);
  const edits = Array.from({ length: 128 }, (_, index) => ({
    select: { iteration: index, note: { step: 1 } },
    set: { pitch: 65 },
  }));
  measure("edit-one", count, () => base.edit([edits[0]]));
  measure("edit-128", count, () => base.edit(edits));
  const serialized = JSON.stringify(base.edit(edits).toSource());
  measure("json-rebuild", count, () => Pattern.fromSource(JSON.parse(serialized)));
}
console.log(
  JSON.stringify(
    {
      benchmark: "source-authoring",
      cpu: cpus()[0]?.model,
      os: `${platform()} ${release()}`,
      node: process.version,
      warmup,
      iterations,
      device: null,
      sampleRate: null,
      blockSize: null,
      callbackP95: null,
      callbackP99: null,
      xruns: null,
      cpuUtilization: null,
      heapMeasurement: "net delta, includes GC; not peak memory",
      measurements,
      checksum,
    },
    null,
    2,
  ),
);
