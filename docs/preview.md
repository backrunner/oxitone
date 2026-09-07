# Code-driven project preview

Oxitone Preview is a native GPUI viewer. TypeScript controls the project; the viewer
provides transport and display controls. It never writes notes, mixer settings or
automation back to the authoring model.

## Start from this checkout

```sh
pnpm install --frozen-lockfile
pnpm build
pnpm build:preview
pnpm preview examples/offline/src/preview.ts
```

`build:preview` builds the pinned GPUI revision and an unsigned development
`target/release/Oxitone Preview.app`. Runtime Metal shader compilation works with
Xcode Command Line Tools; the separate `metal` compiler is not required. For a faster
debug build use `node scripts/build-preview.mjs --debug`. Native UI builds are macOS
only. GPUI enables both `font-kit` (native fonts, including fallback for multilingual
text) and `runtime_shaders`; the GUI rejects an empty system font catalog rather than
silently drawing a window without text. Signed npm platform distribution and minimum-macOS runtime acceptance remain
release gates.
Rebuilding replaces the bundle executable atomically with a fresh inode, avoiding
macOS code-signature cache failures after overwriting a previously launched binary.

The drum-machine project uses real dynamic instrument and effect libraries:

```sh
pnpm example:drums
pnpm preview examples/drum-machine/src/preview.ts
```

Change `examples/drum-machine/src/song.ts` while it plays. The runner watches local
static imports, executes a fresh Node process, and sends each changed snapshot to
the native viewer. Compilation and graph swaps use the Rust engine; no JavaScript
executes in its audio worker or CoreAudio callback.

## Entry contract

Use an ESM TypeScript project (`"type": "module"` in package.json, or a `.mts` entry).
Default-export a Project or a sync/async factory returning one. Named `createProject`
and `project` exports are accepted too:

```ts
import { Project, Pattern, wavetable } from 'oxitone';

export default function createProject() {
  const project = new Project({ name: 'Live code', seed: 42 });
  const keys = project.addChannel({ name: 'Keys', instrument: wavetable() });
  project.addTrack('Phrase').use(keys).add(new Pattern({ lengthBeats: 4,
    notes: [{ pitch: 60, start: 0, duration: 1, velocity: 0.8 }],
  })).at({ bar: 1 }).loop(8);
  return project;
}
```

Factories should describe the music without starting playback themselves. They run
again on every change. Importing the original source preserves `import.meta.url`;
static dependencies are watched through esbuild, while npm packages are external.
Return `{project, assetBaseDir}` to resolve relative sample URIs from a particular
directory; a loaded Project's resource directory is retained automatically.

Register dynamic libraries with `Project.registerPlugin` before returning the
Project. Paths, checked hashes and explicit plugin policy travel separately from
the musical snapshot. Library discovery and runtime downloading are not performed.

Useful options:

| Option | Behavior |
| --- | --- |
| `--watch` | Default; debounce source changes by 150 ms |
| `--no-watch` | Execute once and keep the viewer open |
| `--watch-path <path>` | Also watch a runtime-read file/directory or dynamic import; repeatable |
| `--viewer <path>` | Use an installed binary or macOS `.app` bundle |
| `--headless` | Use the native compiler and simulated sink without GPUI for integration tests |

Each execution has a 10-second timeout and 64 MiB result limit. Syntax/runtime
errors and rejected native graphs appear in diagnostics while the previous graph
keeps playing. Unchanged valid content is skipped; rejected content can retry on
the next watched event. Stable IDs and a fixed seed help preserve selection and
deterministic music across rebuilds. Dynamic-library source changes require an
explicit rebuild of the library and another authoring run.

## Read-only controls

The integrated header replaces the separate system title bar while retaining the
native macOS close, minimize and fullscreen buttons. Drag the project/status area
to move the window; double-click follows your macOS title-bar preference. The Scopes
button stays independent of the drag area, and fullscreen removes the button gutter.

All panels follow the system's light or dark appearance, including changes while
the viewer is open. Notes, meters, diagnostics and control states use matching
palettes. Appearance changes are display-only and do not rebuild the music.

