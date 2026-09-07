# Authoring and native API guide

This guide describes the current development API. Exact exported types and options
live in `packages/*/src`; the versioned wire contract is documented in
[04-api-contracts.md](../.agents/docs/04-api-contracts.md). Runtime errors expose a
stable `code` through `OxitoneError`, plus a message and optional details/path.

## Package entry points

| Package | Use |
| --- | --- |
| `@oxitone/core` | Project, builders, instrument helpers, automation and project files |
| `@oxitone/samples` | `importSample` metadata and normalized WAV caching |
| `@oxitone/midi` | SMF export facade and option/report types |
| `@oxitone/protocol` | Schemas, wire types, canonical encoding and error codes |
| `oxitone` | Unified Project/builders, presets, sample import and native facade |
| `@oxitone/native` | Lower-level engine facade and native binary resolver |
| `@oxitone/cli` | `render`, `export-midi`, `doctor`, `preview` commands |

The generated native package is a build artifact interface, not an authoring API.

## Time, notes and clips

`new Project({ name?, seed?, sampleRate?, blockSize? })` creates an editable project.
Defaults are 48 kHz and 128 frames. Keep a fixed seed for reproducible chance,
probability and export dither. `setTempo(bpm, curve?)` and
`addTempoSegment({startBeat, bpm, curve?})` support step, linear and exponential
changes; BPM is 20..999. Time signatures change at bar boundaries.

`addTrack(name?)`, `addChannel(options?)` and `track.use(channel)` establish the
arrangement. A Pattern has a positive `lengthBeats` and notes with `pitch` (0..127),
`start`, `duration` and `velocity` (0..1). Place it with
`track.add(pattern).at({bar: 1, beat: 0})`. Bars start at 1; beat offsets start at 0.
Note start/duration and clip lengths use beats; envelope times use seconds.

Use `chord` and `arp` for generated note arrangements; their option types are
exported from core. Track `enabled`, `midiChannel` and `tempo` are editable. Track
tempo overrides its note and sample-clip clock; the Project tempo remains the shared
Channel instrument/effect clock.

Snapshots are detached wire data. `project.snapshot()` does not expose the live
builders. `revisionBigInt` retains the complete u64 revision; `revision` is a number
convenience and rejects values beyond JavaScript's safe-integer range.

## Instruments and mixer

```ts
import { wavetable, sampler, slicer } from '@oxitone/core';

const synth = wavetable({ oscA: { wave: 'saw', unison: 4, detune: 12 },
  filter: { type: 'lowpass', cutoff: 3000 }, amp: { release: 0.2 } });
const keys = sampler(sample, { rootKey: 60, loop: 'forward' });
const chops = slicer(sample, { slices: { grid: 8 }, tempoSync: 'repitch' });
channel.instrument = chops;
```

These return declarative InstrumentRefs. Omitted controls use Rust defaults.
Sampler resources must refer to a Sample in the target project. Slicer also accepts
explicit `{start: {frames: bigint | number}, end?, rate?, level?, pan?, reverse?}`
markers, beat markers or `{onset: {algorithm: 'onset-v1', sensitivity?}}`.
`triggerNote` defaults to 60; `playMode` is oneshot or gate. Slice tables are immutable
during playback; update the Session after changing them.

Channel controls are `level` (0..2), `pan` (-1..1), `swing` (0..1), `mute`, `solo`,
`mixerChannelId` and an ordered `effectChain`. `addEffect(ref)` appends an insert.
`project.master` and `addMixerChannel({name?, ...})` expose bus controls and inserts.
`bus.send(destination, {ratio?, preFader?, sidechain?})` creates a send;
`removeSend(destination)` removes it. Rust rejects routing and detector cycles.

EffectRefs contain `pluginId`, `pluginVersion`, declared `parameters`, and optional
`mix` (0..1) / `bypass`. Built-ins are `oxitone.eq`, `limit`, `clipper`, `filter`,
`phaser`, `reverb`, `compressor`, `delay`, `gate`, `chorus`, `saturator`, `utility`,
each at plugin version `1.0.0`. Parameter IDs are validated against the descriptor;
unknown IDs fail compile. Inserting an effect changes the graph and requires update.

## Samples and persistence

```ts
import { importSample } from '@oxitone/samples';

const imported = importSample('/source/loop.mp3', {
  assetBaseDir: '/work/song', cacheDir: 'cache',
});
const sample = project.addSample({ ...imported, musicalLengthBeats: 8 });
const clip = track.sample(sample).at({ bar: 1 }, { tempoSync: 'stretch' });
clip.fitBars(2);
await project.save('/work/song/project', { assetBaseDir: '/work/song' });
const restored = await Project.load('/work/song/project');
```

Without `cacheDir`, import is read-only metadata inspection. With it, Rust publishes
a float32 WAV cache and retains source provenance. PCM never crosses into JavaScript.
`frames` accepts bigint or a safe integer; asset hash/format describe the actual
playback asset, while provenance describes the original source.

Sample edits are non-destructive descriptors. Clip `tempoSync` supports off, repitch
and `wsola-v1` stretch. `fitBeats`, `fitBars` and `fitToContent` update the clip's
musical duration. Repitch changes speed and pitch; stretch preserves pitch.

`Project.save` copies verified assets into a portable directory and atomically
publishes its canonical manifest. `Project.load` returns editable builders and
retains the asset base for future compile/render/save. `Project.fromSnapshot`
restores wire data, with an optional `assetBaseDir`. Lower-level `loadProject`
returns `{snapshot, assetBaseDir}`; `saveProject` accepts a snapshot directly.
Missing or changed assets fail with `AssetUnavailable`; incompatible major versions
fail with `ProtocolVersionUnsupported`. Preset files are still a planned feature.

## Automation and live changes

