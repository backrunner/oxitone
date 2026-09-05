# Oxitone code review, 2026-09-05

The existing implementation is not ready to be treated as correct or realtime safe. Existing checks pass, but 17 targeted Rust assertions fail and a separate TypeScript probe demonstrates unchanged revisions after authoring mutations. Findings below distinguish reproductions from static call-path analysis. No production implementation was changed during this review.

## Scope and verification

- Reviewed repository architecture and specifications 01 through 08, with emphasis on TS contracts, native bridge, graph validation, scheduling, automation, DSP integration, mixer, sample decoding, export, realtime queues, device lifecycle, and benchmark validity. Inventoried 196 source files / 33,430 lines; this is a broad review, not a proof that every remaining branch is correct.
- Read the local oxitone-guard skill. No Git metadata is available in this workspace, so commit history and change attribution could not be checked.
- `pnpm lint`, `pnpm typecheck`, `cargo fmt --all --check`, and the original `cargo test --workspace` all passed.
- `pnpm test` passed: protocol 37, native 5, core 82, MIDI 6, total 130. Rebuilt native and TS outputs with `pnpm build:native` / `pnpm build`, then reran the TS tests successfully against the rebuilt addon.
- Added temporary targeted probes, ran `cargo test -p oxitone-render --test review_probes -- --test-threads=1`: 17 tests failed as expected from the defects. Archived their source beside this report, outside Cargo's normal test discovery. These are reproductions, not fixes.
- Apple M4, macOS 27.0, aarch64, Rust 1.98.0. Ten-second simulated realtime run: 48 kHz, 128 frames, 8 tracks, renderAheadBlocks=4, 0 xruns/deadline misses, worker histogram p95/p99 262,144 ns. This is not a real-device long-duration acceptance test or a HAL callback timing measurement.
- Offline Criterion `render_wav_8tracks_20s`: initial mean 1.1214 s (~17.8x realtime); separate follow-up mean 2.1091 s (~9.5x), with substantial variation. Neither meets the documented 20x target on this run. The variation prevents attributing a precise regression to code; baseline noise and workload validity need investigation.
- Logs: `/tmp/oxitone-review-probes.log`, `/tmp/oxitone-review-ts-fresh.log`, `/tmp/oxitone-review-bench.log`, `/tmp/oxitone-review-bench-isolated.log`, `/tmp/oxitone-review-realtime.json`.

## Findings

### 1. P1: SPSC command queue has multiple concurrent producers

Locations: `crates/render/src/realtime/session.rs:172`, `:491`, `:654`, `:731`; `crates/render/src/realtime/ring.rs:140`.

The N-API/control caller and device monitor both call Shared::send on the same SpscQueue. push reads and later stores the write index without multi-producer arbitration. A device notification racing a transport/parameter/compile command can cause both writers to mutate the same Option slot through UnsafeCell, losing commands and invoking undefined behavior. The later worker mutex only wakes the worker and does not serialize push. Static call-path finding; do not rely on stress tests to disprove this race. Use a proven bounded MPSC implementation or serialize control-side producers before entering a properly split SPSC queue.

### 2. P1: Legal deep restart-chance source panics during evaluation

Location: `crates/transport/src/automation/chance.rs:140` and `:51`.

The fixed 160-byte seed buffer is too small for all accepted sources. A depth-64 AST (63 map wrappers around a restart chance), with project/source seed 9007199254740991, passes compilation and then panics: `range end index 167 out of range for slice of length 160`. Both seeds are valid JS safe integers. A buffered worker can die; direct mode reaches this through the extern C audio callback and can abort the process. Size from the supported maximum, precompute fixed seed material, and validate the full bound. Probe: `valid_deep_restart_chance_does_not_panic`.

### 3. P1: Small beat conversion changes values and reverses the timeline

Location: `crates/core/src/beat.rs:136`; used by `crates/transport/src/tempo.rs:185`.

Clamping exp2 without compensating the mantissa changes the represented number. At 120 bpm / 48 kHz, 1/24000 beat converts to 0.005333333333333333 instead of 0.000041666666666666665, a 128x error. frame_to_beat(2)=0.005333333333333333 but frame_to_beat(3)=0.004, violating monotonicity. This conversion is on runtime tempo/automation/sample paths despite its display-only comment. Probes: `small_beat_roundtrip`, `inverse_tempo_is_monotone_at_start`.

### 4. P1: Audio processing allocates and frees heap memory

Locations: `crates/render/src/build.rs:134`, `dispatch.rs:124`, `channel.rs:100`, `params.rs:384`, `realtime/direct.rs:80`, `realtime/worker.rs:278`.

Note/parameter/swing staging starts with Vec::new; callback pushes allocate. The resolved parameter queue is an initially empty VecDeque and inserts allocate on the audio thread. Beat-synced channel inserts construct a fresh updates Vec per block. Both graph replacement paths resize buffers and destroy the old graph on the render thread. Sampler/Slicer reset reconstructs entire voice pools (`crates/instruments/src/sampler/instance.rs:221`, `slicer/instance.rs:235`), and transport seek calls these resets on worker/direct paths. Allocator instrumentation measured one allocation in the first simple synth block and one on first resolved-parameter insertion. Preallocate bounded pools, reject overflow, and return retired graphs/state to a control thread. Probes: `first_audio_block_does_not_allocate`, `resolved_parameter_insertion_does_not_allocate`.

