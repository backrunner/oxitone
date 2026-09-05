# Changelog

All notable changes to Oxitone are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
conventional commits (`type(scope): description`).

## [Unreleased]

### Added

- Atomic portable project save/load through Project.save and snapshot file helpers:
  versioned canonical manifests, content-addressed source assets, SHA-256 verification,
  fsync and atomic publication. Directory moves preserve native render output; load
  returns a snapshot and asset base. Editable builder restoration and compressed WAV
  caching remain pending.
- Graph compilation accepts assetBaseDir; Sessions retain it for updates and offline
  rendering, so saved projects can use relative sample URIs for playback as well.

- `Session.update()` recompiles on the same engine and preserves the previous compiled
  snapshot on rejection. Project.play refreshes changed revisions; Session exports use
  its compiled snapshot. Disposal is idempotent and clears the Project's active session.
- Session/Project transport supports bar+beat, stable marker IDs, seconds and frame(s).
  Rust resolves timecodes at the actual engine rate; pre-play graph updates preserve the
  transport cursor and state. Invalid/conflicting positions leave the graph usable.

- Independent Track tempo authoring with a shared Rust local-to-project clock for
  pattern scheduling, MIDI ticks, sample windows and timeline bounds. Sample repitch
  and stretch follow the local BPM; fitToContent respects the same static override.
  Golden tests cover tempo steps/ramps/lanes, loop/last clipping, actual MIDI replay
  timing and allocation-free sample reset/seek. Default Track clocks remain unchanged.

- `@oxitone/samples#importSample` and the versioned native `inspectSample` command:
  Rust reads, identifies, hashes and decodes local audio on the control thread, returning
  lossless dimensions and source/decoder provenance without exposing PCM. Descriptors
  work with `Project.addSample`, absolute paths or explicit relative asset bases. Tests
  cover audible deterministic WAV, SRC/trim, downmix, AAC/M4A and changed/missing assets.
  Import is synchronous and read-only; disk cache and project persistence remain pending.

- Track authoring exposes validated `enabled` and `midiChannel`, with native MIDI
  export coverage and rejection of Channel objects owned by another Project.

- M2 TypeScript sample authoring: immutable `Sample` references, `SampleClip`
  placement with `off`/`stretch`/`repitch`, loop/gain/pan/rate controls, and
  `fitBeats`/`fitBars`/`fitToContent` helpers wired into validated snapshots.

- TypeScript mixer authoring: editable Master and mixer buses, channel routing,
  pre/post-fader and sidechain sends, bus/send automation, Channel insert chains,
  swing/mute/solo, and defensive snapshots with revision-aware setters. Existing
  protocol 1.0 and Rust DSP are reused; native WAV tests cover inserts and routing.

- M5 dynamic plugins: typed per-engine registration, SHA-256/signature and
  manifest validation, owned C ABI instances, mono/stereo adaptation, fault
  muting and diagnostics. Includes the public C header, C/Rust conformance
  fixtures, native WAV integration tests and a focused adapter benchmark.
- Realtime graph/device-chain replacement now defers destruction to the
  control thread. Full command queues return errors, shutdown is independent
  of queue capacity, and incompatible live graph configurations are rejected.
- Rust plugin descriptors now own ID/version strings and borrow metadata
  from factories; dynamic metadata no longer requires leaking allocations.
  Reset is explicitly realtime-safe; create/prepare expose fallible methods.

- M5 foundation: `@oxitone/cli` provides `render`, `export-midi`, and
  `doctor` commands using the same validated native facade as applications.
- M4 transport loop regions are now part of the versioned transport command
  and TypeScript `Session.play` API, using exclusive sample-frame bounds.
- M5 ABI foundation: added the repr(C) plugin descriptor/parameter boundary
  with ABI-major and manifest validation tests.

- M0 engineering baseline: pnpm + Cargo workspaces, versioned protocol
  (`protocolVersion` 1.0), stable ID rules, error codes, N-API smoke test.
- M0 protocol contract layer: `@oxitone/protocol` (zod wire schemas, canonical
  JSON codec, pcg32-v1/hash64-v1, stable error codes, `OxitoneError`),
  `oxitone-core` wire types/codecs, `schemas/` JSON Schemas and canonical
  fixtures with TS↔Rust byte-exact round-trip tests.
