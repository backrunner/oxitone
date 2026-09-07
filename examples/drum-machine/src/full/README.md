# After the Horizon / 地平线之后

An original F# minor melodic dubstep composition: 140 BPM, 104 bars, 178.29 seconds
of music plus a three-second export tail. All musical parts use native synthesis;
percussion uses the Circuit electronic drum preset. No sample download is required.
The lofi demo has been retired.

The eight-bar C#–C#–E–F# hook begins on crystal FM plucks, becomes the central lead
in both drops, fragments in the breakdown and resolves to F# in the ending.
The revised 20-track arrangement preserves every lead note in both drops. Drop I
introduces shuffled tops, chord plucks and vowel answers in stages; Drop II adds
glass replies, longer final chords and a harmonic halo. Eight-bar turnarounds leave
space for bass fills and phrase-delay tails. A rounded Reese bassline and offbeat
plucks develop the bridge and reprise. Build lifts and downlifters have separate
envelopes, with a short breath before each drop.
Named patterns expose the phrasing in the read-only GPUI preview.
The audio project has no MIDI channel assignments or 16-track limit. Only MIDI
export uses a separate copy with channel sharing, leaving the playback project intact.

- `synth-patches.ts`: crystal pluck, glass, supersaw, chord body, lead, independent
  sine Sub, FM/vowel/Reese bass, chord pluck, harmonic halo, moving pad and noise.
- `mix.ts`: EQ, distortion, Multiband Dynamics, nonlinear filtering, subtle tape,
  drum room convolution, filtered hall/echo, kick sidechain and master processing.
- `motion.ts`: chord brightness envelopes, rhythmic gain recovery, build filtering
  and phrase delay sends. `phrasing.ts`: complementary bass/drum/chord cells.
- `melodic-dubstep.ts`: full arrangement and transitions.

```sh
pnpm build
pnpm example:songs:prepare
pnpm example:songs
pnpm build:wasm
pnpm example:songs:wasm
```

Preparation builds hash-pinned local drum/gain libraries. Rendering and verification
are offline and never open audio devices. Outputs stay under `target/examples/full-songs`:
`after-the-horizon.wav` (48 kHz stereo PCM24), MIDI, snapshot and `report.json`.
The report includes integrated LUFS/true peak from Rust, section RMS/crest, stereo
correlation, low-frequency side/mid ratio and broad first-order spectral estimates.
Four-bar measurements expose changes inside each section. The final 48 kHz master
is 181.286 s, −13.33 LUFS and −2.42 dBTP; drop crest is 10.43–10.73 dB and low
side/mid is below −24.9 dB. Render checks include build/drop contrast and mono bass.
Wasm renders the full song separately and compares every sample with native PCM.
The measured maximum difference is 4.77e−7, with no memory growth across 4000
process blocks. Native/Wasm full exports ran at 2.008×/1.094× realtime in this run.
Process deadline exceedances remain in native and Wasm measurements; these offline
results do not establish sustained realtime playback performance. See the
[measurement archive](../../../../benchmarks/results/2026-09-08-horizon-arrangement.json).
Measurements help catch regressions; subjective commercial-release quality still
requires a listening and production review.

```sh
pnpm build:preview
pnpm preview examples/drum-machine/src/full/melodic-dubstep.ts
```

Preview watches static imports, including presets and mix code. Each synth/effect
has an independent read-only detail window. Invalid edits retain the last accepted
project and panels. Playback starts only when requested in the UI.
