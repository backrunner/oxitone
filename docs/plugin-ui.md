# Instrument and effect windows

Double-click an instrument's Mixer strip, or select its card under **Details → Chain**.
Click any Channel, bus or Master effect card to open it. Each slot gets an independent
window; reopening a slot focuses its existing window.

**Panel** shows a compact native instrument/effect face. Wavetable has Sound and
Envelopes pages with oscillators, filter, voice/output and ADSR diagrams. Every effect
keeps **Mix**, dry/wet percentages and bypass visible above the scrolling panel,
including custom layouts. Set the ratio in code:

```ts
import { createAutomationNamespace } from "oxitone";

channel.addEffect({
  pluginId: "oxitone.delay",
  pluginVersion: "1.0.0",
  parameters: { timeBeats: 0.75, feedback: 0.3 },
  mix: 0.25, // 25% wet, 75% dry; omitted mix defaults to 1
});
// Host mix automation is also supported:
channel.automate("insert.0.mix", createAutomationNamespace().constant(0.4));
```

Use your automation namespace when authoring lanes; the insert index identifies
the effect's position in its chain. Mix is supported on Channel, bus and Master inserts.

**Inspect** lists all parameters, source/default provenance and automation markers.
**Specs** expands IDs, ranges, mapping and smoothing. **Assets** and **Info** show
resources, structured state, descriptor and the verified dylib path/hash.
**Copy JSON** copies the source reference. Values are source configuration, not
effective automation or smoothing readback. ADSR drawings are source schematics
with an illustrative sustain hold, not a measured live envelope.

The controls remain read-only. Page selection, scrolling, resizing and playback
shortcuts work without changing the music. Each window follows system light/dark
appearance and uses a custom title area with native macOS traffic lights.

## Register a custom native layout

Built-ins and dynamic libraries use the same public API, separate from DSP registration:

```ts
import type { PluginUiManifest } from "oxitone";

const echoPanel: PluginUiManifest = {
  uiVersion: "1.0",
  pluginId: "oxitone.delay",
  pluginVersion: "1.0.0",
  title: "Echo",
  size: { width: 520, height: 320 },
  pages: [
    {
      id: "main",
      title: "Delay",
      groups: [
        {
          id: "echo",
          title: "Echo",
          columns: 2,
          controls: [
            { kind: "knob", parameter: "timeBeats", label: "Time" },
            { kind: "knob", parameter: "feedback", label: "Feedback" },
          ],
        },
      ],
    },
  ],
};
project.registerPluginUi(echoPanel);
```

The panel binds an exact plugin ID/version. All parameter IDs refer to the plugin
descriptor; host Mix/bypass are always supplied separately by the viewer. Re-registering
the same identity replaces its layout. This metadata is not saved in the musical
snapshot or portable project: register it in the executable Preview entry.

Supported controls are **knob**, horizontal **fader**, **toggle** (0/1 enum),
**readout**, **choice** (enum options with numeric value and label), and **envelope**
(attack/decay/sustain/release parameter IDs). Pages and groups have stable IDs.
Groups wrap across the window, with 1–6 columns inside each group. Labels and group
titles are customizable; colors follow the host's accessible semantic theme.

The host limits each panel to 8 pages, 16 groups per page, 32 controls per group and
256 controls overall. Layouts are at most 256 KiB, with at most 64 registrations /
2 MiB total. Window sizes range from 440×280 to 1200×900 logical points. Labels
are at most 64 UTF-16 units, IDs 128, with no control characters. The versioned
[JSON schema](../schemas/plugin-ui.schema.json) supports editor validation; the
viewer additionally validates descriptor bindings, unique IDs and total budgets.

See the real drum and gain dylib panels in
[plugin-panels.ts](../examples/drum-machine/src/plugin-panels.ts). They require no
additional DSP instance, UI library or JavaScript running inside GPUI.

## Watch and incomplete code

Keep layouts in a local static TS/JSON import for automatic watching. For external
npm packages or runtime-read files, add an explicit `--watch-path`.

A layout-only change updates existing windows while reusing the audio graph and
its instruments/effects. Stable page IDs preserve selection. A changed declared size
resizes a non-fullscreen window; unchanged sizes preserve manual resizing.
Successful musical changes compile and publish their parameters, Mix and layouts
together, preserving transport with the existing graph-swap semantics.

While code is building or contains syntax/runtime/native compilation errors, windows
show **Building / Last good** and retain their accepted data; playback continues on
the last valid graph. Execution is bounded by the runner's 10-second timeout.
A malformed layout produces a local **PluginUiInvalid** diagnostic, retains the last
compatible layout (or uses the default panel) and allows valid music to advance.
Recovery updates all windows; deleting a registration restores the default panel.

Effects are addressed by owner and slot index. Reordering follows the slot, removal
shows an unattached state, and returning the slot restores its view. Escape/⌘W closes
only that window; closing the main window ends the session.

## Remaining extensions

This implementation renders declarative layouts directly in native GPUI. Arbitrary
AppKit/Metal views from a UI companion dylib, bitmap assets, custom shaders and
effective/live parameter telemetry are separate future extensions. It does not host
VST plugin formats. The audio ABI v1 has no UI entry point.

[The protocol and lifecycle specification](../.agents/docs/11-plugin-ui.md) tracks
those extensions. Run `node scripts/smoke-preview-panels.mjs` after building the
viewer, workspace and drum dylibs to exercise real multiwindow syntax/runtime/native
failures, invalid-layout fallback and recovery.
