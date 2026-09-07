# macOS development checks

`.github/workflows/macos.yml` runs on pushes to main, pull requests and manual
dispatch. It checks an Apple Silicon macOS 15 / Node 24 environment and an Intel
macOS 15 / Node 22 environment. The matrix asserts the actual architecture rather
than inferring it from a mutable runner label. Both builds target macOS 13+.

Each fresh checkout installs the repository's pinned pnpm version and frozen
lockfile, then builds the native addon, SDK, GPUI viewer bundle and TypeScript examples. It checks
generated schema drift, lint, types, rustfmt, all Rust/TS tests, the portable offline
examples (including the dynamic drum/effect chain) and focused Slicer, insert,
dynamic-effect, drum DSP and preview-telemetry benchmarks. Preview tests use a real
Unix socket and simulated sink to check watch, transport, rejection and recovery.
No native build output is reused from
the developer checkout. Failures stop the job. The existing opt-in GPUI layout
benchmark is ignored by the normal Rust test run; functional tests are not skipped.

Tests never open system audio outputs. Native facade transport/latency tests use
`audioBackend: 'simulated'`, exercising the Rust worker, ring and PCM callback without
CoreAudio. Browser tests require `AudioContext({sinkId: {type: 'none'}})` and fail if
the no-device sink is unavailable; Chromium is also launched with `--mute-audio`.
They inspect nonzero PCM upstream of the silent sink, so muting does not weaken DSP
or AudioWorklet assertions. The CI workflow does not install or select audio devices.

The workflow uploads environment information, silent browser reports/screenshots, Criterion
results and example snapshot/report JSON. Large WAVs and native binaries are not
published as release artifacts. Hosted-runner timing is diagnostic: the jobs do not
compare noisy virtualized measurements with the Apple M4 physical baseline.

These files establish the automation but are not evidence of a successful remote
run. Record the first successful runs in the milestone audit after CI executes.
Physical-device switching/unplugging, 10/60-minute sustained load, minimum macOS
runtime testing, performance budgets, npm installation, signing and notarization
remain separate release gates. The workflow uses no release credentials and does
not publish packages.