- M1: Project/Track/Pattern/PatternClip/Note authoring models, tempo map with
  step/linear/exponential curves, time-signature map, deterministic SMF Type 1
  MIDI export (`oxitone-render::midi`, `@oxitone/midi`) with channel
  allocation/`MidiChannelLimit` and tempo resampling; byte-identical golden.
- M1: `Chord`/`Arp` pure authoring helpers and the fluent clip API
  (`track.pattern(p).at(...).loop(n).last(...)`) in `@oxitone/core`.
- M2: `oxitone-dsp` realtime-safe primitives — mip wavetable oscillators,
  ADSR, preallocated voice pool (quietest-then-oldest steal), equal-power
  gain/pan, RBJ biquads (f64 coefficients/state variants), windowed-sinc
  polyphase resampler (>100 dB SNR), `wsola-v1` time stretcher, TPDF dither,
  meters, FTZ/DAZ helpers.
- M2: `oxitone-samples` WAV/AIFF native decode, FLAC/MP3/MP4/M4A offline decode
  via symphonia, non-destructive edit baking (trim/level/normalize/fades),
  SRC, prepared-sample cache; `SampleFormatUnsupported` error code.
- M2: Plugin ABI v1 Rust trait, descriptors and static registry in
  `oxitone-graph`; built-in instruments `oxitone.wavetable`, `oxitone.sampler`,
  `oxitone.slicer` (marker/grid/`onset-v1` slicing, oneshot/gate, per-slice
  overrides) as statically linked ABI plugins.
- M2/M3: snapshot validator (IDs, dangling refs, ranges, plugin parameters,
  slicer state schema, automation targets, mixer DAG with full cycle reports)
  and `compile_plan` producing an immutable `RenderPlan`.
- M3: `oxitone-mixer` buses with pre/post-fader sends, sidechain detector
  routing, graph-wide PDC, meters with Master true-peak; 12 built-in effects
  (EQ, Limit, Clipper, Filter, Phaser, Reverb, Compressor with sidechain,
  beat-synced Delay, Gate, Chorus, Saturator, Utility) with oversampling and
  latency reporting for nonlinear stages.
- M3: automation end-to-end — `@oxitone/core` `AutomationNamespace` builders,
  Rust evaluator in `oxitone-transport` (gate/wave/curve/chance pcg32-v1 with
  absolute/restart phases, combinators, discontinuity-accurate segments),
  tempo lane baking to piecewise-linear BPM tables; golden vectors per
  `07-automation-spec.md` §7.
- M3: offline rendering — `RenderGraph` block renderer with SampleClip players
  (`tempoSync` off/stretch/repitch), channel inserts with automatable
  mix/bypass, metronome, WAV export (float32/16/24-bit with TPDF dither),
  stem export, single-pass loudness report (peak/true-peak/EBU R128 LUFS) and
  `graphLatencyFrames`; block-size parity golden tests.
- M3: N-API commands `renderWav`, `enqueueTransport`, `setParameter`,
  `exportMidi`, `listOutputDevices`, `getOutputLatency` (device calls return
  `DeviceUnavailable` until M4); `oxitone` facade and `Session` API in
  `@oxitone/core`; TS end-to-end tests against the real native binding.
- `oxitone-bench` criterion suite (compile/transport/automation/dsp incl.
  denormal corpus/mixer/render-offline) with JSON baseline archiving to
  `benchmarks/results/`.
- M4: `oxitone-io-macos` CoreAudio HAL output over
  `AudioDeviceCreateIOProcID` (input scope never touched; no AudioQueue/
  AVAudioEngine) — device enumeration with UIDs, nominal-rate negotiation
  (`adapt-device` with system-wide rate set, resample fallback), buffer
  frame size negotiation (target 128, floor 64, diagnostic fallback), f32
  stream-format setup (interleaved preferred, planar copy supported), and
  property listeners (default-device change, removal, nominal rate, buffer
  size) that only forward events to the monitor thread.
- M4: render-ahead realtime engine in `oxitone-render::realtime` — SPSC
  frame ring (`max(renderAheadBlocks, ceil((deviceBuffer+deviceLatency)/
  blockSize))`), dedicated time-constraint render worker (FTZ/DAZ, no
  shared locks with the control thread, `park/unpark` wakeup), polyphase
  device-rate resampling with group-delay accounting, stereo→device layout
  conversion (mono downmix, N-channel zero-fill), `latencyMode: 'direct'`
  with sample-accurate parity tests and automatic buffered fallback.
