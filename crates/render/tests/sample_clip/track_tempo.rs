use super::*;

#[test]
fn local_track_sample_modes_ignore_global_ramps_and_keep_their_windows() {
    for mode in [TempoSync::Off, TempoSync::Repitch, TempoSync::Stretch] {
        let dir = out_dir(&format!("track-clock-{mode:?}"));
        let mut s = clip_snapshot(&dir, mode);
        s.tracks[0].tempo = Some(120.0);
        s.sample_clips[0].start_beat = beat(2, 1);
        s.sample_clips[0].duration_beats = Some(beat(4, 1));
        s.tempo_map[0].curve = Some(TempoCurve::Exponential);
        let registry = builtin_registry().unwrap();
        let store = SampleStore::new(None);
        let plan = oxitone_graph::compile_plan(&s, &registry, &store, &Default::default()).unwrap();
        assert_eq!(
            (
                plan.sample_clips[0].start_frame,
                plan.sample_clips[0].end_frame
            ),
            (48000, 144000)
        );
        assert!(
            plan.tempo
                .beat_to_frame(plan.content_end_beat)
                .abs_diff(144000)
                <= 1
        );
        let (left, _, latency) = render_frames(&s, 160000);
        let (count, spacing) = if mode == TempoSync::Off {
            (4, BEAT)
        } else {
            (8, BEAT / 2)
        };
        let expected: Vec<_> = (0..count)
            .map(|k| 48000 + k * spacing + latency as usize)
            .collect();
        assert_onsets_match_threshold(
            &left,
            &expected,
            if mode == TempoSync::Stretch { 320 } else { 64 },
            if mode == TempoSync::Stretch {
                0.05
            } else {
                0.2
            },
            "local track",
        );
        s.tempo_map[0].curve = Some(TempoCurve::Linear);
        let (other, _, _) = render_frames(&s, 160000);
        assert_eq!(
            left, other,
            "local clip audio must not depend on project ramp"
        );

        s.samples[0].musical_length_beats = None;
        s.sample_clips[0].duration_beats = None;
        let plan = oxitone_graph::compile_plan(&s, &registry, &store, &Default::default()).unwrap();
        assert_eq!(plan.sample_clips[0].duration_beats, beat(8, 1));
        assert_eq!(plan.sample_clips[0].end_frame, 240000);
        s.sample_clips[0].loop_spec = Some(LoopSpec {
            start_beat: None,
            length_beats: beat(2, 1),
            count: Some(2),
            last_beat: None,
        });
        let plan = oxitone_graph::compile_plan(&s, &registry, &store, &Default::default()).unwrap();
        assert_eq!(
            plan.sample_clips[0].loop_region.unwrap().until_frame,
            144000
        );
        let mut node = oxitone_render::clip::ClipNode::new(&plan.sample_clips[0], 48000, 128);
        let (mut left, mut right) = ([0.0; 128], [0.0; 128]);
        assert_eq!(
            allocations::count(|| {
                node.reset();
                node.render(48128, 128, &plan.tempo, &mut left, &mut right);
            }),
            (0, 0)
        );
    }
}
