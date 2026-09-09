//! Offline-parity tests (03-audio-runtime-spec.md §Offline parity): fixed
//! seed/rate/block-size renders are byte-stable, and 64/128/256-frame
//! block sizes render the same project within tolerance.

mod common;

use common::*;
use oxitone_core::wire::{AutomationLaneSpec, AutomationSourceSpec, AutomationTarget, WaveKind};
use oxitone_render::{
    builtin_registry, render_wav, RenderGraph, RenderGraphOptions, RenderOptions, SampleStore,
};

/// Project with pattern notes, a sample-free signal path, inserts and
/// sends (no automation): block-size changes must not alter the output
/// beyond float noise.
fn parity_snapshot() -> oxitone_core::wire::ProjectSnapshot {
    let mut snapshot = base_snapshot();
    snapshot.tracks = vec![track("trk_p", &["chn_p"], &["pcl_p"], &[])];
    snapshot.patterns = vec![pattern(
        "pat_p",
        (4, 1),
        vec![
            note(60, (0, 1), (1, 2), 0.7),
            note(64, (1, 1), (1, 2), 0.6),
            note(67, (2, 1), (1, 2), 0.5),
            note(72, (3, 1), (1, 2), 0.6),
        ],
    )];
    snapshot.pattern_clips = vec![pattern_clip("pcl_p", "pat_p", "trk_p", (0, 1), (8, 1))];
    snapshot.channels = vec![channel(
        "chn_p",
        "mix_p",
        wavetable_ref(&[("filter.cutoff", 3200.0)]),
        vec![effect_ref("oxitone.chorus", &[])],
    )];
    snapshot.mixer_channels = vec![
        mixer_channel("mix_p", vec![], vec![send("mix_fx", 0.3, false)]),
        mixer_channel("mix_fx", vec![effect_ref("oxitone.reverb", &[])], vec![]),
    ];
    snapshot
}

fn render_with_block_size(
    snapshot: &oxitone_core::wire::ProjectSnapshot,
    block_size: u32,
    frames: usize,
) -> Vec<f32> {
    let registry = builtin_registry().unwrap();
    let store = SampleStore::new(None);
    let options = RenderGraphOptions {
        compile: oxitone_graph::CompileOptions {
            block_size: Some(block_size),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut graph = RenderGraph::compile(snapshot, &registry, &store, &options).unwrap();
    graph.transport_mut().begin_render(0);
    let mut out = Vec::with_capacity(frames);
    let mut l = vec![0.0f32; block_size as usize];
    let mut r = vec![0.0f32; block_size as usize];
    let mut done = 0;
    while done < frames {
        graph.process_block(&mut l, &mut r);
        let n = (frames - done).min(block_size as usize);
        out.extend_from_slice(&l[..n]);
        done += n;
    }
    assert!(!graph.faulted());
    out
}

#[test]
fn block_sizes_render_identically_without_automation() {
    let snapshot = parity_snapshot();
    let frames = 4 * 24_000; // 4 beats at 120 BPM
    let reference = render_with_block_size(&snapshot, 128, frames);
    for block_size in [64, 256] {
        let other = render_with_block_size(&snapshot, block_size, frames);
        assert_eq!(other.len(), reference.len());
        let mut max_delta = 0.0f32;
        for (a, b) in reference.iter().zip(other.iter()) {
            max_delta = max_delta.max((a - b).abs());
        }
        assert!(
            max_delta < 1e-5,
            "block size {block_size} diverges: max delta {max_delta}"
        );
    }
}

#[test]
fn block_sizes_stay_close_with_control_rate_automation() {
    let mut snapshot = parity_snapshot();
    snapshot.automation = vec![AutomationLaneSpec {
        playback: None,
        id: "auto_lvl".into(),
        target: AutomationTarget {
            scope: None,
            entity_id: "chn_p".into(),
            parameter_id: "level".into(),
        },
        source: AutomationSourceSpec::Wave {
            wave: WaveKind::Sine,
            period_beats: beat(8, 1),
            phase: None,
            min: Some(0.4),
            max: Some(1.0),
            pulse_width: None,
        },
        combine: None,
        loop_spec: None,
        last_beat: None,
    }];
    let frames = 4 * 24_000;
    let reference = render_with_block_size(&snapshot, 128, frames);
    for block_size in [64, 256] {
        let other = render_with_block_size(&snapshot, block_size, frames);
        let mut max_delta = 0.0f32;
        let mut sum_sq = 0.0f64;
        for (a, b) in reference.iter().zip(other.iter()) {
            max_delta = max_delta.max((a - b).abs());
            sum_sq += ((a - b) as f64) * ((a - b) as f64);
        }
        let rms = (sum_sq / reference.len() as f64).sqrt();
        // Control-rate automation re-reads at each block start, so the
        // smoother tracks differ by at most one block of lag.
        assert!(
            max_delta < 0.05 && rms < 0.005,
            "block size {block_size}: max {max_delta} rms {rms}"
        );
    }
}

#[test]
fn wav_export_is_byte_stable_across_renders() {
    let dir = out_dir("parity");
    let snapshot = parity_snapshot();
    let store = SampleStore::new(None);
    let registry = builtin_registry().unwrap();
    let mut hashes = Vec::new();
    for name in ["r1.wav", "r2.wav", "r3.wav"] {
        let options = RenderOptions::new(dir.join(name));
        render_wav(&snapshot, &registry, &store, &options).unwrap();
        hashes.push(oxitone_samples::sha256_hex(
            &std::fs::read(dir.join(name)).unwrap(),
        ));
    }
    assert_eq!(hashes[0], hashes[1]);
    assert_eq!(hashes[1], hashes[2]);
}

#[test]
fn pcm16_export_with_dither_is_deterministic() {
    let dir = out_dir("parity-dither");
    let snapshot = parity_snapshot();
    let store = SampleStore::new(None);
    let registry = builtin_registry().unwrap();
    let mut hashes = Vec::new();
    for name in ["d1.wav", "d2.wav"] {
        let mut options = RenderOptions::new(dir.join(name));
        options.bit_depth = oxitone_render::BitDepth::Pcm16;
        render_wav(&snapshot, &registry, &store, &options).unwrap();
        hashes.push(oxitone_samples::sha256_hex(
            &std::fs::read(dir.join(name)).unwrap(),
        ));
    }
    assert_eq!(hashes[0], hashes[1], "dithered export must be seeded");
    // Dithered and undithered 16-bit exports differ only at LSB level.
    let mut options = RenderOptions::new(dir.join("d3.wav"));
    options.bit_depth = oxitone_render::BitDepth::Pcm16;
    options.dither = oxitone_render::DitherMode::None;
    render_wav(&snapshot, &registry, &store, &options).unwrap();
    let dithered = std::fs::read(dir.join("d1.wav")).unwrap();
    let plain = std::fs::read(dir.join("d3.wav")).unwrap();
    assert_eq!(dithered.len(), plain.len());
    let mut max_step = 0u32;
    for (a, b) in dithered[44..]
        .as_chunks::<2>()
        .0
        .iter()
        .zip(plain[44..].as_chunks::<2>().0.iter())
    {
        let va = i16::from_le_bytes(*a) as i32;
        let vb = i16::from_le_bytes(*b) as i32;
        max_step = max_step.max(va.abs_diff(vb));
    }
    assert!(max_step <= 2, "dither changed samples by {max_step} LSB");
}