- M4: transport realtime semantics — play/pause/stop/seek take effect at
  the ring horizon via a lock-free command queue (seek flushes voices);
  underruns output silence, increment `xruns`, keep the transport running
  and emit a diagnostic attributed to the busiest channel; device hot-swap
  rebuilds the chain on the monitor thread (`follow-default`) or pauses
  (`pause`), with `DeviceUnavailable` when no device remains.
- M4: diagnostics per 05 — atomic `blocks`/`deadlineMisses`/`xruns`/
  `nanBlocks`/`queueDrops`, worker `engineLoad` EMA, log2 block-time
  histogram (p50/p95/p99/max), `PerformanceWarning` after 3 consecutive
  deadline misses; N-API `getDiagnostics`, real `listOutputDevices`, and
  `getOutputLatency(engineId)` with the ring/resampler/deviceBuffer/
  safetyOffset/deviceLatency breakdown (project-rate frames + seconds).
- M4: `Session.outputLatency()`/`Session.diagnostics()` in `@oxitone/core`;
  `EngineDiagnostics` wire schema in `@oxitone/protocol`.

### Fixed

- Sample fitting now integrates the effective Rust clock across tempo steps, ramps and
  looped/held tempo lanes. `Project.beatsForSeconds` exposes the same read-only conversion.
  Content-derived clip lengths account for trim; repitch fits the final duration and
  pre-rolls at the known initial rate without shifting later onsets. WSOLA resets retain
  buffers and are checked for zero allocation/deallocation and exact replay.
- Tempo lane baking applies loop/lastBeat, repeats source discontinuities and rejects
  excessive grid horizons before allocation. The private workspace root is now named
  `oxitone-workspace`, fixing native/core dependency ordering on first builds.

- Added playback baselines for reset and the first sample block:
  repitch reset 97.05 µs and WSOLA reset 311.65 µs at 48 kHz/128 frames on Apple M4.

- Sample authoring accepts positive u64 bigint frames, rejects unsafe number frames,
  honors explicit IDs, and validates trim relationships and positive musical lengths.
  Failed sample/clip construction does not register an entity or bump revision; drafts
  can retry. `fitBars` retains the full starting-signature length at nonzero beat offsets;
  `fitToContent` uses trimmed frames when no musical length is declared.

### Performance

- Added a focused file inspection benchmark (read/hash/decode/disposal): 48 kHz stereo
  PCM16 1 s and float32 10 s. Apple M4 measurements and limitations are archived in
  `benchmarks/results/2026-09-06-sample-import.json`; no audio callback code changed.

- M4 realtime validation (Apple M4, 48 kHz/128, 8-track synth + FX
  workload, release build): 10-minute device soak with 0 xruns and worker
  p99 well under the 70%-of-deadline budget; jitter-injection soak (worker
  preemption bursts, average load < 70%) with 0 underruns. Records in
  `benchmarks/results/2026-09-05-m4-*.json`.
- Offline render ratio for `render/offline/render_wav_8tracks_20s` raised from
  ≈6.4x to ≈22x realtime (Apple M4 baseline): wavetable mip tables are built
  once per sample rate and shared across instances (compile of the bench
  project 902 ms → <1 ms warm / ~115 ms cold), the 2x oversampler stages use
  block-based polyphase history instead of per-sample register shifts
  (bit-identical output), the true-peak meter evaluates the two cascaded
  stages as one composite 4x polyphase bank (16.1 µs → 3.3 µs per 128-frame
  stereo block, values within float tolerance), EQ bands process the paired
  left/right `BiquadF64` recurrences interleaved in one loop (bit-identical),
  and the export K-weighting meter folds both channels in a single pass.
  Exported WAV bytes are unchanged.

### Known gaps (tracked for later milestones)

- Per-node plugin deadline watchdog, native platform publication, release
  signing/notarization, GPUI preview and full release endurance gates remain.
  In-process native plugins are trusted code, with no crash/hang isolation.
- The historical M4 soak used a finite timeline without looping; its long
  idle portion does not establish sustained active-DSP performance. The
  harness now loops content, and fresh sustained-load release evidence is
  still required.
