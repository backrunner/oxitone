# Electronic effects

The engine includes 26 stereo effects. All inserts support `mix` (0..1, default 1),
`bypass`, parameter automation, offline rendering, Wasm and native Preview details.
Rust owns the DSP; code changes update the declarative project through the existing
watch workflow. Rejected builds retain the last valid graph.

```ts
import { Project, effect, createAutomationNamespace } from "oxitone";

const project = new Project({ bpm: 140 });
const chords = project.addMixerChannel({ name: "Chords", inserts: [
  effect("nonlinearFilter", { cutoffHz: 6500, driveDb: 6, resonance: 0.2 }),
  effect("multibandDynamics", { depth: 0.25, upwardDb: 9 }, { mix: 0.6 }),
  effect("spreader", { bassMonoHz: 220, amount: 0.3 }),
] });
const automation = createAutomationNamespace();
chords.automate("insert.0.parameter.cutoffHz",
  automation.sine({ periodBeats: 1, min: 0.35, max: 0.85 }));
project.master.addEffect(effect("limiter", { ceilingDb: -1, releaseMs: 120 }));
```

Helper parameters use physical values (Hz, dB, milliseconds, semitones). Automation
uses normalized values mapped through the descriptor. A filter sweep changes
brightness; use gain envelopes or a sidechain compressor when you also want volume
ducking. Keep the sub on a separate centered path when widening chords or midbass.

| Helper kind | Controls / use |
| --- | --- |
| `nonlinearFilter` | Driven LP/HP/BP/notch, cutoff, resonance; 2x processing |
| `compactor` | Bounded upward compression, transient emphasis/suppression |
| `multibandDynamics` | Three LR4 bands, upward/downward dynamics, timing and trims |
| `resonator` | Four tuned modes, decay, brightness, inharmonicity, stereo |
| `frequencyShifter` | Signed additive Hz shift and stereo offset |
| `pitchShifter` | ±24 semitones, ±100 cents; fixed duration |
| `flanger` | Fractional delay sweep, signed feedback, stereo phase |
| `convolver` | Stereo impulse response, predelay, wet high/low cuts |
| `bitcrush` | Quantization, sample-and-hold rate, deterministic clock jitter |
| `tape` | 2x saturation, bandwidth, wow/flutter |
| `spreader` | High-band decorrelation, M/S width, bass centering |
| `limiter` | Stereo-linked 4x lookahead, input gain, ceiling, release |
| `compressor` | Peak/RMS detector, knee, makeup, detector highpass, sidechain |
| `gate` | Attack/hold/release, hysteresis, attenuation range, sidechain |
| `delay` | Beat or seconds time, filtered feedback, ping-pong, wet ducking |
| `reverb` | Decay, damping, predelay, wet filtering, ducking and width |

The existing EQ, Filter, Limit, Clipper, Phaser, Chorus, Saturator, Utility,
Distortion and broad-band Multiband remain available through their `EffectRef`
plugin IDs. `oxitone.multiband-dynamics` uses steeper crossovers than the older
`oxitone.multiband`; neither is a third-party product compatibility mode.

External impulses use project sample resources:

```ts
import { convolver, importSample } from "oxitone";

const impulse = await importSample("./assets/room.wav");
const sample = project.addSample(impulse);
const room = project.addMixerChannel({
  name: "Room", inserts: [convolver(sample.id, { highpassHz: 250, lowpassHz: 8000 })],
});
chords.send(room, { ratio: 0.12 });
```

IRs are decoded, hash-checked, resampled and partitioned before processing. Mono
IRs apply to both channels; stereo IRs process L/R independently. There is no
automatic IR normalization. The limit is 262144 resampled frames (5.46s at 48kHz),
with 256 frames of reported latency. IR changes rebuild the graph. Browser hosts
upload the asset bytes through `WasmEngine.importSample` before compile, as for
sampler resources. `effect("convolver")` uses an included synthesized short room.

Delay pitch movement and grain-based pitch shifting have deliberate coloration;
pitch shifting does not preserve vocal formants. Frequency-shifter quadrature is
less accurate near DC/Nyquist, and large shifts can fold edge frequencies. Tape is
an original coloration model. The mastering limiter includes a 0.9 linear reserve
for reconstruction; verify exported true peaks when setting delivery loudness.
`mix < 1` or bypass on a limiter blends unrestricted dry audio into the master.

Preparation budgets and algorithms are specified in
[the effects specification](../.agents/docs/14-effects-production.md). Silent DSP
behavior and parity checks verify implementation; release quality still requires
musical balance, sound design and evaluation of the final arrangement/master.
