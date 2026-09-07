# Full-length songs

Two original compositions authored entirely with the SDK. All synthesis, sample
playback, effects, mixing and export run in Rust.

| Song | Style | Tempo / bars | WAV duration |
| --- | --- | --- | --- |
| Rain on the Window / 窗边雨 | Lofi, D minor | 80 BPM / 60 bars | 3:03 |
| After the Horizon / 地平线之后 | Melodic dubstep, F# minor | 140 BPM / 104 bars | 3:01.29 |

Durations include a three-second export tail. These are complete arrangements:
lofi has intro, A, bridge, B, reprise and outro; dubstep has piano intro, two builds,
two half-time drops, breakdown, reprise and outro. Piano themes return in different
instrument layers. Named patterns/markers expose the form in arrangement and piano
roll; mixer returns and kick sidechain sends expose the routing.

From the repository root:

```sh
pnpm install --frozen-lockfile
pnpm build
pnpm example:songs:prepare
pnpm example:songs
```

`prepare` explicitly downloads ~157 MB of CC0 piano WAVs from a pinned VSCO 2 CE
commit, checks every file's size and SHA-256, imports metadata through Rust, and
builds/registers the local example drum dylib. Subsequent prepares reuse verified
files. Interrupted/corrupt downloads cannot replace a verified file; rerun prepare
to repair the cache. Ordinary build/test never downloads the piano. Node >=22.13,
Cargo and a C compiler are required, as for the existing drum-machine demo.

`render` requires preparation and writes to `target/examples/full-songs/`:

- `rain-on-the-window.wav` and `after-the-horizon.wav`: 48 kHz stereo PCM24.
- Matching `.mid` and `.snapshot.json` files.
- `report.json`: duration, LUFS, sample/true peak, per-section RMS, WAV SHA-256,
  render time, MIDI warnings and native drum fault counters.

Samples, binaries, rendered WAVs and metadata caches stay in ignored `target/`.
Snapshots reference the local cache; use `Project.save()` to create a portable
project directory with its assets. MIDI contains notes/tempo/markers; rendered
timbres and plugin automation are represented by the WAV and SDK source.

For the read-only GPUI preview with source watching:

```sh
pnpm build:preview
pnpm preview examples/drum-machine/src/full/lofi.ts
pnpm preview examples/drum-machine/src/full/melodic-dubstep.ts
```

Run one preview command at a time. Static imports, including the shared piano and
panel code, are watched automatically. Invalid intermediate code retains the last
valid project. Each entry registers the prepared drum dylib by its verified hash
and supplies a compact native piano panel. Existing preview transport, scroll,
zoom and multiple plugin windows apply to these projects.

The piano is VSCO 2 CE Upright Piano by Simon Dalzell / Ivy Audio, distributed by
Versilian Studios / Sam Gossner under CC0 1.0. See [CREDITS.txt](CREDITS.txt) and
[piano-assets.json](piano-assets.json). This curated subset has 13 sampled keys
(MIDI 41,45,…,89) × three recorded dynamics, mapped to MIDI 39…91 with at most two
semitones of repitch. Keys outside that range are silent. It is an acoustic piano
multisample instrument, with ADSR and velocity response; sustain-pedal events,
round-robin and sympathetic resonance are not implemented.

`multisampler()` is also exported from `oxitone` / `@oxitone/core` for other banks.
Regions use inclusive key/velocity ranges, a root key, optional gain and an existing
Project Sample. Region maps prepare off the audio thread; playing and seeking use
preallocated native voices. The reusable SDK does not depend on these demo assets.
