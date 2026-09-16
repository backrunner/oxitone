# macOS development checks

`.github/workflows/macos.yml` runs on pushes to main and manual dispatch.
Nothing in the repository schedules periodic runs; Dependabot automated
security fixes are disabled at the repository level so it never spawns CI. It checks an Apple Silicon macOS 15 / Node 24 environment and an Intel
macOS 15 / Node 22 environment. The matrix asserts the actual architecture rather
than inferring it from a mutable runner label. Both builds target macOS 13+.

Each fresh checkout installs the repository's pinned pnpm version and frozen
lockfile. The pnpm store and Cargo registry/target are cached between runs;
cheap gates (rustfmt, prettier, repo-wide `eslint .` covering packages,
examples and tooling scripts) run before the builds so common failures stop
the job within a minute. It then builds the native addon, SDK, GPUI viewer bundle and TypeScript examples, checks
generated schema drift and types (`tsc --noEmit` resolves workspace packages
through built `dist` output, so it stays after the build), and runs all Rust/TS tests, the portable offline
examples (including the dynamic drum/effect chain) and focused Slicer, insert,
dynamic-effect, drum DSP and preview-telemetry benchmarks. Preview tests use a real
Unix socket and simulated sink to check watch, transport, rejection and recovery.
No native build output is reused from
the developer checkout. Failures stop the job; the 90-minute bound covers a
cold cache plus the Intel leg. The existing opt-in GPUI layout
benchmark is ignored by the normal Rust test run; functional tests are not skipped.

Tests never open system audio outputs. Native facade transport/latency tests use
`audioBackend: 'simulated'`, exercising the Rust worker, ring and PCM callback without
CoreAudio. Browser tests require `AudioContext({sinkId: {type: 'none'}})` and fail if
the no-device sink is unavailable; Chromium is also launched with `--mute-audio`.
They inspect nonzero PCM upstream of the silent sink, so muting does not weaken DSP
or AudioWorklet assertions. The CI workflow does not install or select audio devices.

Plugin suites build their C/Rust fixtures once in asynchronous setup (up to ten
minutes for a cold build). Functional tests keep separate bounded timeouts; compiler
work must not block Vitest's RPC loop. The root TS runner executes packages in order;
core, CLI and Wasm run one test file at a time with a 30-second default test budget
because their suites include compilation, WAV exports and filesystem round trips.
Longer CLI scenarios declare their own bounds; child processes run asynchronously,
and preview IPC failure always closes the connection and cleans up the viewer. This
avoids many workers preparing separate synth tables at once. These timeouts are
failure bounds, not performance acceptance. Avoid overlapping full Cargo/TS runs and
audio benchmarks when collecting timing evidence.

The native panel smoke deliberately injects syntax, runtime, missing-plugin and
invalid-layout failures. It labels each expected rejection and checks last-good
state/recovery. Raw logs remain under `target/*-panels-watch.log`; use `--verbose`
to stream them. Real failures print the raw log and exit nonzero. Web smoke uses
the dev server's default port 4173; CI passes its private port through `OXITONE_WEB_URL`.

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

Keep temporary logs, screenshots and test fixtures under `target/`. Routine cleanup
can remove Cargo compilation caches and temporary test environments while retaining
`target/examples` (generated songs and local plugin libraries), `target/criterion` (comparison baselines),
the preview bundles and the active local tooling. Compilation caches regenerate on
the next build; package `dist` and native bindings are needed to run the current SDK.

Small JSON reports in `benchmarks/results` are versioned; raw benchmark output stays
ignored. Historical measurements are not current acceptance evidence. In particular,
the old 10-minute soak did not maintain a full-song load; its limitations are recorded
in the [review](../.agents/reviews/2026-09-05/README.md).
