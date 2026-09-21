# Local VST3 processing extensions

Source: the crates.io `vst3-host` 0.9.0 source distribution, MIT licensed;
upstream is https://github.com/HelgeSverre/rust-vst3-host/ . The upstream
LICENSE is retained. No compiled binaries are included.

Oxitone uses the loader only inside its own isolated helper, with all default
features disabled. The local extension exposes an explicit, validated process
transport position; upstream 0.9.0 only exposes tempo, signature and playing.
This is necessary for project positions, variable tempo and loop context. It also
exposes `reset_processing`: the standard `setProcessing(false/true)` transition,
without deactivation, setup, state serialization or module reload. The helper
uses it before the first post-seek/loop packet, retaining parameter values.

Local changes are confined to `src/process_position.rs`, `src/lib.rs`, the public and
internal transport/reset methods in `src/plugin.rs`, and their implementations
in `src/internal/plugin_impl.rs`. Keep these changes when updating the
dependency. Unsupported upstream isolation implementations reject this API.
The local ComponentHandler also exposes a cumulative saturating editor-feedback overflow
counter through Plugin::parameter_edit_overflow_count. Gesture and value-queue losses are
observable without clearing the counter, so Oxitone cannot accept incomplete automation.
Other isolation implementations return None rather than claiming lossless capture. This
extension additionally changes src/internal/com_implementations.rs; existing queue caps stay.
The upstream file layout and formatting are retained as a third-party source
exception to the repository module size guideline.

OutputEventConsumer exposes a cumulative failure counter. HostEventList counts raw output
rejections (invalid event, full event/payload budget, poisoned list); the owned output queue
rejects and counts new events when full instead of silently evicting old note-offs. The helper
checks this counter once per processed block before accepting MIDI output. The minimal hooks
stay in the existing upstream files; MIDI conversion, graph routing and protocol live in Oxitone.

`declared_audio_bus_layout` queries bounded component bus declarations on the control thread
without renegotiating or replacing the old processing buffers. Frozen restart capture uses it
because `audio_bus_layout` describes the prepared buffers and is intentionally stale after an
I/O notification. Its activation bits are the component's declared defaults; a new graph chooses
its own activation during preparation. Other backends explicitly reject this inspection API.
