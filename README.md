# Oxitone

Oxitone is a TypeScript authoring SDK with a Rust audio engine. Write notes,
instruments, samples, effects and automation in TypeScript; Rust compiles the graph
and handles playback, WAV rendering and MIDI export. JavaScript never runs in the
audio callback.

macOS 13+ is the target platform. Apple Silicon is the primary development target;
the macOS CI matrix also builds and tests Intel. This is an unreleased development
workspace: publishing platform binaries and making `oxitone` the complete authoring
entry point are still tracked work.

## Run from a checkout

Install Node.js 22.13+ or 24, Rust with `rustfmt`, Xcode Command Line Tools and the pnpm
version specified by `packageManager` in [package.json](package.json).

```sh
pnpm install --frozen-lockfile
pnpm build
pnpm example
```

The [offline example](examples/offline) creates a synth phrase, MIDI file and
portable project, imports the rendered phrase into a normalized sample cache, then
plays its slices through a tempo ramp. It verifies that saving and restoring the
sample project produces identical WAV bytes. Files go into `target/examples/offline`.
It does not need an audio device.

The current authoring entry point is `@oxitone/core`:

```ts
import { Pattern, Project, wavetable } from '@oxitone/core';

const project = new Project({ seed: 42 });
const keys = project.addChannel({ instrument: wavetable({
  oscA: { wave: 'triangle' }, amp: { release: 0.1 },
}) });
project.addTrack('Keys').use(keys).add(new Pattern({ lengthBeats: 4,
  notes: [{ pitch: 60, start: 0, duration: 1, velocity: 0.8 }],
})).at({ bar: 1 });
await project.renderWav({ path: 'phrase.wav', tailSeconds: 0.2 });
```

For device playback, use `const session = await project.play()` and dispose the
session when finished. [The API guide](docs/api.md) covers transport, samples,
automation, persistence and the lower-level native facade.

## Development checks

```sh
pnpm lint
pnpm typecheck
pnpm test
cargo fmt --all --check
cargo test --workspace
cargo bench -p oxitone-bench --bench slicer_tempo
```

`pnpm build`, lint and typecheck include the executable examples. `pnpm schemas`
regenerates protocol schemas; generated native bindings are produced by the native
build and must not be edited manually.

[macOS CI](.github/workflows/macos.yml) installs locked dependencies, builds from
source, checks schema drift, runs all tests, examples and focused benchmarks on
both architectures. A BlackHole virtual output supplies CoreAudio for device smoke
tests. Hosted-runner results do not establish physical-device or long-term latency
acceptance. See [CI details](docs/ci.md).

[Migration notes](docs/migrations.md), [architecture](.agents/docs/01-architecture.md)
and [remaining milestones](.agents/docs/10-implementation-status.md) describe the
current contracts and release gaps. Contributions follow [AGENTS.md](AGENTS.md).

Licensed under [Apache-2.0](LICENSE).
