# Instrument and effect windows

Double-click an instrument's Mixer strip, or select it and click its instrument card
under **Details → Chain**. Click any effect card there to open that effect. This includes
channel effects and bus/Master inserts. Each instance/slot gets its own window;
opening the same item again brings its existing window forward.

The windows show all declared parameters, their source values or plugin defaults,
units and ranges, mapping/smoothing/rate, automation bindings, and host mix/bypass.
Parameters use two-column readout cards; **Specs** reveals parameter IDs, mapping,
smoothing/rate and full automation bindings. Use **All**, **In source** or **Automated**
to filter, scroll or use navigation keys,
and switch to **Resources / State** or **Plugin info** for samples, structured state,
channel context and plugin capabilities. **Copy JSON** copies the selected source
instrument/effect reference. These are viewing controls; they never edit the music.

Values are initial configuration, not a live readback of automation or smoothing.
Dynamic plugins use the descriptor validated by the Rust loader. Their info tab
also shows the registered library path and verified SHA-256; viewing details does
not load another library or instantiate another instrument/effect.

Successful watch builds update open windows. Failed builds keep the previous valid
details. Effects currently have slot indices rather than instance IDs: reordering
effects changes what that slot's window displays. Removing the slot shows an empty
state; returning it restores the view. Escape/⌘W or the close button closes just
that detail window. Closing the main project window ends the session and all details.
Each window uses the integrated title area and follows system light/dark appearance.
Space plays/pauses the project, Enter replays from the cue, and Shift+Space stops at
the cue. Beat/bar, marker, project-boundary and loop shortcuts are shared with the
main window; unmodified navigation keys still scroll details. See [shortcuts](preview.md).

## Custom UI for dylib plugins: planned extension

Custom plugin-provided UI is **not implemented yet**. The current audio ABI v1 has
no UI entry point; adding unknown UI fields to today's manifest does not enable one.
The proposed implementation order is:

1. **Declarative UI package**: a versioned local JSON layout, parameter-ID bindings,
   theme tokens and hashed assets, rendered by GPUI. Plugins can arrange their own
   panels while the host enforces read-only behavior and provides fallback details.
2. **Optional native companion**: a separate UI dylib and independently versioned C
   entry that attaches an AppKit view to a host-owned window. It gets an independent
   UI context and versioned snapshots, never a DSP instance pointer or audio callback.
3. **Opt-in live feedback**: bounded Rust telemetry for effective parameters and
   plugin-specific visualizations, with generation/frame ordering and drop counts.

UI registration remains separate from musical snapshots and DSP manifests. An
unsupported/failed UI falls back to the generic window. Native code in the same
process is trusted code; crash isolation would require a separate helper process.

The [design and acceptance criteria](../.agents/docs/11-plugin-ui.md) cover contracts,
lifecycle, parameter provenance, readonly enforcement, watch, DPI/theme support,
compatibility, packaging and realtime benchmarks. All proposed UI API names remain
draft until the corresponding TS/Rust/schema and conformance implementation lands.
