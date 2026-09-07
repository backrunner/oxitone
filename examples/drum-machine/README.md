# Midnight Circuit / 午夜回路

A self-contained 112 BPM, 16-bar electronic groove: native kick/snare/closed and
open hats, round bass, Am7–Fmaj7–Cmaj7–G6 keys and a bell motif. Intro, breakdown,
fills and a fade ending are authored in [src/song.ts](src/song.ts). Duration is
about 36 seconds including a two-second export tail.

From the repository root:

```sh
pnpm install --frozen-lockfile
pnpm build
pnpm example:drums
```

The command builds the unpublished `oxitone-example-drums` Rust cdylib and the
reference C gain effect with Cargo and the system C compiler. It registers both
with explicit paths, manifests and SHA-256 hashes on a development engine. Drum
DSP runs entirely in native code; TypeScript only writes notes and control data.
The keys and melody also use the built-in native delay effect.

Outputs in `target/examples/drum-machine`:

- `midnight-circuit.wav`: 48 kHz stereo, 24-bit, deterministic TPDF dither.
- `drums-only.wav`: the same arrangement with only the drum track enabled.
- `midnight-circuit.mid`: four MIDI tracks, drums explicitly on channel 10.
- `project/` and `midnight-circuit.snapshot.json`: editable authoring data.
- `report.json`: library hashes, parameter verification, peak/true-peak/LUFS,
  MIDI diagnostics, fault counters and restored-render SHA-256 parity.

`src/verify.ts` checks real N-API → Rust → drum library → gain library → WAV:
unity/bypass PCM parity, 6.0206 dB attenuation for half instrument volume and half
effect gain, initial/host/automation parity, automation precedence, block sizes
64/128/256, decay response and invalid parameter rejection. `pnpm test` includes
this check. Rust tests cover pad output, choke, reset, event offsets, tail and zero
heap allocations/frees in processing and reset.

To render a restored project, register the two libraries on an engine, then use
`renderWav(engine, restored.snapshot(), options)` as in `src/index.ts`. The general
CLI and `Project.renderWav()` do not inherit a different engine's plugin registry.
The project contains no samples or machine-specific library paths. MIDI retains
notes and tempo; plugin automation and the synthesized timbres are in the WAV and
project, with skipped MIDI automation listed in the report.

The supplied drum plugin uses one fixed voice per pad: MIDI 36 kick, 38 snare,
42 closed hat, 46 open hat. Hits retrigger the pad, note-offs are ignored, and a
closed hat chokes an open hat. `volume` is a linear 0..1 gain; `decay` is a 0.5..2
duration multiplier sampled when each hit starts. The noise generator resets to a
fixed seed. These are local unsigned example libraries, not published platform
packages or arbitrary third-party compatibility certification.
