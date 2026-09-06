use oxitone_core::wire::{
    ParameterMapping, ParameterRate, ParameterSmoothing, ParameterSpec, ParameterUnit,
};
use oxitone_mixer::parameter_queue::ParameterQueue;

#[test]
fn large_descriptors_keep_every_parameter_and_latest_value_without_growing() {
    let specs = (0..320)
        .map(|index| ParameterSpec {
            id: format!("p{index}"),
            label: format!("Parameter {index}"),
            unit: ParameterUnit::Normalized,
            min: 0.,
            max: 1.,
            default: 0.,
            smoothing: ParameterSmoothing::None,
            rate: ParameterRate::Control,
            automation: Some(true),
            mapping: Some(ParameterMapping::Linear),
        })
        .collect::<Vec<_>>();
    let mut queue = ParameterQueue::new(&specs);
    drop(specs); // The source descriptor may disappear before this graph.
    for i in 0..320 {
        queue.set(i, 0.);
    }
    for i in (0..320).rev() {
        queue.set(i, 0.75);
    }
    queue.with_events(|events| {
        assert_eq!(events.len(), 320);
        for (i, event) in events.iter().enumerate() {
            assert_eq!(event.parameter_id, format!("p{i}"));
            assert_eq!(event.value, 0.75);
            assert_eq!(event.frame_offset, 0);
        }
    });
    queue.with_events(|events| assert!(events.is_empty()));
}
