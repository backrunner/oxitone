# Offline phrase and sample slices

From the repository root:

```sh
pnpm build
pnpm example
```

To choose an output directory:

```sh
pnpm --filter @oxitone/example-offline start /absolute/path/to/output
```

- [song.ts](src/song.ts) authors a two-bar phrase with Wavetable, a beat-synced delay
  insert and pan automation.
- [chops.ts](src/chops.ts) imports the rendered phrase through the Rust WAV cache,
  uses Slicer grid markers and reorders the slices under a 150→180 BPM ramp.
- [index.ts](src/index.ts) exports WAV/MIDI/snapshot, saves portable project
  directories and checks the restored sample project's WAV hash against its source.

Outputs default to the ignored `target/examples/offline` directory. The example
runs entirely offline. Open `phrase.wav` and `chops.wav` in an audio player to hear
the results; `phrase-project` and `chops-project` can be opened with `Project.load`.