`createAutomationNamespace()` supplies constant, line, curve/polyline, gate, wave,
sine/cos and deterministic chance sources. Sources combine through map, clamp,
invert, scale, offset, mix, add, multiply, min/max and quantize.
Bind with `channel.automate(parameterId, source, options?)` or the corresponding
Project/MixerChannel API. Lane output is normalized 0..1; the descriptor maps it
to physical units. For pan, 0.5 is center.

Insert targets use `insert.<index>.mix`, `insert.<index>.bypass` and
`insert.<index>.parameter.<pluginParameterId>`. Bus send automation uses
`send.<destinationId>.ratio`. Project tempo automation is compiled into the effective
tempo map. Slicer `tempoFactor` is reserved for the host.

`session.setParameter(entityId, parameterId, value, atFrame?)` accepts physical
values and uses the normal native parameter queue/smoothing. Automation overrides
a host change at the same frame. Audio-rate and control-rate behavior comes from the
descriptor; authoring mutations alone do not update an already compiled Session.

## Sessions, export and CLI

`await project.compile(options?)` returns a Session. `await project.play(position?,
loop?)` compiles current authoring and starts device playback. Position supports
bar/beat, marker, seconds or frame; loop bounds use absolute frames. Session exposes
play, pause, stop, seek, update and dispose. `await session.update()` keeps the engine
ID; rejected compilation preserves the old compiled graph. Always dispose sessions
you no longer use. Device playback is separate from offline rendering.

`project.renderWav({path, ...})` and `session.renderWav(...)` use the Rust offline
renderer, including sample decoding, PDC, effects and loudness measurement. The
Session exports its compiled snapshot. Options include start/end, tailSeconds,
bitDepth, dither and mixer-channel/track stems. Float32 is the default; integer
exports default to deterministic TPDF dither. Reports include duration, peak,
true peak, LUFS and graph latency.

`exportMidi({path, ...})` produces deterministic SMF Type 1 without importing MIDI.
Unmapped audio parameters are reported as skipped automation. Tracks beyond the
16-channel automatic allocation limit require explicit `midiChannel` assignments.

The CLI accepts a snapshot JSON file, portable project directory or its
`oxitone.project.json`. Snapshot assets resolve relative to the input file; outputs
resolve relative to the current working directory:

```sh
pnpm --filter @oxitone/cli exec node dist/index.js doctor
pnpm --filter @oxitone/cli exec node dist/index.js render /path/phrase.snapshot.json /path/phrase.wav
pnpm --filter @oxitone/cli exec node dist/index.js export-midi /path/phrase.snapshot.json /path/phrase.mid
pnpm --filter @oxitone/cli exec node dist/index.js render /path/chops-project /path/chops.wav
```

CLI failures return JSON diagnostics on stderr with exit code 1; invalid command
usage returns exit code 2. `preview <entry.ts>` launches the read-only GPUI app and
dependency watcher; see [preview usage](preview.md).
Low-level consumers can use `createEngine`,
`compile`, `enqueueTransport`, `renderWav`, `exportMidi`, diagnostics and `dispose`
from `oxitone`. Third-party plugins register explicitly with `registerPlugin` and
their manifest/path; loading and validation occur on the control thread. See the
[plugin ABI specification](../.agents/docs/08-plugin-abi.md).

The [drum-machine example](../examples/drum-machine) demonstrates a complete dynamic
instrument/effect chain, including host parameter changes, automation, hash checking
and project restore. Run `pnpm example:drums` after `pnpm build`. Its source builds
local unsigned development libraries and explicitly selects `allowPlugins: "any"`.
Instrument parameters use their descriptor IDs (the example uses `volume` and
`decay`); channel controls such as `level` and `pan` take precedence over instrument
parameters with the same name. Saved projects retain plugin IDs and versions;
register the matching libraries on the engine used to render their snapshots.

`project.registerPlugin({libraryPath, manifest, expectedHash?}, {allowPlugins?})`
retains validated paths and hashes for subsequent compile/play/render calls, and
registers with an existing Session. `session.registerPlugin` affects that Session;
`session.pluginDiagnostics()` reports per-plugin faults. Register before compiling
when selecting a different trust policy: an existing Session keeps its engine policy.
`getPluginInfo(engine, id, version)` reads authoritative native descriptor metadata.
Registration is runtime configuration and is not saved into the project snapshot.

## Presets

```ts
import { createChannelPreset, savePreset, loadPreset, applyPreset } from 'oxitone';

const preset = createChannelPreset(channel, { name: 'Soft keys', samples: project.samples });
await savePreset(preset, '/path/presets/keys.oxitonepreset.json', { assetBaseDir: project.assetBaseDir });
const loaded = await loadPreset('/path/presets/keys.oxitonepreset.json');
await applyPreset(project, channel, loaded.preset, { assetBaseDir: loaded.assetBaseDir });
await project.session?.update();
```

Instrument/effect presets use `createPluginPreset(ref, 'instrument' | 'effect',
{name?, samples?})`; `presetInstrument` / `presetEffect` extract detached references
for authoring. Effect presets can be assigned to an insert array with `presetEffect`;
their resource IDs must already exist in that project. `applyPreset` applies an
instrument or whole Channel, imports/remaps its sample IDs, and compiles a detached
candidate first. Invalid state/parameters/resources leave the original project intact.

Files contain versioned canonical metadata and relative resource URIs/hashes.
Sample bytes remain external and must be inside the preset directory. Loading
validates plugin versions, ABI major, kind, parameters, state and sample hashes.
Pass `{plugins, allowPlugins}` to file/validation helpers for dynamic plugins.
Applying a preset changes authoring only; `Session.update` adopts structural changes
at a block boundary, resetting voices. Use `setParameter` for continuous live changes.
