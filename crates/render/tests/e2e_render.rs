//! End-to-end render test: wavetable synth + pattern clips + automation
//! (gate / sine / chance) + mixer sends + sidechain compressor +
//! delay/reverb → `renderWav`. Verifies byte determinism (two renders hash
//! identically), sane loudness, and stems summing to the master within
//! floating-point tolerance (06-format-and-export.md §Stem 导出).

mod common;

use common::*;
use oxitone_core::wire::{
    AutomationCombine, AutomationLaneSpec, AutomationSourceSpec, AutomationTarget, ChanceSpec,
    WaveKind,
};
use oxitone_render::{builtin_registry, render_wav, RenderOptions, SampleStore, StemMode};

fn lane(
    id: &str,
    entity: &str,
    parameter: &str,
    source: AutomationSourceSpec,
) -> AutomationLaneSpec {
    AutomationLaneSpec {
        id: id.into(),
        target: AutomationTarget {
            entity_id: entity.into(),
            parameter_id: parameter.into(),
        },
        source,
        combine: None,
        loop_spec: None,
        last_beat: None,
    }
}

/// The e2e project: two channels/buses, an FX bus, a sidechained
/// compressor, and three automation styles. 16 beats at 120 BPM (8 s).
fn e2e_snapshot() -> oxitone_core::wire::ProjectSnapshot {
    let mut snapshot = base_snapshot();
    snapshot.tracks = vec![
        track("trk_kick", &["chn_kick"], &["pcl_kick"], &[]),
        track("trk_pad", &["chn_pad"], &["pcl_pad"], &[]),
    ];
    snapshot.patterns = vec![
        pattern(
            "pat_kick",
            (4, 1),
            (0..4).map(|b| note(36, (b, 1), (1, 4), 0.9)).collect(),
        ),
        pattern(
            "pat_pad",
            (4, 1),
            vec![note(60, (0, 1), (4, 1), 0.5), note(64, (0, 1), (4, 1), 0.4)],
        ),
    ];
    snapshot.pattern_clips = vec![
        pattern_clip("pcl_kick", "pat_kick", "trk_kick", (0, 1), (16, 1)),
        pattern_clip("pcl_pad", "pat_pad", "trk_pad", (0, 1), (16, 1)),
    ];
    snapshot.channels = vec![
        channel(
            "chn_kick",
            "mix_kick",
            wavetable_ref(&[("oscA.pitch", -12.0), ("filter.cutoff", 900.0)]),
            vec![effect_ref(
                "oxitone.delay",
                &[("timeBeats", 0.5), ("feedback", 0.3)],
            )],
        ),
        channel(
            "chn_pad",
            "mix_pad",
            wavetable_ref(&[("filter.cutoff", 2400.0)]),
            vec![],
        ),
    ];
    snapshot.mixer_channels = vec![
        mixer_channel(
            "mix_kick",
            vec![],
            vec![send("mix_fx", 0.25, false), send("mix_pad", 0.8, true)],
        ),
        mixer_channel(
            "mix_pad",
            vec![effect_ref(
                "oxitone.compressor",
                &[("thresholdDb", -18.0), ("ratio", 4.0)],
            )],
            vec![send("mix_fx", 0.2, false)],
        ),
        mixer_channel(
            "mix_fx",
            vec![
                {
                    let mut fx = effect_ref("oxitone.reverb", &[("decaySeconds", 1.2)]);
                    fx.mix = Some(0.6);
                    fx
                },
                effect_ref("oxitone.delay", &[("timeBeats", 0.75), ("feedback", 0.25)]),
            ],
            vec![],
        ),
    ];
    snapshot.automation = vec![
        lane(
            "auto_gate",
            "chn_pad",
            "level",
            AutomationSourceSpec::Gate {
                period_beats: beat(1, 2),
                duty: 0.75,
                phase: None,
                on: None,
                off: None,
            },
        ),
        lane(
            "auto_cutoff",
            "chn_kick",
            "filter.cutoff",
            AutomationSourceSpec::Wave {
                wave: WaveKind::Sine,
                period_beats: beat(8, 1),
                phase: None,
                min: Some(0.2),
                max: Some(0.9),
                pulse_width: None,
            },
        ),
        AutomationLaneSpec {
            combine: Some(AutomationCombine::Replace),
            ..lane(
                "auto_chance",
                "chn_pad",
                "pan",
                AutomationSourceSpec::Chance(ChanceSpec {
                    probability: 0.7,
                    seed: 17,
                    smooth_beats: Some(beat(1, 16)),
                    random_phase: None,
                    rate: Some(1.0),
                    interval_beats: None,
                }),
            )
        },
        lane(
            "auto_send",
            "mix_pad",
            "send.mix_fx.ratio",
            AutomationSourceSpec::Wave {
                wave: WaveKind::Triangle,
                period_beats: beat(16, 1),
                phase: None,
                min: Some(0.1),
                max: Some(0.5),
                pulse_width: None,
            },
        ),
    ];
    snapshot
}

fn read_wav_f32(path: &std::path::Path) -> (Vec<f32>, Vec<f32>) {
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(&bytes[..4], b"RIFF");
    let data = &bytes[44..];
    let mut left = Vec::with_capacity(data.len() / 8);
    let mut right = Vec::with_capacity(data.len() / 8);
    for frame in data.as_chunks::<8>().0 {
        left.push(f32::from_le_bytes(frame[..4].try_into().unwrap()));
        right.push(f32::from_le_bytes(frame[4..].try_into().unwrap()));
    }
    (left, right)
}

