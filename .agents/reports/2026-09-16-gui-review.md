# GUI review: native DAW and plugin panels

Reviewed the working tree based on `1454e2ae864687053cfea2edb0d1ed9b9b9b754a`, using a fresh
debug app bundle and a rebuilt CLI with its workspace dependencies. This review follows the
source writeback review from the same date. Changes remain uncommitted.

Environment: Apple M4, macOS / Darwin 27.0.0, Node 26.5.0. Native scenarios use disposable
projects and simulated audio output. No audio device playback is part of this acceptance.

## Findings and fixes

- The plugin library detail drawer could show a partially cut metadata row in a small window
  without a visible scrollbar. Details and usage lists now use the existing draggable vertical
  scrollbar, with nonshrinking content. At 340 × 300 logical pixels, the regression clicks the
  actual scrollbar, reaches its maximum offset, verifies pointer capture ends, and verifies no
  document transaction was created. The screenshot shows the complete final SHA-256 row.
- Capture audio isolation previously depended on the DAW or transport scenario flags. Builtin
  panel captures did not set either flag. All capture runs now select the simulated sink at
  startup. `OXITONE_PREVIEW_SIMULATED=1` also enables persistent GUI review without opening an
  audio device.

## Visual assessment

The current interface meets the restrained desktop-tool direction in the inspected states:
neutral surfaces, compact toolbars, consistent typography and icons, and accent colors reserved
for selection, musical content and state. Plugin groups correspond to actual synthesis/effect
modules. There are no ornamental dashboard cards, marketing copy or decorative gradients.
The existing visual system was retained rather than introducing another visual style.

Reviewed Arrangement, Piano roll, Mixer, Automation, Browser, plugin library, configuration and
save-close dialogs in light/dark appearances at 1440 × 920 and 1060 × 720 logical pixels.
No blocking overlap, missing text or inaccessible primary action was found in these captures.
Intentional clipping inside scrollable editors is distinct from clipped fixed controls.
Palette tests cover text contrast of at least 4.5:1 and scope/meter signals of at least 3:1.

## Native acceptance evidence

- `node scripts/smoke-ui.mjs`: all 13 scenarios passed. Fresh run started
  `2026-09-16T19:35:59.599Z`; results and screenshots are under `target/ui-review/`.
  Coverage includes actual pointer hit testing and keyboard dispatch, note/velocity editing,
  track/mixer controls, clip actions, automation, internal window layout, plugin configuration,
  undo/redo, source save/reopen, modal input isolation, invalid-source recovery, native close
  interception and save-lock failure preserving the draft.
- After the drawer change, reran `smoke-daw.mjs` with `CAPTURE_MANAGER=1`,
  `CAPTURE_LIBRARY=details`, `CAPTURE_CONFIGURATION=1`, light appearance and 1060 × 720.
  The added scrollbar assertions and source save/reopen passed. Fresh evidence:
  `target/ui-review/library-details-small.png` and its `.png.log`.
- `node scripts/smoke-builtin-panels.mjs`: all 30 builtins passed, with fresh dark screenshots
  under `target/builtin-panels/dark/`. Inspected all panels directly or in contact sheets.
- Nine additional builtin scenarios passed: light 440 × 540 panels for EQ, Compressor,
  Multiband, Wavetable and Multisampler; dark narrow Wavetable Filter & output; light narrow
  Modulation; light Matrix; and Filter editing. The editing case verifies knob hit testing,
  live projection, shared-preset instance isolation, Undo/Redo, Escape cancellation, host
  Mix/bypass, no-op click and save/reopen in a fresh document process.

Selected screenshots:

- [Small light workspace](../../target/ui-review/windows-light-small.png)
- [Dark piano editing](../../target/ui-review/piano-dark.png)
- [Small light automation](../../target/ui-review/automation-light-small.png)
- [Plugin detail drawer after scrolling](../../target/ui-review/library-details-small.png)
- [Expected save-lock failure preserves draft](../../target/ui-review/close-dark-small.png)
- [Dark Wavetable](../../target/builtin-panels/dark/wavetable.png)
- [Narrow light modulation](../../target/builtin-panels/light-narrow-modulation/wavetable.png)
- [Filter after editing and saving](../../target/builtin-panels/editing/filter.png)

## Checks

`pnpm format:check`, `pnpm lint`, `pnpm typecheck`, `cargo fmt --all --check` and
`git diff --check` passed. `cargo test --workspace`: 546 passed, 0 failed, 3 explicitly
ignored benchmarks. This includes 69 Preview tests and both theme contrast palettes.

`cargo test -p oxitone-preview --release benchmark_editing_gestures -- --ignored --nocapture`
completed all 14 microbenchmark scenarios (100 warmups, 1000 samples each). For example,
10,000-note content projection measured p95 87.958 µs / p99 261.541 µs; selection restoration
at that size measured p95 9.746 ms / p99 29.069 ms. Other build processes were active, so this
is a functional/performance sample under contention, not an idle-system frame budget gate.
It does not establish smoothness for large projects. Device, sample rate, block size, callback
timing, xruns and GPU timing are not applicable to these UI/control-only measurements.

Logs from this run, including the full benchmark samples, are copied to
`target/ui-review/checks/`. Build outputs and captures remain ignored development artifacts.

## Acceptance limits

The native harness renders actual GPUI/AppKit windows and dispatches targeted NSEvents; it is
not a replacement for a physical mouse/trackpad session. A persistent simulated DAW was started
successfully, but the desktop automation service could not acquire its window
(`cgWindowNotFound`; another attempt timed out). Therefore macOS traffic lights/fullscreen,
physical gestures, IME and VoiceOver have not received manual acceptance in this run.
GPU frame timing, real device callback timing, xruns and audio listening are not measured here.
These limits prevent claiming exhaustive or unconditional GUI acceptance.