### 5. P1: Full command queue silently loses transport and shutdown commands

Locations: `crates/render/src/realtime/session.rs:172`, `:482`, `:839`.

send returns no error when push fails. transport still returns a predicted successful state; compile still updates the parameter index/revision even if ReplaceGraph was dropped. If Shutdown is dropped, Drop joins a worker that was never told to exit. This can hang dispose/the Node caller; a lost replacement also permits later parameter indices to target the old graph. Static finding. Required transport acknowledgements and shutdown must not use best-effort delivery.

### 6. P1: Recompiling during playback stops and rewinds playback

Locations: `crates/napi/src/lib.rs:198`, `crates/render/src/realtime/worker.rs:276`, `direct.rs:78`.

The newly compiled graph starts Stopped at frame 0. Replacement adopts that state without preserving the current transport/loop position. Reproduced with a simulated session playing past frame 2048: replacing the same project yields Stopped. Probe: `replace_graph_preserves_playback`.

### 7. P1: Replacement leaves session tempo and device chain stale

Locations: `crates/render/src/realtime/session.rs:502`, `:516`; `worker.rs:276`.

The session keeps its original CompiledTempoMap, project sample rate and monitor block size. Replacing 120 bpm with 60 bpm still resolves beat 1 to frame 24000 instead of 48000. Replacing block size/sample rate also leaves device conversion buffers/resampler settings stale; direct mode can become permanently silent on frame-count mismatch. Probe confirms stale tempo: `replace_graph_refreshes_tempo_mapping`. Device-chain consequences are from static analysis. Publish matching control metadata atomically and rebuild or reject incompatible device configuration changes.

### 8. P1: Block-local automation boundaries and timed parameters are ignored

Locations: `crates/render/src/graph.rs:241`, `bindings.rs:97`.

Parameter events only drain when frame <= block start and bindings evaluate only at the block start. There is no graph segmentation at source discontinuities and no branch for audio-rate specs. At block size 128, a mute event for frame 64 is still pending after rendering frame 127; a mute gate edge at frame 120 does not mute frames 120..127. This breaks sample-accurate timing and changes output with block size. Probes: `parameter_inside_block_applies_at_target_frame`, `gate_boundary_inside_block_is_sample_accurate`.

### 9. P1: Transport loop wraps only after rendering audio beyond its end

Locations: `crates/render/src/graph.rs:235`, `transport.rs:179`.

process_block schedules/renders the full block before advance wraps the cursor. Loop [0,64) with a 128-frame block incorrectly plays a note at absolute frame 96. It neither splits at the loop edge nor renders the repeated beginning into the remainder. Loop iteration context is also never advanced for restart chance. Probe: `transport_loop_does_not_render_events_past_loop_end`.

### 10. P1: Track stems contain wrong audio or silence

Locations: `crates/render/src/render_wav.rs:228`, `:271`, `:282`; `graph.rs:171`; `crates/mixer/src/bus/mod.rs:387`.

An empty track has an empty bus list and takes the master-output branch: reproduced at -8.759 dBFS instead of silence. A track routed directly to mix_master receives no stem tap and exports digital silence despite audible master audio. Tracks sharing a bus receive the same already-mixed bus, so their stems cannot isolate each track. Need explicit output kind and genuine per-track source isolation with documented shared-return treatment. Probes: `empty_track_stem_is_silent`, `track_routed_directly_to_master_has_audible_stem`.

### 11. P1: MIDI export ignores the effective tempo lane

Locations: `crates/render/src/midi/tempo.rs:104`, `midi/mod.rs:234`.

The conductor reads raw tempoMap and explicitly skips tempo lanes, contrary to 06-format-and-export.md. A snapshot with a 120 bpm map and constant 60 bpm lane renders at 60 bpm but exports a 500000 microseconds/quarter MIDI tempo (120 bpm). Probe: `midi_export_uses_effective_tempo_lane`.

### 12. P1: Mixer insert mix and bypass are silently ignored

Location: `crates/mixer/src/bus/mod.rs:117`; `bus/process.rs` insert processing.

EffectRef accepts mix and bypass, but InsertSlot stores neither and the mixer always processes full wet. A bypassed Utility polarity insert still inverts a positive signal in the reproduction. This affects mixer buses and Master, while channel inserts implement a separate path. Probe: `mixer_insert_bypass_is_honored`.

### 13. P1: Extensible WAV valid bits replace the storage width

Locations: `crates/samples/src/decode/wav.rs:57`, `:112`.

