# Persistent native VST3 stream

This increment implements an optional persistent helper and a preallocated Rust audio port. It is SDK
infrastructure, not an Engine insert or DAW transport integration. `vst3-host` 0.9.0's public process methods
take an audio-level mutex, and its event facade can call the controller. Those calls remain on the isolated
helper's main thread. No callback or existing RenderGraph code changed.

## Implementation

- Versioned TS start/readiness contracts and generated schemas describe control setup. PCM is a bounded
  native binary protocol, not JS messages. `stream` alone builds the client without the VST3 loader;
  `host,stream` builds the server. Ordinary SDK inspect/render/preset APIs retain their existing behavior.
- Native `Session::spawn` verifies initialization, starts a private process group/socket, creates 2–16
  reusable packets and owns a background IO worker. `RealtimePort::submit/receive/status` only perform
  bounded checks/copies and lock-free queue operations. No allocation, freeing, I/O, clock read or process
  operation occurs in those methods. Destruction of both handles is explicitly control-thread work.
- Startup checks exact class/hash, sample rate, block size, parameter identities and bus capabilities.
  Existing bundle signature/hash/state validation is shared with offline loading. The helper restores the
  initial state/parameters, activates Realtime processing, checks activation changes, then acknowledges.
- Blocks contain a sequence, actual frame count, stereo PCM and at most 256 parameter/note events.
  Same-frame events are stable-sorted note-off/parameter/note-on. Invalid input, unknown/readonly/nonautomatable
  parameters and unsupported notes never partially enter the queue. Full queues return Full without accepting
  a block or advancing the sequence. Short blocks clear the unused output region.
- One absolute deadline covers each IO transaction, including slowly arriving response fragments. Replies
  must match sequence/frame count and contain finite PCM. Timeouts, exit, corrupt replies and processor
  faults latch distinct statuses; no queued audio is delivered after a fault has been observed. Runtime
  topology/latency/controller changes require explicit reconstruction. Closing interrupts in-flight IO,
  kills the helper process group and reaps/joins it; normal close does not overwrite a recorded fault.

## Evidence

- `pnpm format:check`, `pnpm lint`, `pnpm typecheck`, `cargo fmt --all --check`, and `git diff --check` pass.
- Full `pnpm test`: **442 passed**. `cargo test --workspace`: **556 passed**, three intentionally ignored
  benchmarks. `cargo test -p oxitone-vst3-host --features host,stream`: **15 passed** (11 unit / 4 process tests).
  `cargo check -p oxitone-vst3-host --no-default-features --features stream` passes without the loader or warnings.
- Allocator instrumentation covers 10,000 submit/receive cycles plus empty/full/fault paths with **zero
  allocations and zero frees** on the calling thread. Binary tests cover partial buffers, bad/truncated
  headers and event offsets. Readiness tests cover unsupported versions/settings, duplicate and readonly
  parameters, identity pins and unsupported MIDI.
- A standalone Rust adversarial helper tests startup hang, processing hang, crash, wrong sequence, NaN,
  terminal plugin fault and slow byte-by-byte replies. Tests verify silence/fault latching and process reaping.
  Control close interrupts a pending 10-second request in under one second. The initial 500 ms budget for
  normal test-executable startup failed under parallel test/build load; normal startup now allows three
  seconds while a dedicated startup-hang test uses 50 ms. Runtime request deadlines were not relaxed.
- A release build of the actual helper and `vst3-stream-probe` passed `scripts/smoke-vst3.mjs --stream` with
  local Vesti Gain class `56455354494741494e30303030303031`, bundle hash
  `a2ec83f9537b03c694f98ee6440bf66d28d722e1d74ba33a30fd7410115246aa`.
  It processes **2,208 ordered blocks**, including 17-frame blocks, four queued requests, initial opaque
  configuration/parameter setup, a bypass event at the final block frame, and two independent process
  lifecycles. The expected PCM is 0.25 on both channels; short-block remainder is zero. The existing offline
  identity/state/mono/tail/create-only smoke also passes with the refactored shared loader.

## Native stream measurement

Archive: `benchmarks/results/2026-09-18-vst3-native-stream.json`, including raw samples. Apple M4,
Darwin 27, Rust 1.98.1 / LLVM 22.1.8, release, 48 kHz / 128 frames / stereo, queue depth 4,
constant 0.25 input, bypass=1. Warmup 100 blocks and 1000 timed blocks; no simultaneous builds/tests.

| Measured operation                           |   p95 ms |   p99 ms |   max ms |
| -------------------------------------------- | -------: | -------: | -------: |
| Queue, worker, isolated processor and return |    0.167 |    0.172 |    0.233 |
| Port submit                                  | 0.000291 | 0.000333 | 0.000792 |
| Successful port receive                      | 0.000250 | 0.000333 | 0.000833 |

Startup was 19.13 / 10.33 ms for the two sessions. The caller busy-polls while the IO worker polls an idle
queue at 100 microseconds; this microbenchmark is not a scheduled callback. Sub-microsecond port timings
are near timer/measurement overhead. Device, callback p95/p99, CPU utilization and xruns are unmeasured/null.
No audio device, listening test, graphical editor or other-vendor compatibility test was performed.

## Remaining boundary

The queue is asynchronous and its capacity is not fixed audio latency. The caller must associate replies
with sequences; Full cannot be treated as an accepted/dropped block while continuing the old musical clock.
The helper currently runs a continuous timeline from zero with fixed tempo/signature and playing=true.
Graph integration still needs scheduling with a declared latency, late-result disposal, epochs, seek/loop
reset semantics, PDC, retirement, and a session-level failure policy. No VST3 source registration, realtime
DAW assignment, tempo map, output MIDI or native vendor editor is exposed in this increment.

New native modules separate wire contracts, binary framing, queue port, process lifecycle, server and tests;
all are below 300 formatted lines. Existing unrelated working-tree changes were preserved. The previous
source-daw Save benchmark observation remains unresolved; this increment neither changes that path nor
claims its performance regression gate passed.
