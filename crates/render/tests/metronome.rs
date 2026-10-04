#[path = "common/allocations.rs"]
mod allocations;
mod common;
use oxitone_render::{builtin_registry, RenderGraph, RenderGraphOptions, SampleStore};

#[test]
fn runtime_toggle_and_click_render_never_allocate_or_reset_transport() {
    let mut graph = RenderGraph::compile(
        &common::base_snapshot(),
        &builtin_registry().unwrap(),
        &SampleStore::new(None),
        &RenderGraphOptions {
            master_limiter: false,
            metronome_level: Some(0.5),
            ..Default::default()
        },
    )
    .unwrap();
    graph.transport_mut().begin_render(0);
    let mut left = [0.; 128];
    let mut right = [0.; 128];
    for enabled in [false, true, false, true] {
        graph.seek(24000);
        let before = graph.transport().cursor;
        let counts = allocations::count(|| {
            graph.set_metronome_enabled(enabled);
            graph.process_block(&mut left, &mut right);
        });
        assert_eq!(counts, (0, 0));
        assert_eq!(graph.transport().cursor, before + 128);
        assert_eq!(left, right);
        assert_eq!(left.iter().any(|s| s.abs() > 0.1), enabled);
    }
}
