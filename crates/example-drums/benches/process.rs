//! Native C entry, four active voices and same-frame parameter/note handling.
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use oxitone_graph::{abi_c::*, NoteEvent, NoteEventKind};

fn bench(c: &mut Criterion) {
    let entry = unsafe { &*oxitone_example_drums::oxitone_plugin_entry_v1() };
    let mut group = c.benchmark_group("plugin/drums_native_entry");
    for frames in [64, 128, 256] {
        let host = OxiHostContextV1 {
            sample_rate: 48000.0,
            max_block_size: frames,
        };
        let instance = unsafe { entry.create.unwrap()(&host) };
        assert_eq!(
            unsafe { entry.prepare.unwrap()(instance, 48000.0, frames) },
            0
        );
        let mut left = vec![0.0; frames as usize];
        let mut right = vec![0.0; frames as usize];
        let outputs = [left.as_mut_ptr(), right.as_mut_ptr()];
        let notes = [36, 38, 42, 46].map(|pitch| NoteEvent {
            frame_offset: 0,
            kind: NoteEventKind::NoteOn,
            pitch,
            velocity: 0.8,
        });
        let parameter = OxiParameterEventV1 {
            frame_offset: 0,
            parameter_index: 1,
            value: 1.0,
        };
        let mut ctx = OxiProcessContextV1 {
            frames,
            sample_rate: 48000.0,
            inputs: std::ptr::null(),
            input_count: 0,
            outputs: outputs.as_ptr(),
            output_count: 2,
            notes: notes.as_ptr(),
            note_count: 4,
            parameters: &parameter,
            parameter_count: 1,
            sidechain: std::ptr::null(),
            sidechain_count: 0,
        };
        let mut block = 0;
        group.bench_with_input(BenchmarkId::from_parameter(frames), &frames, |b, _| {
            b.iter(|| {
                // Retrigger every eight blocks so all four voices remain active.
                ctx.note_count = if block % 8 == 0 { 4 } else { 0 };
                block += 1;
                assert_eq!(unsafe { entry.process.unwrap()(instance, &ctx) }, 0);
                black_box(&left);
            });
        });
        unsafe {
            entry.dispose.unwrap()(instance);
        }
    }
    group.finish();
}
criterion_group!(benches, bench);
criterion_main!(benches);