wValidBitsPerSample is assigned to fmt.bits and subsequently controls bytes-per-sample stride. A valid 24-bit signal stored in a 32-bit WAVE_FORMAT_EXTENSIBLE container is read in three-byte steps: four frames decode as five with corrupted values. Preserve container bits/blockAlign separately from valid precision, and honor left-aligned valid bits. Probe: `extensible_wav_24_valid_bits_in_32_bit_container`.

### 14. P2: Note chance is accepted but never applied

Locations: `crates/transport/src/scheduler.rs:226`, `crates/render/src/midi/expand.rs:188`.

Only clip.probability participates in the random decision. note.chance is validated and serialized but ignored in both execution paths. A note with chance=0 still produces scheduler events. Probe: `zero_note_chance_produces_no_note_events`.

### 15. P2: Exclusive clip end does not trim crossing note-offs

Locations: `crates/transport/src/scheduler.rs:239`, `crates/render/src/midi/expand.rs:199`.

Both paths compute off_beat = on_beat + duration without clipping it to the clip end. A four-beat note in a clip ending at beat 1 schedules note-off at frame 96000 instead of 24000. Existing tests named for clipping only assert note-on positions. Probe: `exclusive_last_beat_clips_note_off`.

### 16. P2: Authoring level/pan changes do not increment revision

Location: `packages/core/src/channel.ts:89`, `:103`.

Both public setters change snapshot contents without Project.touch(). Direct Node probe: revision stays "1" while level changes 1 -> 0.25 and pan 0 -> 0.75. Revision-based consumers may drop legitimate updates. Other exposed nested authoring objects (instrument parameters, marker returned by addMarker, lane.target) are also mutable without tracked setters. Snapshot mutation tracking needs a consistent ownership policy.

### 17. P2: Ten-minute soak does not sustain the advertised synth workload

Locations: `crates/bench/src/bin/realtime-soak.rs:116`, `crates/bench/benches/render_offline.rs:61`.

The soak always builds 32 beats at 120 bpm (16 seconds) and starts playback once without looping/extending the arrangement. After note releases/tails decay, most of a 600-second run measures an idle synth with continuing bus DSP. The graph-block Criterion benchmark similarly advances past its finite 20-second arrangement across warmup/iterations. Existing zero-xrun reports therefore do not establish ten minutes at the advertised voice load. Extend the score to measurement duration or use a verified persistent workload, and assert ongoing voice activity.

### 18. P2: Listener-registration failure leaks the HAL proc and callback state

Location: `crates/io-macos/src/stream.rs:294`.

After AudioDeviceCreateIOProcID succeeds, ListenerGuard::new uses `?` before an OutputStream guard owns proc/state. Registration failure exits without AudioDeviceDestroyIOProcID or Box::from_raw(state). In direct mode graph recovery then fails because the leaked callback still owns the graph. Static error-path finding. Acquire resources under RAII guards before further fallible setup.

### 19. P2: Dry/wet and bypass omit insert latency alignment

Location: `crates/render/src/channel.rs:235`.

Channel insert mix blends undelayed dry samples with the effect's delayed output. For oversampled/latency-reporting effects this comb-filters partial mixes. Bypass removes the effect latency while the compiled graph still compensates as if it were present, misaligning parallel paths. Static finding. Add per-insert dry/bypass delay compensation and test impulse alignment for mix=0/0.5/1 and bypass.

## Additional contract gaps

- `.agents/docs/08-plugin-abi.md` requires a C ABI, but `crates/graph/src/abi.rs` currently exposes Rust traits/slices/trait objects; built-ins have a private constructor path in `build_plugins.rs`. Treat this as an unimplemented milestone, not a stable third-party binary ABI.
- TS Project generates a random-looking mixer ID for its claimed Master, while Rust reserves `mix_master`; the TS bus is actually an ordinary bus routed into the implicit Rust Master. Also, the TS builders still expose only Master routing, empty effect chains and no sample authoring. M2/M3 native coverage must not be confused with complete SDK authoring support.
- Current negative phase support, tempo lane loop/lastBeat baking, track membership consistency, extreme numeric resource limits, and CoreAudio aggregate/multichannel buffer layouts need focused follow-up. No claim of correctness is made for these paths.
- Device removal/reconnect recovery was inspected statically but not exercised with physical hardware. No sanitizer/Miri/fuzz campaign or fresh ten-minute/sixty-minute real-device soak was run.

## Reproduce and repair order

The archived `review_probes.rs` expects to live temporarily at `crates/render/tests/review_probes.rs`, beside the existing `common` fixture module. Restore it there with apply_patch, then run:

```sh
cargo test -p oxitone-render --test review_probes -- --test-threads=1
```

All 17 assertions currently fail. Keep each reproducer as a focused regression test when its fix lands. The allocator probe counts only allocations/reallocations on the measured thread; it does not count deallocations or prove general RT safety.

Repair concurrency and panic paths first, then beat conversion and realtime allocation/retirement. Next repair graph replacement metadata/transport, block segmentation and loops, export/decoder behavior, and SDK revision tracking. Fix the benchmark workload before using performance reports as release evidence. Implementation changes must update the corresponding specification where behavior/contracts change and run the repository's full required checks plus relevant benchmarks.
