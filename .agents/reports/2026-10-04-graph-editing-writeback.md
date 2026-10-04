# Graph editing and plugin source writeback — 2026-10-04

## Delivered

- Graph surfaces select the nearest node and edit it directly. EQ XY edits frequency/gain;
  Option/Alt-drag changes Q alone. Selected values appear below the graph; the matching knobs remain available.
- Compressor threshold/makeup and Option/Alt knee, Gate threshold, Compactor threshold/output,
  limiter ceiling, and multiband crossover/trim nodes use the same document transaction path.
- Shift provides fine adjustment; double-click resets bound values; Escape cancels. No movement
  creates no revision. An XY gesture commits both parameters together on release and supports Undo/Redo.
- VST3's explicit capture action is now **Apply state to code**, with a visible explanation of
  temporary native-editor changes and subsequent project Save. Capture checks both the response
  identity and the nested configuration class/hash against the registered plugin.

## Source ownership and persistence

Graph/dial gesture → instance-scoped `DocumentOperation::Configuration` → Node source writer →
candidate evaluation and native graph compile → complete-project equivalence check → accepted
document revision → Preview graph. Save persists the source through the existing journal.

Plugin parameter keys are literal IDs: `band.2.gain` is not a nested property. Host Mix/Bypass
use a separate operation from plugin parameters, even when a plugin calls a parameter `mix`.
Factory calls, imports and shared definitions remain intact when editing a local instance.
Invalid values, stale revisions, mismatched instances and invalid native candidates are rejected.

VST3 vendor editor changes affect the current runtime instance until explicitly captured.
`Apply state to code` writes `pluginConfig(kind, original).replaceParameters(values).withState(state)`:
the returned normalized parameter table and opaque preset state are both retained. Source reads,
document revision, runtime generation, instance identity and plugin class/hash are checked.
Ordinary source controls author normalized VST3 values; they do not claim to display a vendor's
effective/clamped/automated value. C ABI parameters use their declared descriptor units.

## Evidence

- `pnpm format:check`, `pnpm lint`, `pnpm typecheck`, `cargo fmt --all --check`: passed.
- `cargo test --workspace`: **663 passed, 3 ignored**, 92 suites after the review fix.
- CLI suite: **152 passed**, 47 files. The expanded external configuration test was rerun after
  adding the independent PCM oracle and passed.
- Native `--edit-graph` captures passed for EQ, Filter, Wavetable ADSR and Compressor in dark and
  light themes. They check native hit testing, nearest-node selection, XY/Alt projection,
  one revision per gesture, instance isolation, Undo/Redo, Escape, no-op, reset and Save/reopen.
- Narrow light captures passed for EQ, Compressor, Multiband and Limiter. Screenshots inspected
  included EQ, Compressor and narrow Multiband; the narrow layout scrolls its vertically stacked plots.
- `external-configuration.test.ts` compiles a real two-parameter C dynamic library in a temporary
  npm package. It tests dotted IDs, plugin/host mix separation, atomic changes, rejection of invalid
  values/stale revisions/wrong instances, unchanged sibling/master inserts, Undo/Redo and Save/reopen.
  The independent dry/wet gain oracle requires sample error below **2e-6** and non-silent output.
  Undo and reopened renders match the corresponding float32 WAV bytes exactly. npm JS and binary
  bytes remain unchanged. No system audio output is opened.
- `smoke-vst3-configuration.mjs`: passed real helper/fixture dynamic buses and parameter tables,
  read-only/unknown parameter rejection, state restoration, bulk 4096-parameter overrides,
  retired generation rejection, live instance capture, Undo/Redo and Save/reopen.
  Detailed report: `target/vst3-configuration.json`.
- Installed **VestiGain.vst3**, `smoke-vst3-live-daw.mjs --editor`: passed current native editor
  open/close, two independent instances, live capture, state restoration, Undo/Redo and Save/reopen.
  Restored offline PCM peak error **3.815415174773795e-9** across **48,000 samples**.
  The simulated playback snapshot reported **0 xruns, 0 plugin faults**.
  Detailed report: `target/vst3-live-daw.json`. Parameter changes were sent through the real host
  control API; this is not manual vendor-editor gesture acceptance or a commercial-plugin matrix.

## Performance and limits

Apple M5 Max, macOS 27.0.0; release source-plot benchmark, 48 kHz parameter model,
26 effect diagrams per batch, 100 warmups / 1,000 samples: **p95 133.166 µs,
p99 153.833 µs** in the pre-commit rerun. This measures source model construction, not GPUI frame time or audio callback time.
VST3 live verification used 48 kHz / 128 frames and a simulated stereo sink.

UI changes add no work to the audio callback. During a graph drag, the UI previews source values;
the audio graph changes after transaction acceptance on release. These are source response diagrams,
not live FFT or gain-reduction telemetry. Arbitrary third-party DSP does not automatically acquire
a built-in EQ model. C ABI 1 still has no opaque state/resource editing support.
The passing fixtures verify the implemented paths; they cannot establish 100% compatibility with
every third-party plugin or validate unheard audio through physical devices.

Screenshots: `target/builtin-panels/graph-dark/eq.png`, `graph-light/eq.png`,
`graph-light/compressor.png`, and `light-narrow/` under the same directory.

## Pre-commit review

Found and fixed an ADSR interaction defect: recalculating the viewport during every pointer move
kept Release pinned to the right edge and made other time nodes drift from the pointer. The gesture
now holds the initial time axis and illustrative sustain hold fixed, with 20% of the viewport left
for extending Release. Automatic fitting resumes when the gesture ends.

A geometry regression checks time-node displacement and unchanged earlier segments. Native pointer
regressions check Decay/Release positions within one logical pixel, isolated Release edits,
Undo/Redo and saved Release recovery. The final graph capture and the existing scalar/synth editing
smokes passed. Formatting, lint, typecheck, workspace Rust tests, all 152 CLI tests, the focused
benchmark, VST3 configuration conformance and installed VestiGain live-instance conformance were rerun.
All 30 built-in panels passed the final opening smoke; the updated ADSR native pointer regression
also passed in the light theme.
The review also checked source transaction rejection, instance identity, descriptor units, audio
callback isolation and module boundaries; no additional blocking defects were found.
