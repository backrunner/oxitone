# Code-driven project preview

Oxitone Preview is a native GPUI viewer. `oxitone preview` provides transport and display
controls. `oxitone daw song.ts` opens the editable mode: notes, clip placement,
mixer settings, tempo, plugin instances and source automation are written to TypeScript by the Node
Document Service. Save persists the owned project sources through its journal.
Closing a modified DAW project offers **Save & close**, **Close without saving**, and
**Cancel**. A failed save keeps the window and draft open. Save & close waits for the
matching saved revision; newer edits cancel automatic closing.

The [VS Code extension](../packages/editor-vscode/README.md) connects linked TypeScript
tabs to the same service, including unsaved edits, conflict review and journal Save.
Install `target/oxitone-vscode.vsix` after building it with
`pnpm --filter oxitone-vscode package`, then run **Oxitone: Open Project in DAW**.
Already running sessions print a document socket for **Oxitone: Connect to DAW Session**.
An editor launcher can reserve a private directory and supply `--document-socket <path>`.

## Start from this checkout

```sh
pnpm install --frozen-lockfile
pnpm build
pnpm build:preview
pnpm install:cli
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

`pnpm install:cli` links this checkout's built CLI to `~/.local/bin/oxitone`. Put
`~/.local/bin` on your shell PATH if it is not there already. The command then works
from any directory; rebuilding this checkout updates it. Existing installations at
that path are not overwritten. Use `node scripts/install-cli.mjs <bin-directory>`
for a different directory. The checkout and installed workspace dependencies must remain.

For the full melodic dubstep project, from the repository root:

```sh
pnpm example:songs:prepare
oxitone preview examples/drum-machine/src/full/melodic-dubstep.ts
```

The default watch mode follows imported arrangement, patch and mix files. Preview
opens stopped; press Space or Play to listen. Absolute entry paths work outside the checkout.

## Single-file project builds

```sh
oxitone build examples/drum-machine/src/full/melodic-dubstep.ts -o target/examples/after-the-horizon.mjs
oxitone preview target/examples/after-the-horizon.mjs
# To rebuild the file on edits, run this in a separate terminal:
oxitone build examples/drum-machine/src/full/melodic-dubstep.ts -o target/examples/after-the-horizon.mjs --watch
```

Both `build` and `preview` use esbuild to combine local imports/exports, JSON and
statically resolvable dynamic imports into **one ESM JavaScript file**, including
its source map. Preview executes that file in a fresh process on each rebuild.
Syntax failures leave the previous artifact and accepted preview intact. Native
validation and runtime errors keep the previous accepted graph in Preview.

This is a local code artifact: installed npm/SDK/native packages, samples and dylibs
remain external, with their existing source-relative locations preserved. Nonliteral
runtime imports/file reads still require those resources and explicit `--watch-path`
when watching them. Use the portable project format for relocating sample assets.

The drum-machine project uses real dynamic instrument and effect libraries:

```sh
pnpm example:drums
pnpm preview examples/drum-machine/src/preview.ts
```

To explore bus sends and sidechains, use the routed version of the same song:

```sh
pnpm preview examples/drum-machine/src/mixer-preview.ts
```

It routes instruments through Drum/Music buses, adds Room reverb and Tape echo returns,
and includes pre/post-fader sends, an automated send and a drum sidechain into the
Music bus compressor. The original song/export entry stays available separately.

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
again on every change. Each source module's `import.meta.url`, `import.meta.dirname`
and `import.meta.filename` remain anchored to the source file after bundling;
npm packages are resolved in the author's package scope and kept external.
`__oxitoneSourceDirectory` is a reserved bundle export used for default asset lookup.
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
the next watched event. Source associations preserve selection without adding IDs
to your code; a fixed seed keeps generated music deterministic. Dynamic-library source changes require an
explicit rebuild of the library and another authoring run.

## Workspace controls

The integrated header replaces the separate system title bar while retaining the
native macOS close, minimize and fullscreen buttons. Drag the project title area
to move the window; double-click follows your macOS title-bar preference. Editor
navigation stays independent of the drag area, and fullscreen removes the button gutter.

All panels follow the system's light or dark appearance, including changes while
the viewer is open. Notes, meters, diagnostics and control states use matching
palettes. Appearance changes are display-only and do not rebuild the music.
Plugin detail panels show **Initial settings**. In DAW mode, **Edit configuration**
opens a separate settings window for that instance. Its title identifies the owner,
plugin and insert slot. These inputs update TypeScript; the detail-panel knobs do not yet
provide continuous parameter audition.

Plugins is a compact name/type list with search and All/Instruments/Effects filters.
Select a row with a click or the arrow keys; ⌘/Ctrl+F searches. **Used in project**
opens instance shortcuts, and **Details** reveals package information and maintenance
actions. Enter toggles usage shortcuts and Escape closes the drawer. Browsing the
library does not change an open settings editor. Add package remains in the toolbar;
the library does not show parameter tables. Use **Add effect** or **Replace plugin**
in Mixer Chain to open a picker for that slot. Select a compatible plugin and press
Enter or Add/Replace. Built-in and verified npm plugins use the same workflow;
registration is saved in the current project, and musical edits leave npm package files intact.

- The Playlist has pinned Track headers and a bar ruler. A Track is an arrangement
  container; it can overlap multiple pattern, sample and automation clips. Channels
  own instrument and Mixer routing. Colored pattern clips show
  actual note thumbnails, including repeats and Track tempo overrides. It initially
  fits the project; **Fit** restores this view. Scroll in both axes, Shift-scroll
  horizontally, or ⌘/Ctrl-scroll to zoom. Both scrollbars can be dragged.
  Sample clips and channel-targeted automation are placed directly on the same
  Track lane. Each Track header has **M**ute and **S**olo controls below its name.
  These affect only that Track's placements, even when Tracks share a Channel.
  Muting keeps the timeline length unchanged. **Patterns** opens a compact browser. Drag a Pattern or
  Sample from the browser into any Track; the drop is saved through one Document
  Service transaction. Pattern placement details are secondary to the drag workflow.
- In DAW mode, drag a clip to move it or Shift-drag to copy it. With the Playlist
  focused, ⌘/Ctrl+D duplicates at the clip end, Delete removes it and M toggles it.
  Drag the right edge to change a Pattern/Automation duration or fit a Sample's
  playback window using its existing AUDIO/STRETCH/REPITCH mode. Click a Track's
  colored header strip to enable/disable it. Clips on Tracks with local tempo
  overrides currently require code edits for moving, copying and resizing.
  The clip and its contents follow the pointer directly, including cross-track moves,
  copies, browser insertion and resizing. The final position stays visible while code accepts the edit.
- Click a pattern clip to inspect its notes. The piano roll fits the phrase and keeps
  its keyboard, local beat ruler and velocity lane pinned. Scroll to reach all 128
  MIDI keys; Shift-scroll moves through time and ⌘/Ctrl-scroll zooms time. The key-height
  icons or ⌘/Ctrl-Shift-scroll change row height. Middle-drag pans the grid.
  After clicking the grid, use arrows, Page Up/Down and Home/End
  to navigate, **±** to zoom, and **F** to fit. Both scrollbars support dragging.
- Click anywhere in the Playlist ruler/clips/empty lanes or piano ruler/note/velocity
  area to locate precisely, without snapping to a tick. Playback continues if running.
  Double-click or Option/Alt-click there (or a marker) to play from that position.
  Piano positions follow the current clip repetition and Track tempo, clamped to a
  truncated clip's end; outside the clip they use its first repetition.
- Clicking the timeline, piano ruler or a marker sets the **cue**, marked at the top of the Playlist lanes.
  Double-click or Option/Alt-click plays from that position. Enter replays from the cue and Stop returns to it.
- Loop toggles the current range; **Loop clip** uses the selected clip's range.
  Locating outside an enabled loop turns it off. Enabling a loop while outside it
  moves to the loop start. The **?** button in the transport bar opens the shortcut guide.
- Click a Channel or bus strip to select the waveform, Hann-512 spectrum and Mid/Side
  stereo scope. In DAW mode, drag level/pan, hold Shift for fine adjustment, or
  double-click to reset. M/S toggles mute/solo. Release commits one undoable source
  edit; Escape cancels. Routing remains a source readout. Peak/RMS and note-gate
  highlights come from native playback telemetry.
- Click BPM to edit a static project tempo; Enter commits and Escape cancels.
  Projects with tempo automation or a multisegment tempo map retain their existing
  tempo definition. The input consumes typing without starting playback.
- Master stays at the left of the Mixer. Scroll or use **‹/›** to browse strips;
  **Details** toggles the independently scrolling inspector. **Chain** shows instrument
  and compact effect rows with open/replace/remove controls; **Routing** shows sends first, then output and inputs.
  Send cards show source percentage/dB, pre/post-fader, detector-only sidechain and
  automation status. A Channel's downstream sends are labeled **Sends via [bus]**;
  the sends belong to that bus. Click a route to select/reveal its other endpoint.
  Related strips show IN/OUT and each strip's footer identifies its output/send count.
  The maximize icon or **F9** gives Mixer the full workspace; restoring returns to the dock.
  Spare bank space shows a compact signal-flow map (first three connections, with the
  complete list in Routing). At small panel heights, Alt-scroll or drag the vertical
  scrollbar to reach lower readouts.
  After clicking the Mixer, Left/Right and Home/End select and reveal a strip;
  Up/Down and Page Up/Down scroll vertically. After clicking Details, navigation keys
  scroll that panel independently. Meter columns mean **peak and RMS**.
- Drag the horizontal divider above the editors to change their height, or the
  divider between the piano roll and Mixer to change their widths.
- Double-click an instrument strip or click its instrument name under **Chain**;
  click an effect slot to open that instrument/effect in an independent, live-updating
  detail window. See [plugin windows and custom layouts](plugin-ui.md).
- The footer shows native load, estimated output/graph latency, xruns and dynamic
  plugin faults. Error diagnostics include code/path where available.

| Shortcut | Action |
| --- | --- |
| Space | Play / pause |
| Enter | Replay from cue |
| Shift+Space | Stop and return to cue |
| Option/Alt+Left / Right | Move one beat backward / forward |
| Option/Alt+Shift+Left / Right | Move by one bar using the current time signature |
| Command/Ctrl+Home / End | Move to project start / end |
| `[` / `]` | Previous / next marker |
| L | Toggle loop |
| ? | Show shortcut guide; Escape closes it |

Playback, cue, beat/bar, marker and loop shortcuts also work in plugin windows.
Held toggle keys do not repeatedly start/stop playback; navigation keys can repeat. Plain arrow/Home/End
keys retain their piano, Mixer and detail-panel scrolling behavior.

Meters and scopes refresh around 30 Hz. Fixed rings can drop analysis frames when
the UI falls behind; the strips report drops. Master scope true peak is a 4× estimate
of consumed output, not an export loudness report. Audio runs independently of the
UI and continues if the runner disconnects. Closing the viewer or explicitly
stopping the CLI stops the paired session. A locked macOS desktop can suppress
window refresh while the native engine and IPC continue working.

Changing sample rate or block size after playback starts requires restarting the
viewer. Musical graph swaps keep transport position/loop settings and rebuild voice state; UI-only layout updates reuse audio instances;
they do not promise seamless preservation of sustained voices. Mixer meters remain
display-only; DAW piano/source edits use the bounded document transaction path. Large-project virtualization, signed npm bundles
and physical-device endurance are tracked separately from the current preview.

## Developer visual smoke

`node scripts/smoke-ui.mjs` checks thirteen native-window scenarios across dark/light
appearance and standard/minimum sizes, including editing, internal windows, plugin
configuration, plugin assignment, mixer controls, library navigation and failed-save recovery. Captures, per-scenario logs and the current
run status are in `target/ui-review/`. Every scenario uses a disposable project and a
simulated audio sink, then reopens the saved TypeScript. It does not certify physical
input devices, IME, VoiceOver or native fullscreen behavior.

The DAW workspace offers Arrange, Piano roll and Mixer views. Use F5/F7/F9 or
the maximize icon to give an editor the whole workspace. Arrange starts with the
timeline above a full-width piano dock. Its dock buttons switch between piano,
mixer and both; clicking the active dock button collapses it. Restore keeps the dock
configuration and sizes. Drag the dividers to resize the arrangement, piano/mixer
split, mixer details, velocity lane and analysis area. Analysis starts collapsed;
the waveform icon in the status bar reveals it. Tool names and shortcuts appear on hover.

In `oxitone daw`, P selects Draw, B selects Paint and E selects Select. Click to
insert and drag to place the note before release; Shift-draw stretches its duration.
Paint fills a continuous passage, including fast vertical and diagonal strokes.
Command/Ctrl-click toggles notes, Command/Ctrl-drag selects a box, Shift-drag copies,
and right-drag erases. Command/Ctrl-A selects all, D with Command/Ctrl duplicates,
Q quantizes starts and Delete removes the selection. Shift-arrows change octave or
  duration. The magnet button toggles the selectable snap grid; Alt while inserting or
  dragging temporarily releases it for 1/960-beat placement. The grid remains visible
  while the magnet is off.
Middle-drag or Command/Ctrl-Alt-drag pans the piano. Command/Ctrl-scroll zooms time;
adding Shift zooms key height around the pointer. Escape
cancels the current gesture. An entire gesture has one undo entry. The grid keeps
extending as you scroll. Drawing, moving, resizing or duplicating beyond the phrase
extends its source length in the same edit, rounded up to a whole beat. Explicit
Playlist clip durations still keep their chosen boundaries. F fits the phrase;
End scrolls to its end. Source updates preserve the current zoom and scroll.
Notes and their velocity handles move directly during a drag; release commits one edit,
and the final notes remain visible until the accepted source replaces them.
Click or sweep across the velocity lane to draw absolute levels from 0 to 127.
A selection limits the stroke to those notes; without a selection it affects every
crossed note, including chords. Softer notes have lower opacity. Right click in the
velocity lane leaves notes intact. Internal floating windows share rounded corners.

Automation offers Points, Draw and Line tools with Linear, Smooth and Step
interpolation. Drag a point, Alt-drag a segment to bend it, or right-click an interior
point to remove it. Command/Ctrl-scroll zooms around the pointer; ordinary scroll
pans. Edits change the source range, including its repeated occurrences in a loop.

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

With `mixer-preview.ts`, `OXITONE_PREVIEW_CAPTURE_MIXER=split|expanded|chain` checks
send values/automation, follows an outgoing send and returns via an incoming route,
and dispatches Home/End to verify independent inspector scrolling before capture.
Use it separately from the navigation/plugin capture modes.

For instrument/effect windows, set `OXITONE_PREVIEW_CAPTURE_PLUGIN` to `instrument`,
`synth` (first Wavetable), `effect`, or `info` (effect metadata). This also checks
window reuse, close/reopen, detail keyboard scrolling and the main-window close
path while details remain open. Use the drum example, which has both kinds of plugin.
`OXITONE_PREVIEW_CAPTURE_REVISION=2` waits for an accepted watch update before capture.

After building the workspace, drum example and viewer, run the actual multiwindow
watch smoke with `node scripts/smoke-preview-details.mjs` (optionally pass a viewer
path). It changes a private temporary entry after the windows open, verifies the new
parameter value and revision in both windows, and writes `target/plugin-details-watch.png`.
It removes the temporary entry and leaves the example source intact.

`OXITONE_PREVIEW_CAPTURE_TRANSPORT=1` runs separately from the other smoke modes.
With the drum example it checks precise pointer controllers using measured bounds,
real GPUI keyboard dispatch, cue/loop behavior and plugin-window
transport shortcuts. It uses the real Rust engine with a simulated sink and never
opens an audio device; ordinary captures do not start playback. This checks control
semantics, not physical mouse hit testing or device/xrun endurance.

These options only apply when `OXITONE_PREVIEW_CAPTURE` is present. Appearance is
overridden on that window; system preferences stay intact. The opt-in redraw driver
also works when a locked macOS desktop suppresses display-link ticks. Screenshots
and controller checks do not replace physical mouse/trackpad, traffic-light,
fullscreen and live system-appearance testing on an unlocked desktop.

`OXITONE_PREVIEW_CAPTURE_EDITING=1 node scripts/smoke-daw.mjs` additionally checks
batch selection/move/duplicate/resize/Undo and cancellation, then sends targeted
native mouse events through GPUI hit testing to resize the actual dividers, switch
full-workspace editors and restore the split. Combine with
`OXITONE_PREVIEW_CAPTURE_AUTOMATION=1` for curve tension/Undo checks. The fixture
uses a temporary 8-beat pattern and a simulated sink; no system audio device opens.
`OXITONE_PREVIEW_CAPTURE_WORKSPACE=piano|mixer|split` chooses the final screenshot.
With `OXITONE_PREVIEW_CAPTURE_AUTOMATION=1`, the Automation editor is restored after the
workspace gesture checks so the final capture shows the edited curve.