- The Playlist has pinned track headers and a bar ruler. Colored pattern clips show
  actual note thumbnails, including repeats and Track tempo overrides. It initially
  fits the project; **Fit** restores this view. Scroll in both axes, Shift-scroll
  horizontally, or ⌘/Ctrl-scroll to zoom. Both scrollbars can be dragged.
- Click a pattern clip to inspect its notes. The piano roll fits the phrase and keeps
  its keyboard, local beat ruler and velocity lane pinned. Scroll to reach all 128
  MIDI keys; Shift-scroll moves through time and ⌘/Ctrl-scroll zooms time. **Keys ±**
  changes row height. After clicking the grid, use arrows, Page Up/Down and Home/End
  to navigate, **±** to zoom, and **F** to fit. Both scrollbars support dragging.
- Play/Pause and Stop control the native transport. Click a ruler beat or marker to
  seek. The Go field accepts `bar.beat` (both start at 1), `mm:ss`, or seconds with
  an `s` suffix; press Enter. The transport display includes bar.beat.tick and time.
- Loop toggles the current range; **Loop clip** uses the selected clip's range.
  Space plays/pauses when the Go input is not being edited.
- Click a Channel or bus strip to select the waveform, Hann-512 spectrum and Mid/Side
  stereo scope. Level, pan, mute/solo, inserts and send routes are read-only values
  from source. Peak/RMS and note-gate highlights come from native playback telemetry.
- Master stays at the left of the Mixer. Scroll or use **‹/›** to browse strips;
  **Inserts** toggles the independently scrolling effect/routing inspector. At small
  panel heights, Alt-scroll or drag the vertical scrollbar to reach lower readouts.
  After clicking the Mixer, Left/Right and Home/End select and reveal a strip;
  Up/Down and Page Up/Down scroll vertically. Meter columns mean **peak and RMS**.
- Drag the horizontal divider above the editors to change their height, or the
  divider between the piano roll and Mixer to change their widths.
- The footer shows native load, estimated output/graph latency, xruns and dynamic
  plugin faults. Error diagnostics include code/path where available.

Meters and scopes refresh around 30 Hz. Fixed rings can drop analysis frames when
the UI falls behind; the strips report drops. Master scope true peak is a 4× estimate
of consumed output, not an export loudness report. Audio runs independently of the
UI and continues if the runner disconnects. Closing the viewer or explicitly
stopping the CLI stops the paired session. A locked macOS desktop can suppress
window refresh while the native engine and IPC continue working.

Changing sample rate or block size after playback starts requires restarting the
viewer. Graph swaps keep transport position/loop settings and rebuild voice state;
they do not promise seamless preservation of sustained voices. Faders/notes cannot
be dragged or edited in the viewer. Large-project virtualization, signed npm bundles
and physical-device endurance are tracked separately from the current preview.

## Developer visual smoke

Capture the real GPUI window without starting audio playback:

```sh
OXITONE_PREVIEW_CAPTURE="$PWD/target/preview-dark.png" \
OXITONE_PREVIEW_APPEARANCE=dark \
node packages/cli/dist/index.js preview examples/drum-machine/src/preview.ts \
  --no-watch --viewer target/release/oxitone-preview
```

Use `light` for the other palette. Optional `OXITONE_PREVIEW_CAPTURE_SIZE=1060x720`
checks the minimum logical window size. `OXITONE_PREVIEW_CAPTURE_NAVIGATION=1`
adds a keyboard-dispatch and scroll/drag-controller smoke using measured view bounds,
then captures the resulting selection and zoom. Use the drum example for this smoke.
The process exits after capture and the CLI cleans up its socket.

These options only apply when `OXITONE_PREVIEW_CAPTURE` is present. Appearance is
overridden on that window; system preferences stay intact. The opt-in redraw driver
also works when a locked macOS desktop suppresses display-link ticks. Screenshots
and controller checks do not replace physical mouse/trackpad, traffic-light,
fullscreen and live system-appearance testing on an unlocked desktop.
