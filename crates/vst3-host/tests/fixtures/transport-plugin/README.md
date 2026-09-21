# Native VST3 conformance fixtures

Derived from the MIT-licensed `vst3` 0.3.0 gain example. `LICENSE-MIT` is retained.
This crate is a test-only VST3 bundle; no audio device or proprietary plugin resources are needed.

| Class ID suffix | Class | Behavior |
| --- | --- | --- |
| F318797D | Transport | Encodes host clocks and processed frame count into stereo PCM |
| F318797E | Sidechain | Stereo main multiplied by an initially inactive mono detector |
| F318797F | Multibus | Three indexed mono/stereo buses with an inactive middle slot |
| F3187980 | Instrument | Zero audio inputs, MIDI notes, three output buses at gains 1/2/3 |
| F3187983 | Recording Gain | Stereo input multiplied by parameter 0 at actual sample offsets |

The prefix is `6E33225254224A00AA69301A`. These five processors share the original controller.
The instrument responds at each event's sample offset and clears voices/phase on processing reset.
The mono auxiliary output defaults inactive, so the Project test exercises explicit activation.

Run `node scripts/smoke-vst3-transport.mjs` and `node scripts/smoke-vst3-buses.mjs` from the
workspace after `pnpm build`. The latter checks native bus packets, Project sidechain, instrument
routing, fader/mute, stems, save/reopen PCM, simulated playback and graph replacement.
Set `OXITONE_VST3_VIEWER` to the built preview executable for an additional native routing capture.
Neither script opens system audio outputs.

The shared controller's non-automatable parameter 99 triggers real `IComponentHandler`
callbacks for `smoke-vst3-edits.mjs`. Values 1/8 through 7/8 produce a complete gesture,
begin only, end only, 4097 values (queue overflow), unknown parameter, NaN value, and a
non-automatable parameter respectively. This test-only controller does not create a GUI.
Value 1 generates 1024 ordered values to check multi-page reads without loss.
The smoke checks native packet timestamps and the actual SDK/N-API simulated engine path.
Recording Gain independently observes parameter delivery in PCM, including same-block changes,
Touch release, Write, paused gestures, cancellation, existing automation restoration and seek.
The SDK recording is converted into Source, compared with an independently authored step curve
through complete WAV output, then saved/reopened with the same exact WAV. No GUI input is simulated.

## Dynamic configuration fixtures

The following classes combine their processor and controller in one component.

The `F3187981` class changes its mono/stereo bus count, latency and parameter table after
Mode changes or opaque state restoration. It rejects nonzero sample processing and requires
inactive bus negotiation. Read-only parameters expose processor gain, latency and deactivation
count. Expanded gain 0.75 requests a bulk controller-value refresh to 0.625. Modes 2 and 3 request
component replacement and endless latency restarts, which the host must reject within its budget.

The `F3187982` class exposes 4096 writable parameters. Its opaque state records the last
parameter's processor value, independently from the controller. Restoring that state with an
empty parameter map proves that a full preset plus overrides reached the processor without
queue truncation. Run `node scripts/smoke-vst3-configuration.mjs --benchmark` for these cases.

`vst3-transport-probe` also captures state between audio packets through stream 8/control 1.
The processor accepts zero-sample parameter flushes and its monotonically increasing PCM counter
proves that capture does not reset processing or advance musical time. No audio device is opened.

`F3187984` adds live processing to the dynamic fixture. Controller changes, queued automation,
and an independent process-time notification at project frame 96001 exercise frozen state,
silent old-layout replies, recording invalidation and replacement helper restoration.
`F3187985` changes its main buses from stereo to mono, adds parameter 7 and physically delays
PCM by 64 frames. Two independent configured instances verify graph-local parameter tables,
PDC, source capture/Undo/Redo/Save and exact saved replay. Both run through
`node scripts/smoke-vst3-restart.mjs`, with simulated/offline outputs only.
