# After the Horizon / 地平线之后

An original F# minor melodic dubstep composition: 140 BPM, 104 bars, 178.29 seconds
of music plus a three-second export tail. Soft Piano opens the theme; Grand Piano
supports the breakdown. Electronic parts use native synthesis and Circuit drums.
The lofi demo is retired.

The 24-track arrangement preserves every main lead note in both drops. A sustained
mono sub has its own short kick-duck envelope; a separate harmonic bassline makes
the root audible above the sub range. FM turbine, vowel motion, sync stabs and
Reese layers alternate their rhythmic roles. Middle-register supersaws, central
chord body, plucks, glass replies and a final harmonic halo develop the energy.
The snare has a clap layer; shorter kicks leave space for bass recovery. Eight-bar
turnarounds expose bass fills and delay tails. MIDI channel sharing is applied only
to an export copy; normal playback/preview has no 16-track limit. Shared MIDI
families cannot reproduce independent synth patches, CCs or overlapping same pitches.

- `piano.ts` / `piano-assets.json`: explicit, hash-verified sample preparation and
  resource mapping. `softPiano`/`grandPiano` helpers use Rust multisampler playback.
- `synth-patches.ts` / `bass-patches.ts`: independent source patches. FM and warp
  are separated to avoid compounding conservative wavetable bandlimiting.
- `mix.ts`: sub/bassline/midbass separation, EQ, distortion, Multiband Dynamics,
  nonlinear filter, tape, room convolution, ducked hall/echo and master limiting.
- `motion.ts` / `phrasing.ts`: filter envelopes, kick recovery, bass/drum/chord cells
  and phrase sends. `melodic-dubstep.ts`: complete structure and transitions.

From the repository root:

```sh
pnpm build
pnpm example:songs:prepare
pnpm example:songs
pnpm --filter @oxitone/example-drums songs:stems
pnpm build:wasm
pnpm example:songs:wasm
```

Preparation builds local drum libraries and downloads 120 pinned FLAC recordings
(179 MB compressed; ~599 MiB decoded for the full bank). Existing verified files
are reused. The demo registers 52 recordings in keys 48–84 (~256 MiB decoded).
Ordinary builds/tests never download assets; CI uses small local recorded fixtures.
All render/verification commands are offline and never open audio devices.

Outputs stay under `target/examples/full-songs`: `after-the-horizon.wav` (48 kHz
stereo PCM24), MIDI, snapshot and `report.json`. `drop-stems/` holds eight bars of
separate bus taps and a master, verifying bass energy independently of kick. Taps
are each bus's direct master contribution: send-only buses are zero here, with
their signal included in downstream buses (e.g. the saw stack in Music).
Reports include loudness/true peak, section/phrase RMS, crest, stereo correlation,
mono low-end and broad overlapping spectral bands. Wasm uploads the same sample
bytes, renders the full song and compares native PCM. Timing is a separate check:
offline exports do not establish sustained realtime performance or subjective
commercial-release quality. See the current evidence in
[the piano/bass archive](../../../../benchmarks/results/2026-09-08-horizon-piano-bass.json).

```sh
pnpm install:cli
oxitone preview examples/drum-machine/src/full/melodic-dubstep.ts
oxitone build examples/drum-machine/src/full/melodic-dubstep.ts -o target/examples/after-the-horizon.mjs
oxitone preview target/examples/after-the-horizon.mjs
```

Preview watches imports by default and starts stopped. Invalid source edits retain
the last accepted project. `oxitone build ... --watch` also refreshes the single
ESM artifact after successful builds. Installed SDK/native libraries and sample
assets remain external; this is one project logic file, not an embedded sample bank.

Recordings: **Salamander Grand Piano v3**, Alexander Holm, **CC BY 3.0**.
[FLAC distribution](https://github.com/sfzinstruments/SalamanderGrandPiano) by
sfzinstruments/kinwie; pinned revision `3382bf9496bba2486f5ab0de55a264d1dfc38404`.
[License](https://creativecommons.org/licenses/by/3.0/). Oxitone selects four of
the original 16 velocity layers and supplies independent mapping, envelopes and
mix processing; original recordings are unchanged. Attribution is also written
to the prepared cache. No sample binaries are committed.

Technique references: Au5's [Supersaw Killer](https://www.youtube.com/watch?v=IhvO8grER5s),
[Cracked Bass Drops](https://www.youtube.com/watch?v=t3__vw0vaV8),
[Reese tutorial](https://www.youtube.com/watch?v=W87uuuGcq9c), and the
[Virtual Riot / Modestep interview](https://splice.com/blog/virtual-riot/).
These inform general voicing, layered timbres, alternating bass gestures and drum
detail; patches and composition are original.
