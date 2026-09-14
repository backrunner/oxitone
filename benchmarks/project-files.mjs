// Control-thread project save/load benchmark. Requires `pnpm build`.
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir, cpus, release } from "node:os";
import { join } from "node:path";
import { performance } from "node:perf_hooks";
import { loadProject, Pattern, Project } from "../packages/core/dist/index.js";
import { inspectSample } from "../packages/native/dist/index.js";

const root = await mkdtemp(join(tmpdir(), "oxitone-files-bench-"));
try {
  const source = new Project({ seed: 42 });
  source
    .addTrack()
    .use(source.addChannel())
    .add(new Pattern({ lengthBeats: 2, notes: [{ pitch: 69, start: 0, duration: 2, velocity: 0.5 }] }))
    .at({ bar: 1 });
  const path = join(root, "source.wav");
  await source.renderWav({ path, end: { seconds: 1 }, tailSeconds: 0 });
  const info = inspectSample(path);
  const project = new Project({ seed: 42 });
  project.addSample({ ...info, assetUri: path, frames: BigInt(info.frames) });
  const target = join(root, "song");
  const save = [],
    load = [];
  for (let i = 0; i < 35; i++) {
    let start = performance.now();
    await project.save(target);
    const saveMs = performance.now() - start;
    start = performance.now();
    await loadProject(target);
    if (i >= 5) {
      save.push(saveMs);
      load.push(performance.now() - start);
    }
  }
  const stats = (values) => {
    values.sort((a, b) => a - b);
    return { medianMs: (values[14] + values[15]) / 2, p95Ms: values[28], p99Ms: values[29] };
  };
  process.stdout.write(
    JSON.stringify(
      {
        schema: "oxitone.project-file-benchmark.v1",
        date: new Date().toISOString(),
        machine: {
          cpu: cpus()[0]?.model,
          osRelease: release(),
          arch: process.arch,
          node: process.version,
          tempRoot: tmpdir(),
        },
        config: {
          sampleRate: 48000,
          blockSize: 128,
          channels: 2,
          assets: 1,
          frames: 48000,
          bitDepth: "float32",
          warmupIterations: 5,
          measurementIterations: 30,
        },
        save: stats(save),
        load: stats(load),
        callbackP95Ns: null,
        callbackP99Ns: null,
        xruns: null,
        notes: [
          "Warm filesystem cache; save revalidates an already-published asset and atomically replaces the manifest, including fsync.",
          "Fixture rendering and decoding occur outside measurements. Load validates manifest and source hashes without decoding audio.",
          "Control-thread I/O measurement, not realtime callback/worker performance acceptance.",
        ],
      },
      null,
      2,
    ) + "\n",
  );
} finally {
  await rm(root, { recursive: true, force: true });
}
