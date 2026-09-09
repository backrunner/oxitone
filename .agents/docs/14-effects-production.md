# Electronic effects and mastering

The processing palette is complete; the electronic demo now uses selected processors
for separate timbre, bus, spatial-return and mastering roles (see 13-electronic-production.md).
All effects use the existing versioned EffectRef, physical parameter values,
normalized automation, host mix/bypass and fixed prepare-time PDC. TypeScript
`effect(kind, parameters, {mix, bypass})` validates the new parameter tables;
`convolver(sampleId, parameters, options)` binds a project impulse SampleRef.

New IDs are `oxitone.nonlinear-filter`, `compactor`, `multiband-dynamics`, `resonator`,
`frequency-shifter`, `pitch-shifter`, `flanger`, `convolver`, `bitcrush`, `tape`,
`spreader`, `limiter` (all with the `oxitone.` prefix, version 1.0.0).
Ranges are defined in `packages/protocol/src/effects.ts` and Rust descriptors;
cross-boundary tests must keep these in agreement. Continuous controls are
smoothed within DSP, with bounded storage allocated before processing.

- Nonlinear filter: driven 2x state-variable filter, low/high/band/notch modes;
  feedback saturation and resonance are bounded. Constant 24-frame latency.
- Compactor: bounded upward compression with a silence floor and transient
  emphasis/suppression; it is a density processor, not a named commercial clone.
- Multiband Dynamics: stereo-linked three-band upward/downward dynamics with steep LR4 splits,
  per-band trims and a global timing scale. Depth zero uses the phase-matched
  crossover sum. This is an original algorithm, not preset or code compatibility.
  Public names, IDs, UI and presets use descriptive Oxitone names; third-party
  product names are not used as aliases or compatibility labels.
- Resonator: four tuned damped modes with harmonic/inharmonic spacing and stereo spread.
- Frequency shifter: analytic-signal quadrature modulation, signed Hz shift;
  frequencies shift additively, with a 257-tap windowed Hilbert FIR and 128-frame
  latency. Quadrature accuracy degrades near DC/Nyquist; extreme shifts can fold
  content at the band edges. Pitch shifter uses overlapping delay grains and
  semitone/cents ratios; it preserves duration, has modulation coloration and
  does not claim formant-preserving vocal or phase-vocoder quality.
  Its 50ms window reports half-window + 8 frames as nominal latency (1208 at
  48kHz); actual read positions vary during shifting. Tape reports 5ms + 24 frames
  (264 at 48kHz); speed modulation also moves the instantaneous read position.
- Flanger: fractional delay sweep, signed feedback and stereo phase offset.
- Convolver: partitioned FFT convolution, stereo or mono IR, 256-frame latency;
  at most 262144 resampled IR frames, rejecting invalid/empty/oversized impulses
  before DSP. No resource selects a deterministic built-in short room. IR changes
  rebuild the graph; arbitrary impulse arrays/files are never loaded in process.
- Bitcrush: sample-and-hold rate reduction, quantization and deterministic jitter.
  Aliasing/quantization are intentional creative processing, distinct from export dither.
- Tape: oversampled saturation, bandwidth, deterministic wow/flutter; no generated
  hiss, and no claim to measured hardware calibration.
- Spreader: high-band allpass decorrelation, M/S width and protected centered bass.
- Limiter: stereo-linked 4x peak processing, fixed 5ms lookahead, bounded peak-hold
  gain recovery and reconstruction margin. Verify reconstructed true peaks with
  the offline meter; oversampling is an estimate, not an absolute analog guarantee.
  Latency is 5ms + 36 frames (276 at 48kHz). A fixed 0.9 linear reconstruction
  reserve sits below the selected ceiling. Existing `oxitone.limit` also uses a
  sliding peak window and corrected release recovery; its 5ms + 24 latency stays.

Compressor/Gate must use their own input when no sidechain route exists. A routed
silent detector remains a real external sidechain. Gate adds hysteresis and range;
compressor adds detector highpass. Reverb and delay retain existing IDs/defaults,
with reverb wet highpass/lowpass, input-driven ducking and stereo width. Filter
cutoff redesign now uses a persistent 32-frame cycle, independent of host blocks.
All 26 effects have dedicated native panels with parameter-derived response diagrams;
DAW controls submit instance-scoped source transactions. Source/watch, host mix/bypass,
custom panel overrides and system appearance follow 11-plugin-ui.md. Diagrams do not
claim measured signal or effective parameter telemetry.
Tests cover transfer/frequency/time
behavior, tail exhaustion, reset, block independence, PDC and zero allocations.
Native and Wasm use the same DSP. Automated tests never open output devices.

Verification archive: `benchmarks/results/2026-09-08-effects-production.json`.
At 48kHz/128 frames on Apple M4, individual new-effect default process p99 is at
most 31.042µs; the maximum-length IR case is 443.292µs. These are per-effect
offline timings, with no device/callback/xrun claim. The 16384-frame native/Wasm
comparison includes all 16 typed effects and a non-silent uploaded custom IR;
maximum PCM difference is zero for that fixture, with no process memory growth.

Production direction: layered supersaw chords, short plucks, central sub plus
articulated midbass, harmonic wavetable variations, envelope/automation filter
movement, parallel dynamics and filtered spatial returns. These are general sound
design techniques; no claims of copying Au5/Virtual Riot patches or matching
commercial release quality follow from finite PCM or loudness measurements.
