# Studio titlebar and independent Pattern rack — 2026-10-05

The workspace opens Arrange with a separate Patterns window and no piano dock.
The resource Browser remains independent and can float or dock on the left, with
resizing and a direct return-to-floating action. The titlebar provides Pattern
selection, previous/next, BPM, metronome, real Master waveform/Peak/RMS and one
Views dropdown. Keyboard help is available through the native Help menu and `?`.
About uses the embedded brand mark, build version, architecture and project/license links.

Pattern selection, window layout and metronome are local runtime state. BPM edits
retain the existing Document Service transaction, accepted revision, Undo and Save
path. Multi-point tempo maps and tempo automation are displayed without replacing
their definitions. The rack shows actual channel parts and notes, with instrument
and piano navigation. It is not a new pattern creation API or step sequencer.

Master samples come from the existing post-limiter telemetry cache with one ring
consumer. The two meter rows are explicitly Peak and RMS. Click buffers are
prepared when compiling the Preview graph off the audio thread; toggles cross the
existing bounded command queue and apply at a block boundary without seeking the
transport. Both direct and buffered modes use the same graph operation. Preview
retains the metronome state across graph replacements; export defaults are unchanged.

## Review and validation

- Required formatting, lint, TypeScript checking, Rust formatting and workspace tests
  passed: 665 tests passed, 3 benchmark tests ignored. After the final UI refinements,
  the Preview/bench targets were checked again: 82 tests passed, 3 ignored.
- Offline PCM verifies metronome silence/click output, unchanged cursor progression,
  and zero allocations/deallocations around toggle plus render. Queued direct and
  buffered processing produce identical samples, including enable between beats.
- Native studio smoke verifies the initial workspace, pointer and keyboard Pattern
  navigation, titlebar BPM accept/Undo/Save, Browser dock/float, runtime metronome,
  live Master samples, rack piano/instrument navigation, native About action and
  modal isolation. The disposable source is saved and reopened.
- The complete 18-scenario native UI suite passed at 1440 × 920 dark and
  1060 × 720 light, including view menus, library/configuration, internal windows,
  piano, automation and close flows. Studio and About screenshots were inspected;
  live Master capture contains 1,024 real samples with nonzero waveform and RMS.
- The existing close-review smoke still expected a text-sized source configuration
  button. Its geometry assertion now matches the shipped 28 px icon, checks height
  and viewport containment, and retains native pointer/configuration-scope assertions.
  Dirty close, cancel, failed Save retention and successful retry passed in both themes.
- No TS/native protocol version changed and no generated bindings were edited.
  Musical edits continue through Document Service rather than direct DSP setters.

## Performance and evidence boundaries

`realtime-soak --simulated --metronome --seconds 15 --tracks 8` ran on Apple M5 Max,
macOS 27.0.1, 48 kHz, 128 frames, buffered mode with four render-ahead blocks.
The run rendered 5,647 blocks and queued 59 metronome toggles: zero xruns, deadline
misses, NaN blocks or queue drops. Worker p95/p99 histogram upper bounds were both
524,288 ns; the block deadline was 2,666,666 ns. Maximum sampled engine-load EMA
was 0.1596. This is worker wall time, not CPU utilization or HAL callback timing.
The raw record is `benchmarks/results/2026-10-05-m5max-metronome.json`.

All audio checks use offline rendering or a simulated sink, with no system output
device. Native interactions use GPUI keyboard dispatch and targeted NSEvents.
They do not establish physical-device performance or the longer release soak gates.
Screenshots and native logs are under `target/ui-review/`; artifacts are not committed.

New UI modules separate composition, selection, menus, drawing, sidebar and About.
The existing realtime session/worker modules receive only a bounded scalar command
branch; unrelated ownership and platform lifecycles were not refactored. The existing
single-workload soak harness is 322 formatted lines because argument parsing, fixture
setup and its JSON evidence remain together; it adds only the metronome scenario.

Reproduce with `scripts/build-preview.mjs --debug`, `scripts/smoke-ui.mjs`, the required
workspace checks, and the simulated benchmark command above. The earlier view-menu
report describes the intermediate six-entry menu; this report and the specifications
describe the final eight-entry menu with Patterns/About and no titlebar help button.
