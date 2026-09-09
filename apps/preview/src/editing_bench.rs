//! UI/control latency only: no device, DSP callback or GPU frame timing.
use crate::{
    automation_curve,
    document_wire::{AutomationPoint, SourceNote},
    note_transform::{transform, NoteAction},
    piano_layout::Snap,
};
use std::{hint::black_box, time::Instant};
fn measure(name: &str, mut work: impl FnMut()) {
    for _ in 0..100 {
        work();
    }
    let mut samples = Vec::with_capacity(1000);
    for _ in 0..1000 {
        let start = Instant::now();
        work();
        samples.push(start.elapsed().as_secs_f64() * 1e6);
    }
    samples.sort_by(f64::total_cmp);
    eprintln!("{{\"scenario\":\"{name}\",\"warmup\":100,\"samples\":1000,\"p50Us\":{},\"p95Us\":{},\"p99Us\":{},\"device\":null,\"sampleRate\":null,\"blockSize\":null,\"cpuUtilization\":null,\"callbackP99\":null,\"xruns\":null,\"gpuFrameTime\":null}}", samples[500], samples[950], samples[990]);
}
#[test]
#[ignore = "release UI editing microbenchmark"]
fn benchmark_editing_gestures() {
    for count in [128, 4096] {
        measure(&format!("note-brush-{count}"), || {
            black_box(crate::note_brush::stroke_cells(
                (0.5, 24.5),
                (count as f64 - 0.5, 60.5),
            ));
        });
    }
    for count in [128, 10_000] {
        let notes: Vec<_> = (0..count)
            .map(|index| SourceNote {
                pitch: 36 + (index % 48) as u8,
                start: index as f64 * 0.25,
                duration: 0.5,
                velocity: 0.7,
            })
            .collect();
        measure(&format!("note-group-{count}"), || {
            black_box(transform(
                black_box(&notes),
                NoteAction::Move,
                0.375,
                4,
                Snap::Quarter,
            ));
        });
        measure(&format!("note-content-projection-{count}"), || {
            black_box(crate::piano_note_paint::visible_notes(
                black_box(&notes),
                &Default::default(),
                None,
            ));
        });
        let outputs: Vec<_> = notes
            .iter()
            .enumerate()
            .map(|(index, note)| crate::document_wire::SourceOutput {
                note: note.clone(),
                select: serde_json::json!({ "step": index }),
            })
            .collect();
        let layout =
            crate::piano_layout::PianoLayout::new(1000., 400., 4., [60].into_iter(), 1., 1., None);
        let mut changes = vec![];
        measure(&format!("velocity-stroke-{count}"), || {
            crate::note_velocity::stroke(
                black_box(&outputs),
                &Default::default(),
                &mut changes,
                (76., 330.),
                (900., 380.),
                layout,
            );
            black_box(&changes);
        });
        let selection_changes: Vec<_> = outputs
            .iter()
            .enumerate()
            .map(|(index, output)| crate::note_gesture::NoteChange {
                index: Some(index),
                before: output.note.clone(),
                after: output.note.clone(),
                selector: output.select.clone(),
            })
            .collect();
        measure(&format!("note-selection-restore-{count}"), || {
            black_box(crate::note_commit::selection_indices(
                black_box(&outputs),
                &selection_changes,
            ));
        });
    }
    for count in [128, 4096] {
        let clips: Vec<_> = (0..count)
            .map(|i| crate::playlist_actions::Clip {
                kind: crate::playlist_edit::ResourceKind::Pattern,
                id: format!("clip-{i}"),
                resource: "pattern".into(),
                track: format!("track-{}", i % 32),
                start: i as f64,
                length: 4.,
                enabled: true,
            })
            .collect();
        let drag = crate::playlist_edit::Drag {
            mode: crate::playlist_edit::DragMode::Move,
            kind: crate::playlist_edit::ResourceKind::Pattern,
            resource: "pattern".into(),
            clip: Some(("clip-1".into(), 0)),
            length: 4.,
            anchor: gpui::point(gpui::px(0.), gpui::px(0.)),
            offset: 0.,
            target: Some(("track-2".into(), 8.)),
            moved: true,
        };
        measure(&format!("playlist-content-projection-{count}"), || {
            black_box(crate::playlist_projection::project(
                black_box(clips.clone()),
                Some(&drag),
            ));
        });
    }
    for count in [128, 4096] {
        let points: Vec<_> = (0..count)
            .map(|i| AutomationPoint {
                beat: i as f64 * 0.25,
                value: (i % 10) as f64 / 10.,
                curve: Some(oxitone_core::wire::Curve::Smooth {}),
            })
            .collect();
        measure(&format!("curve-preview-{count}"), || {
            let source = automation_curve::compiled_points(black_box(&points)).unwrap();
            for pixel in 0..1024 {
                black_box(source.value_at(
                    pixel as f64 / 1024. * count as f64 * 0.25,
                    &Default::default(),
                ));
            }
        });
    }
}
