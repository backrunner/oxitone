use super::*;
use oxitone_graph::NoteEvent;
#[path = "../../render/tests/common/allocations.rs"]
mod allocations;

fn machine() -> Machine {
    Machine {
        voices: [Voice::default(); 4],
        rate: 48000.0,
        level: 0.8,
        decay: 1.0,
        noise: 1,
    }
}

#[test]
fn all_pads_are_audible_finite_and_end_in_silence() {
    for pitch in [36, 38, 42, 46] {
        let mut kit = machine();
        kit.trigger(pitch, 0.8);
        let mut energy = 0.0;
        for _ in 0..48000 {
            for sample in kit.tick() {
                assert!(sample.is_finite());
                energy += f64::from(sample).powi(2);
            }
        }
        assert!(energy > 0.1);
        assert_eq!(kit.tick(), [0.0, 0.0]);
    }
}

#[test]
fn closed_hat_chokes_open_hat_and_reset_preserves_parameters() {
    let mut kit = machine();
    kit.level = 0.3;
    kit.decay = 1.5;
    kit.trigger(46, 1.0);
    kit.tick();
    assert!(kit.voices[3].remaining > 0);
    kit.trigger(42, 1.0);
    assert_eq!(kit.voices[3].remaining, 0);
    kit.reset();
    let render = |kit: &mut Machine| {
        kit.trigger(38, 1.0);
        std::array::from_fn::<_, 512, _>(|_| kit.tick())
    };
    let first = render(&mut kit);
    kit.reset();
    assert_eq!(first, render(&mut kit));
    assert_eq!((kit.level, kit.decay), (0.3, 1.5));
}

#[test]
fn abi_offsets_parameter_before_note_tail_and_no_heap_activity() {
    let mut kit = machine();
    let mut left = [0.0; 128];
    let mut right = [0.0; 128];
    let outputs = [left.as_mut_ptr(), right.as_mut_ptr()];
    let notes = [NoteEvent {
        frame_offset: 7,
        kind: NoteEventKind::NoteOn,
        pitch: 38,
        velocity: 0.8,
    }];
    let parameters = [OxiParameterEventV1 {
        frame_offset: 7,
        parameter_index: 1,
        value: 2.0,
    }];
    let ctx = OxiProcessContextV1 {
        frames: 128,
        sample_rate: 48000.0,
        inputs: std::ptr::null(),
        input_count: 0,
        outputs: outputs.as_ptr(),
        output_count: 2,
        notes: notes.as_ptr(),
        note_count: 1,
        parameters: parameters.as_ptr(),
        parameter_count: 1,
        sidechain: std::ptr::null(),
        sidechain_count: 0,
    };
    let ptr = (&mut kit as *mut Machine).cast();
    let mut status = 0;
    let mut remaining = 0;
    let counts = allocations::count(|| {
        for _ in 0..100 {
            unsafe {
                reset(ptr);
                status |= process(ptr, &ctx);
                remaining = tail(ptr);
            }
        }
    });
    assert_eq!(counts, (0, 0));
    assert_eq!(status, 0);
    assert_eq!(remaining, 48000 - 121);
    assert_eq!(left[..8], [0.0; 8]);
    assert!(left[8..].iter().any(|v| *v != 0.0));
    let first = left;
    unsafe {
        reset(ptr);
        process(ptr, &ctx);
    }
    assert_eq!(first, left);
}
