# VST3 offline workbench verification

The optional SDK and disposable macOS helper now back a session-only Plugin Library workbench:
explicit bundle/class selection, Inspect, paged normalized parameters, input/output WAV, content/tail/tempo,
instrument test notes, render and cancellation. Native vendor editors, realtime inserts, project persistence,
multi-bus/sidechain and system scanning remain outside this increment. No callback code changed.

## Evidence

- `pnpm format:check`, `pnpm lint`, `pnpm typecheck`, `cargo fmt --all --check` passed.
- `pnpm test` and `cargo test --workspace` passed; focused follow-up tests cover the subsequent SDK response
  identity and cancellation-history changes. `cargo test -p oxitone-vst3-host --features host` passes six native
  boundary/WAV tests. GPUI form tests cover input requirements, numeric bounds and note-event serialization.
- Disposable helper fixtures exercise crash, timeout, abort, malformed/versioned/oversized replies,
  stdout isolation, create-only publication, staging cleanup and mismatched response identity.
- CLI fixtures prove static npm discovery without running package entry code, package path containment,
  inherited policy, hash/configuration pinning, readonly/unknown parameter rejection, no MIDI on effects,
  session budgets and refresh invalidation. Cancellation remains deliverable with 64 ordinary requests queued;
  over 300 cancel replies preserve the running render's idempotency and expire old completed replies.
- `scripts/smoke-vst3.mjs --gui` used the installed local Vesti Gain class
  `56455354494741494e30303030303031`, hash
  `a2ec83f9537b03c694f98ee6440bf66d28d722e1d74ba33a30fd7410115246aa`.
  Real helper PCM tests verified state restore, mono duplication, content ending at frame 129 inside a block,
  seven zero-input tail frames, output preservation and wrong hash/class/state/parameter/symlink rejection.
  GPUI used actual field/button hit tests and clipboard keyboard input, rendered one second plus 0.1 second
  tail, then verified PCM, source/revision stability and refresh invalidation. The screenshot
  `target/daw-vst3.png` was visually inspected; the parameter form and actions are visible without overlap.
  All audio was offline or simulated; no listening, system output or vendor-editor validation occurred.
- Release benchmark on Apple M4: three warmups, 20 measurements, 48 kHz/128 frames, bypassed mono 0.25
  input. Whole-helper inspection p95 **11.16 ms**; one-second WAV render p95 **13.24 ms**. Initial baseline:
  `benchmarks/results/2026-09-18-vst3-offline-helper.json`. This includes process startup, bundle hashing,
  loading/state/DSP/WAV/teardown with warm filesystem cache; device/callback/CPU utilization/xrun are null.
  These figures do not establish realtime readiness or compatibility with other vendors.

New source files keep native hosting, configuration, events and WAV separate; UI model, input, controls,
layout and smoke are separate. Existing large ProjectDocument/DocumentUi/capture orchestration files receive
only delegating methods/branches; unrelated source-writing work in the working tree is preserved.
