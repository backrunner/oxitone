# Oxitone

Oxitone is a TypeScript authoring SDK with a Rust audio engine. Write notes,
instruments, samples, effects and automation in TypeScript; Rust compiles the graph
and handles playback, WAV rendering and MIDI export. The native callback runs no
JavaScript; the Web Audio worklet only copies pre-rendered PCM from shared memory.

macOS 13+ is the target platform. Apple Silicon is the primary development target;
the macOS CI matrix also builds and tests Intel. This is an unreleased development
workspace. The shared Rust engine also runs in Wasm runtimes and browsers through
`@oxitone/web`. `oxitone` is the native unified authoring entry; publishing signed platform
binaries remains tracked work.

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

`pnpm example:drums` builds a local Rust drum-machine dynamic library and a C gain
effect, verifies their native parameter/automation paths, and renders
[Midnight Circuit](examples/drum-machine): a 16-bar song with drums, bass, chords and
melody. WAV, drum solo, MIDI, portable project and verification report go into
`target/examples/drum-machine`. No external samples are needed.

The unified entry point is `oxitone`; the individual workspace packages remain available:

```ts
import { Pattern, Project, wavetable } from 'oxitone';

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

## Preview the code as a DAW

For Wasm and Web Audio, see [the web guide](docs/web.md):

```sh
rustup target add wasm32-unknown-unknown --toolchain stable
pnpm build:wasm
pnpm example:web
```

Open the displayed localhost URL and press Play. The browser demo includes synths,
the same drum machine, effects, seek/pause, a live output scope, WAV download and
source watch. Edit `examples/web/src/song.ts`; invalid updates retain the last good
song. After `pnpm example:songs:prepare && pnpm example:songs`, the three-minute
all-synth melodic dubstep demo is available in the browser; `pnpm example:songs:wasm`
renders and compares its complete Wasm output under `target/examples/wasm`.

The GPUI viewer continues to use the native host:

```sh
pnpm build:preview
pnpm preview examples/offline/src/preview.ts
# The bundled drum-machine arrangement, after pnpm example:drums:
pnpm preview examples/drum-machine/src/preview.ts
```

The GPUI app shows tracks, pattern/sample clips, a piano roll, channel rack, mixer
routes/meters and waveform/spectrum/stereo scopes. Play, pause, seek and loop are
available; music is edited in TypeScript. Watch is on by default: imported source
changes rebuild the project and swap the native graph during playback. Code or
compile errors leave the last valid project playing and appear as diagnostics.

Export a Project or a sync/async factory from your entry file. `--no-watch` loads
once; `--watch-path path` adds file dependencies read at runtime. The local build
creates an unsigned macOS app bundle. See [the preview guide](docs/preview.md) for
entry examples, transport controls, plugin registration and current limits.

## Development checks

```sh
pnpm lint
pnpm typecheck
cargo build -p oxitone-preview
pnpm test
cargo fmt --all --check
cargo test --workspace
cargo bench -p oxitone-bench --bench slicer_tempo
cargo bench -p oxitone-bench --bench preview
```

`pnpm build`, lint and typecheck include the executable examples. `pnpm schemas`
regenerates protocol schemas; generated native bindings are produced by the native
build and must not be edited manually.

[macOS CI](.github/workflows/macos.yml) installs locked dependencies, builds from
source, checks schema drift, runs all tests, examples and focused benchmarks on
both architectures. Native tests use simulated sinks; browser tests require a
no-device audio sink and fail if it is unavailable. Tests never open system audio
outputs or change the selected device. These checks do not establish physical-device
or long-term latency acceptance. See [CI details](docs/ci.md).

[Migration notes](docs/migrations.md), [architecture](.agents/docs/01-architecture.md)
and [remaining milestones](.agents/docs/10-implementation-status.md) describe the
current contracts and release gaps. Contributions follow [AGENTS.md](AGENTS.md).

Licensed under [Apache-2.0](LICENSE).
