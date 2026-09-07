# Electronic synthesis and production

The electronic demo uses native synthesis for electronic parts, recorded soft/grand
piano in quieter sections, and the C ABI drum machine for percussion. The lofi demo is retired. Shared native
and Wasm DSP and silent-test requirements continue to apply.

## Synthesis contract

`oxitone.wavetable@1.0.0` retains its original defaults and parameter indices.
Additive controls provide 16 unison voices per oscillator, three eight-frame
wavetable banks (`analog`, `digital`, `vowel`), phase warp (`bend`, `asymmetric`,
`sync`), B-to-A phase modulation (`fm`) and ring modulation (`ring`). Bank `pair`
retains the original wave/morphTo interpolation. Position scans adjacent bank
frames continuously. Tables are generated and bandlimited before rendering.

A/B `octave` is an integer −4…4 (default 0), added as 12×octave to the existing
±24-semitone `pitch`. Their independent `level` is 0…1 (default 1), applied to the
audible mix after FM/ring; silent B can still modulate A. Sub is centered, after
the main filter and before amp/velocity, with level 0…1 (default 0), independent
octave −4…4 (default −1) and wave `sine/triangle/saw/square/pulse/rounded` (0…5,
default sine). Pulse is a DC-free 25% duty wave; rounded uses odd 1/n³ partials.
Existing sine Sub preserves its analytic phase path. A/B octave never transposes
Sub. Global pitch modulation/glide affects all three. Frequencies above 0.49 of
the sample rate are suppressed. Bank/warp/FM mip headroom reduces high-frequency
content but is not a guarantee of alias-free audio-rate FM at extreme settings.

Two note-triggered LFOs, amp/filter/modulation envelopes, velocity, key tracking,
deterministic note random and four macros feed eight fixed modulation slots.
Each slot declares source/target/amount/curve. Source IDs and target IDs are
ordered by `packages/protocol/src/synth-modulation.ts`; sources are bipolar for
LFO/keytrack/random and unipolar for envelopes/velocity/macros. Amount ±1 spans
±24 semitones for pitch, ±48 semitones for cutoff and ±1 for normalized targets.
Targets sum before clamping. Feedback between modulation slots is not allowed.
Curve is a bounded monotonic remap. All storage and indices are fixed before DSP.

ADSR curve values −1..1 change segment curvature while preserving exact segment
durations and continuity at retrigger/release. Zero retains the previous linear
envelope. Note-triggered LFO rates use Hz; authoring can derive them from the song
BPM. This version does not claim native MPE, sample resynthesis, arbitrary imported
wavetable banks or full compatibility with commercial synthesizers.

## Effects and drums

The delay supports ping-pong, feedback highpass and wet ducking,
and advances stereo time smoothing exactly once per sample. A 4x-oversampled
multimode distortion and a stereo-linked three-band upward/downward compressor are built in.
Compression boosts are bounded and gated below the detector floor; no noise gain
from digital silence. Host insert mix/bypass and PDC apply to every effect.
Drum timbre parameters control kick tuning/sweep/click, snare body/snap, hat color,
and per-pad decay. The default kit remains compatible; an electronic preset is
shared by the native and statically linked Wasm drum implementation.

`oxitone.distortion`: mode soft/hard/fold/asymmetric (0…3); driveDb 0…36,
outputDb −36…12, toneHz 200…20000, bias −1…1. Reports 36 frames latency.
`oxitone.multiband`: depth 0…1; bounded upwardDb 0…24, downwardRatio 1…20;
upperThresholdDb −36…0, lowerThresholdDb −60…−24; attackMs 0.1…100,
releaseMs 10…1000; crossovers lowHz 60…500/highHz 1000…8000; three band gainDb
trims ±18 and outputDb −24…12. Complementary residual first-order splits reconstruct
the input at unity gain; these are broad, overlapping bands, not steep linear-phase
crossovers. Upward gain fades out from −60 to −72 dB; digital silence stays zero.
`oxitone.delay` adds pingPong (0/1, default 0), highpassHz 20…2000 (20 bypasses
feedback highpass), ducking 0…1 (default 0). Its time smoother advances once per
stereo frame; this fixes the old channel timing mismatch.
Drums append kickTune/kickSweep/kickClick, snareTune/snareSnap, four pad decay
multipliers and hatTone; exact metadata is mirrored in the C descriptor and manifest.

## Production and acceptance

