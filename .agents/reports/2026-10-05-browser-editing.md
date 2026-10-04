# Sample Browser and DAW editing

Browser now has a compact 36 px header with 24 px controls, a lazy sample folder tree,
native folder selection, keyboard navigation/refresh, and file/project sample drag payloads.
File enumeration runs off the UI/audio threads, filters supported audio formats and
symlinks, and bounds directory and visible-tree work. Added folders are session-local.

Dropping onto an Arrangement lane or named plugin sample slot stages one Document
transaction. Rust decodes and caches the audio in owned content-addressed WAV assets;
source uses `Project.useSample` with builder addresses and portable asset metadata.
Sampler, Convolver, Slicer and existing named plugin resources retain their instance,
parameters and automation. Unsupported VST3 host resources and explicit Slicer markers
are rejected. Existing plugin identities are re-resolved at drop time; a detached or
replaced plugin view cannot target its former slot. Undo retains immutable cache data.

Arrangement adds P/B/E tools, interpolated brush/erase strokes, marquee and toggle
selection, group movement/duplication, clipboard, nudging and duration shortcuts.
Piano adds an application clipboard across patterns, using the same Notes transaction
and acknowledged-selection path. Command-D and Shift-drag remain fast duplication;
Command-C/X/V paste at the last editor cursor, with Shift-V retaining original timing.
Copies preserve complete placement settings and channel membership. Group addresses
resolve against the pre-edit order, with isolated preflight before any mutation.
Short clips stop shrinking together at the minimum duration instead of growing or
distorting their relative lengths. Escape clears an idle Arrangement selection.

Reference workflows: [FL Studio shortcuts](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/basics_shortcuts.htm),
[FL Studio Playlist](https://cluster.image-line.com/fl-studio-learning/fl-studio-online-manual/html/playlist.htm),
and [Ableton Live shortcuts](https://www.ableton.com/en/manual/live-keyboard-shortcuts/).
This covers the listed editing operations; source-offset trimming, phase-preserving
split and arbitrary VST3 file injection remain outside the current execution contract.

## Review and evidence

- Formatting, ESLint, workspace TypeScript checking and Rust formatting passed.
- Rust workspace: 666 passed, 3 intentionally ignored benchmark tests. The final
  Preview target was rerun after selection/resize refinements: 83 passed, 3 ignored.
- Protocol: 64 tests; Core: 210 tests; CLI: 155 tests in the full run, followed by
  4 focused sample-drop tests including the added sampler Save/reopen case.
- New tests cover atomic failed batches, stable addresses across removals/moves,
  cut/paste configuration retention, sampler instance/automation preservation,
  convolver host controls, rejected destinations, asset directory symlinks, native
  import, source Save/reopen and Undo/Redo. No tests open a system audio device.
- All 20 native UI scenarios passed: 1440×920 dark and 1060×720 light Browser/file
  drops, group brush/copy/cut/paste, Piano clipboard, studio/views, library,
  embedded windows, existing piano/automation/effect editing and close/save flows.
  Inputs are GPUI keystrokes and targeted NSEvents with simulated audio. Screenshots
  were inspected in both themes; this is not physical-device or listening acceptance.
- The old floating Browser smoke used a fixed row coordinate. It now locates the
  actual Pattern row bounds and retains real pointer-drag and saved-source assertions.
  The close-state fixture now includes the already-required projectRoot wire field.
- `target/ui-review/results.json`, PNGs and logs contain local UI evidence and are
  intentionally not committed. The unsigned development bundle was rebuilt.

No callback/DSP code changed. The focused offline Playlist benchmark at 48 kHz/128
frames measured p99 134625 ns (32 placements) and 125916 ns (1024), versus a
2666667 ns block interval. No HAL callback or xrun result is implied. CPU/platform,
command and timings are in `benchmarks/results/2026-10-05-m5max-browser-editing.json`.

New responsibilities are split between tree state/view/navigation, drag destinations,
plugin slots, selection, brush, clipboard, group projection, shortcut dispatch and
source transactions. New handwritten modules remain below roughly 300 formatted lines.
Existing Project/Document owners receive only small method hooks; transaction and
import implementation lives in dedicated modules. Generated JSON schemas are rebuilt;
native bindings, bundles and build output are not hand-edited or committed.
