# Wasm and Web Audio

Oxitone compiles the existing Rust engine to one import-free `wasm32-unknown-unknown`
module. Node, Wasmtime and browsers use the same graph compiler, scheduling, synthesis,
decoders, sample edits, automation, effects, sends, sidechains, mixer and WAV/MIDI encoders.

| Capability | Wasm / browser entry |
| --- | --- |
| Project, Pattern, tracks, notes, mixer, automation builders | Existing classes re-exported by `@oxitone/web` |
| Compile, process planar stereo PCM, transport, parameter events | `WasmEngine` |
| Sampler, multisampler/piano, slicer | Upload source bytes with `importSample`, then compile |
| Built-in synths/effects and example drums | Same Rust implementations; drums use the static C ABI adapter |
| WAV and MIDI | In-memory bytes from `renderWav` / `exportMidi` |
| Browser playback | `WebAudioSession`, Worker + SharedArrayBuffer + AudioWorklet |
| Code updates | `session.update(project)`; the example server watches its TS song source |
| Local files, N-API, CoreAudio, GPUI, Mach-O dylibs | Native host capabilities; not executable in a Wasm sandbox |

`Project.play/compile/renderWav/save/load`, native preset validation and
`Project.beatsForSeconds` retain their native-host contracts. In browsers use the explicit
Wasm/Web Audio session methods, `WasmEngine.resolveBeatDuration`, snapshot JSON and
in-memory asset/preset descriptors. There is no implicit Node polyfill or native addon
download. Import `@oxitone/web` in browser code; `oxitone` remains the native umbrella.

## Build and run

From a checkout, after `pnpm install --frozen-lockfile && pnpm build`:

```sh
rustup target add wasm32-unknown-unknown --toolchain stable
pnpm build:wasm
pnpm example:web
```

Open the displayed URL (default `http://127.0.0.1:4173`). Use `OXITONE_WEB_PORT` if
that port is occupied. Click Play to authorize the browser's AudioContext. Space
toggles playback; the timeline seeks to any sample position. The page supports system
light/dark appearance, an output waveform, parameter/graph updates and an 8-second WAV
download. Edit `examples/web/src/song.ts` while playing: the server rebuilds and updates
the accepted graph. Syntax, runtime and Rust validation failures retain the prior song.

The Wasm artifact is generated at `packages/web/dist/oxitone.wasm`, also exported as
`@oxitone/web/oxitone.wasm`. Package prepack builds it. No binary is checked into Git.

## Offline and non-browser runtimes

```ts
import { readFile, writeFile } from 'node:fs/promises';
import { Project, Pattern, WasmEngine } from '@oxitone/web';

const project = new Project({ sampleRate: 48000, seed: 7 });
project.addTrack().use(project.addChannel()).add(new Pattern({
  lengthBeats: 4,
  notes: [{ pitch: 60, start: 0, duration: 1, velocity: 0.8 }],
})).at({ bar: 1 });

const engine = await WasmEngine.create(
  await readFile('packages/web/dist/oxitone.wasm'),
);
try {
  engine.compile(project);
  engine.transport({ command: 'play' });
  const [left, right] = engine.process(); // borrowed PCM, one block
  // Consume/copy PCM before another control command or process overwrites it.
  await writeFile('phrase.wav', engine.renderWav({ frames: 96000, bitDepth: 24 }));
  await writeFile('phrase.mid', engine.exportMidi());
} finally { engine.dispose(); }
```

`WasmEngine.create` accepts a URL, BufferSource or compiled WebAssembly.Module.
Each engine has its own instance and memory. Controls are synchronous and should run
in a Worker for large projects. `process()` itself has no JSON, host calls,
allocation/deallocation or memory growth; `memoryDiagnostics()` exposes allocator
counters for verification. Control calls invalidate borrowed PCM views because Wasm
memory may grow while compiling/decoding.

For a sample, call `engine.importSample(bytes, 'wav')` (also AIFF, FLAC, MP3, M4A,
MP4). Register the returned hash/format/rate/channels/frames with
`project.addSample({ ...info, frames: BigInt(info.frames), assetUri: 'memory:keys' })`.
All referenced source bytes must be uploaded to each instance before compiling.
Rust verifies the declared hash and metadata and prepares each edit/rate variant.
An `assetUri` is descriptive here; the Wasm engine performs no file/network reads.