Use an original F# minor hook, eight-bar phrasing, builds, contrasting drops and
an ending. Drops layer highpassed supersaws, a central lead and separate mono sub
and mid-bass parts. Duck music and effect returns at the kick/snare pattern;
arrange density before loudness. Track EQ, controlled nonlinear processing,
band-limited echo/reverb sends, bus dynamics and peak limiting are explicit code.
Export a full master and reference analysis: section loudness, true peak, spectrum,
stereo correlation, mono low-end and native/Wasm parity. Objective measurements
and silent tests do not establish subjective equivalence to commercial releases.

### After the Horizon arrangement revision

Keep the existing eight-bar hook's pitches, note starts, lengths and velocities in
both drops. Preserve the F# minor chord progression, 140 BPM and 104-bar form.
The arrangement develops four-bar accompaniment cells:
Drop I introduces shuffled tops and chord plucks after the first statement;
Drop II adds glass answers, longer final chords and a high harmonic halo. The
eighth-bar turnaround removes late saw/kick accents to expose bass and delay tails.
FM/formant answers have separate onsets; sub notes connect beneath the syncopation.
A rounded Reese and offbeat plucks develop the bridge/reprise. Noise lifts and
downlifters use different envelopes, with pad/lift fades before each drop.

Saw brightness follows chord attacks, gain recovery follows the actual kick/snare
events, and phrase delay throws automate a send before the return (preserving its
tail). Keep saw widening above the bass region; the mono sub is separately filtered.
Selected processing: Multiband Dynamics on saw/midbass, nonlinear filtering on FM
bass, short convolution drum room, light tape/transient shaping, filtered ducked
hall/echo, detector-highpassed master glue, bass centering and lookahead limiting.
Master fader trim/fades occur after inserts and are included in exported true-peak
acceptance; the insert ceiling alone is not the delivery ceiling.

The offline render checks finite PCM, duration, −18…−9 integrated LUFS, <−0.8 dBTP,
positive section correlation, small DC, drop crest >6 dB and low side/mid <−20 dB.
Each drop must exceed its build's section RMS by 2 dB. Four-bar energy measurements
are included in report.json. These are regression bounds, not a loudness target
for every project or proof of subjective mix quality. Final evidence is archived
in the benchmark archive; the earlier 20-track version is recorded in
`benchmarks/results/2026-09-08-horizon-arrangement.json`.

### Piano and bass-weight revision

Soft Piano uses quiet recorded velocity layers in the intro and theme, with Grand
Piano voicings in the break/reprise. Salamander Grand Piano v3 recordings by
Alexander Holm (CC BY 3.0), distributed as FLAC by sfzinstruments/kinwie, are pinned
to revision `3382bf9496bba2486f5ab0de55a264d1dfc38404`. Explicit prepare downloads
120 files (layers 2/5/9/13, minor-third roots A0–C8), verifies their sizes/SHA-256,
and writes attribution. No ordinary build/test downloads. The demo registers only
52 files in keys 48–84 (~256 MiB decoded PCM); sample bytes stay outside Git.
CI uses tiny local fixtures for native piano layer selection and graph validation.

Drop sub is sustained and uses a separate bus, with a short kick-only gain envelope
recovering after 0.28 beats (120 ms). A separate filtered harmonic bassline supports
small-speaker translation; Reese, FM turbine, vowel and sync-warp stabs use distinct
rhythmic roles. Avoid combining phase warp and FM in these patches: conservative
bandlimiting compounds their mip headroom and removes carrier partials. No engine
anti-alias safeguards are relaxed. Supersaws include middle-register thirds, a
central body, sustained gates and brighter automated filtering. Snare has a clap
layer, and kick duration leaves room for bass recovery. Original drop hook,
140 BPM and 104-bar form are preserved. MIDI alone uses explicit timbre-family
channel sharing; independent patches/CCs and same-pitch overlaps cannot be faithfully
reproduced on shared MIDI channels.

Public technique references, used for principles rather than copied patches:
- Au5, [Supersaw Killer](https://www.youtube.com/watch?v=IhvO8grER5s): stereo,
  additional oscillators, filtering and chord voicings.
- Au5, [Cracked Bass Drops Technique](https://www.youtube.com/watch?v=t3__vw0vaV8):
  alternating bass gestures and rhythmic flow.
- Au5, [1 Minute Reese](https://www.youtube.com/watch?v=W87uuuGcq9c): Reese layering.
- [Virtual Riot / Modestep interview](https://splice.com/blog/virtual-riot/):
  drum fills, one-shots, top detail and processing experimentation.

Capability references:
- [Serum 2](https://xferrecords.com/products/serum-2): continuous frame positions,
  FM/phase distortion/ring modulation and oscillator warps.
- [Vital](https://vital.audio/): wavetable warping, modulation remapping, envelopes,
  randomized sources and expressive modulation.