fn render_e2e(dir: &std::path::Path, name: &str, stems: StemMode) -> oxitone_render::RenderReport {
    let snapshot = e2e_snapshot();
    let store = SampleStore::new(None);
    let mut options = RenderOptions::new(dir.join(name));
    options.stems = stems;
    render_wav(&snapshot, &builtin_registry().unwrap(), &store, &options).unwrap()
}

#[test]
fn end_to_end_render_is_deterministic_and_metered() {
    let dir = out_dir("e2e");
    let snapshot = e2e_snapshot();
    let store = SampleStore::new(None);
    let registry = builtin_registry().unwrap();
    let options_a = RenderOptions::new(dir.join("a.wav"));
    let options_b = RenderOptions::new(dir.join("b.wav"));
    let report_a = render_wav(&snapshot, &registry, &store, &options_a).unwrap();
    let report_b = render_wav(&snapshot, &registry, &store, &options_b).unwrap();

    // Byte determinism: same snapshot/options → identical WAV bytes.
    let bytes_a = std::fs::read(dir.join("a.wav")).unwrap();
    let bytes_b = std::fs::read(dir.join("b.wav")).unwrap();
    assert_eq!(
        oxitone_samples::sha256_hex(&bytes_a),
        oxitone_samples::sha256_hex(&bytes_b),
        "two renders of the same snapshot differ"
    );

    // Report sanity: one master file, ~8 s, limiter-controlled peak,
    // finite loudness, nonzero graph latency (limiter lookahead).
    assert_eq!(report_a.files.len(), 1);
    let master = &report_a.files[0];
    assert!((master.duration_seconds - 8.0).abs() < 0.01);
    assert!(master.peak_dbfs <= 0.1, "peak {} dBFS", master.peak_dbfs);
    assert!(master.peak_dbfs > -12.0, "peak {} dBFS", master.peak_dbfs);
    assert!(master.integrated_lufs > -40.0 && master.integrated_lufs < 0.0);
    assert!(master.true_peak_dbfs >= master.peak_dbfs - 0.5);
    assert!(report_a.graph_latency_frames >= 200);
    // Identical except for the output path embedded in each report.
    assert_eq!(report_a.files.len(), report_b.files.len());
    for (a, b) in report_a.files.iter().zip(report_b.files.iter()) {
        assert_eq!(a.stem, b.stem);
        assert_eq!(a.duration_seconds, b.duration_seconds);
        assert_eq!(a.peak_dbfs, b.peak_dbfs);
        assert_eq!(a.true_peak_dbfs, b.true_peak_dbfs);
        assert_eq!(a.integrated_lufs, b.integrated_lufs);
    }
    assert_eq!(report_a.graph_latency_frames, report_b.graph_latency_frames);
}

#[test]
fn stems_sum_matches_master() {
    let dir = out_dir("e2e-stems");
    let report = render_e2e(&dir, "stems", StemMode::MixerChannels);
    // master + 3 buses.
    assert_eq!(report.files.len(), 4);
    let (master_l, master_r) = read_wav_f32(&dir.join("stems").join("master.wav"));
    let mut sum_l = vec![0.0f32; master_l.len()];
    let mut sum_r = vec![0.0f32; master_r.len()];
    for bus in ["mix_kick", "mix_pad", "mix_fx"] {
        let (l, r) = read_wav_f32(&dir.join("stems").join(format!("{bus}.wav")));
        for i in 0..sum_l.len() {
            sum_l[i] += l[i];
            sum_r[i] += r[i];
        }
    }
    // Stem taps reconstruct the Master bus *input* sample-aligned; the
    // master file additionally passes the master fader (equal-power
    // balance: 1/sqrt(2) at center) and the limiter (unity gain here,
    // graph-latency shift). Compensate both factors.
    let latency = report.graph_latency_frames as usize;
    let fader = std::f32::consts::FRAC_1_SQRT_2;
    let mut max_delta = 0.0f32;
    for i in latency..sum_l.len() {
        max_delta = max_delta.max((sum_l[i - latency] * fader - master_l[i]).abs());
        max_delta = max_delta.max((sum_r[i - latency] * fader - master_r[i]).abs());
    }
    assert!(max_delta < 2e-3, "stems vs master max delta {max_delta}");
}

#[test]
fn track_stems_and_metronome_export() {
    let dir = out_dir("e2e-tracks");
    let snapshot = e2e_snapshot();
    let store = SampleStore::new(None);
    let mut options = RenderOptions::new(dir.join("tracks"));
    options.stems = StemMode::Tracks;
    options.include_metronome = true;
    let report = render_wav(&snapshot, &builtin_registry().unwrap(), &store, &options).unwrap();
    // master + 2 tracks.
    assert_eq!(report.files.len(), 3);
    let stems: Vec<Option<&str>> = report.files.iter().map(|f| f.stem.as_deref()).collect();
    assert!(stems.contains(&None));
    assert!(stems.contains(&Some("trk_kick")));
    assert!(stems.contains(&Some("trk_pad")));
    // Metronome is mixed into master and all stems: every file is
    // non-silent from the first click.
    for file in &report.files {
        assert!(file.peak_dbfs.is_finite(), "{} silent", file.path);
    }
}