The raw ABI is described in
[the memory ABI specification](../.agents/docs/12-wasm-web-audio.md).
For a JavaScript-free host:

```sh
python3 -m venv target/wasmtime
target/wasmtime/bin/pip install wasmtime
target/wasmtime/bin/python scripts/wasmtime-smoke.py
```

This executes the static drum plugin, checks 1,000 blocks and allocator counters,
and writes `target/examples/wasm/wasmtime-drums.wav`.

## Browser host

```ts
import { WebAudioSession } from '@oxitone/web';

const session = await WebAudioSession.create({
  wasmUrl: '/oxitone.wasm',
  workerUrl: '/worker.js',
  workletUrl: '/worklet.js',
});
// Construct your Project at session.context.sampleRate.
await session.update(project);
await session.play(0); // call from a user gesture
await session.seek(48000);
await session.pause();
await session.dispose();
```

Bundle the package's `worker` and `worklet` entry points as separate ESM files.
The example's esbuild configuration is a working reference. If you serve the package's
unbundled files, resolve its module dependencies with your bundler; default URLs point
beside `session.js`. Serve `.wasm` as `application/wasm`.

Web Audio needs HTTPS (or localhost), cross-origin isolation and these response headers:

```text
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

The context's actual sample rate must match the Project; mismatch is rejected.
The Worker produces Rust PCM. The worklet only copies shared PCM and outputs zeros
on underrun. Producer/consumer own separate cursors; an epoch acknowledgment prevents
old PCM from surviving pause, seek or a graph replacement. The presentation cursor
advances only as the worklet consumes PCM, including pre-loop spans and cursor wrap.
It excludes the final device/OS latency.

`ringFrames` accepts powers of two from 512 through 65536 (default 4096); the render
horizon is at most 2048 frames. Project block size must fit that horizon/ring. Use
`diagnostics()` for buffered frames and underrun count. `onFault` reports fatal Worker
or worklet errors. Disposing terminates the Worker and disconnects output; an externally
provided AudioContext stays caller-owned.

Compilation/decoding happen in the Worker, so ordinary invalid updates retain the
last valid graph. A large successful build can drain its existing PCM buffer and cause
an audible gap: this version does not prepare graphs in a second Worker. Source watch
retains output on syntax/runtime/validation errors, but does not interrupt an infinite
loop in user code; run untrusted authoring in your own bounded Worker.

## Verification and limits

```sh
pnpm build:wasm
pnpm --filter @oxitone/web test
pnpm --filter @oxitone/example-web exec playwright install chromium
# In a second terminal, while pnpm example:web is running:
pnpm test:web
```

The smoke defaults to the dev server's port 4173. Set `OXITONE_WEB_PORT` or
`OXITONE_WEB_URL` when using another port; CI passes its isolated port explicitly.

Set `OXITONE_CHROME_PATH` to use an installed Chrome executable.
Tests force a no-device AudioContext sink (`sinkId: {type: 'none'}`), verify it and
also launch Chromium with `--mute-audio`. They fail if a no-device sink is unavailable;
they never fall back to system output. DSP/worklet assertions inspect the real PCM
before the silent sink. The interactive demo still plays only when you press Play.
`OXITONE_WEB_FULL_SONGS=1` also exercises the prepared melodic dubstep song in the browser.
After preparing and rendering the native song, `pnpm example:songs:wasm`
renders the full all-synth arrangement, checks every section for sound,
compares every PCM sample against native output, and records process p95/p99 and memory
counters. Output/report files go into `target/examples/wasm`.

ABI limits: 16 MiB JSON, 64 MiB per asset upload, 256 MiB PCM per in-memory WAV export,
1 GiB maximum Wasm linear memory. Long exports can be streamed by a raw host using
`process`; the convenience WAV API is bounded. The current melodic dubstep song needs
no sample uploads; desktop Chromium is the tested browser.
Wasm traps/OOM poison the instance and require recreation; they are distinct from
recoverable validation errors. Mobile, Safari/Firefox, background-tab scheduling,
multi-hour playback and device-loopback latency remain unverified. No Wasm plugin
dynamic linker or browser GPUI port is included.
