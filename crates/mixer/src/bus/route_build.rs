//! Resolve explicit input deliveries and outgoing compensation after all mixer indices are fixed.
use super::{
    routes::{delay, InputSend},
    MixerEngine,
};
use oxitone_core::wire::MixerChannelSpec;

pub(super) fn prepare(engine: &mut MixerEngine, specs: &[MixerChannelSpec]) {
    for spec in specs {
        let dest_index = engine.index[&spec.id];
        for (insert_index, effect) in spec.inserts.iter().enumerate() {
            let Some(routes) = effect
                .instance_id
                .as_ref()
                .and_then(|id| spec.insert_routes.as_ref()?.get(id))
            else {
                continue;
            };
            for (physical, source) in routes.inputs.iter().flatten() {
                let physical: usize = physical.parse().expect("validated input bus");
                let input_index = engine.buses[dest_index].inserts[insert_index]
                    .inputs
                    .iter()
                    .position(|r| r.bus_index == physical)
                    .expect("prepared input bus");
                let source_index = engine.index[source];
                let compensation = engine.pdc.edge_delays[&(source.clone(), spec.id.clone(), true)];
                engine.buses[source_index].input_sends.push(InputSend {
                    dest_index,
                    insert_index,
                    input_index,
                    delay: delay(compensation as usize, engine.max_block),
                });
            }
        }
    }
    let ids: Vec<_> = engine.buses.iter().map(|bus| bus.id.clone()).collect();
    for bus in &mut engine.buses {
        bus.input_sends
            .sort_by_key(|send| (send.dest_index, send.insert_index, send.input_index));
        for slot in &mut bus.inserts {
            for route in &mut slot.outputs {
                let compensation =
                    engine.pdc.edge_delays[&(bus.id.clone(), ids[route.dest_index].clone(), false)];
                route.route_delay = delay(compensation as usize, engine.max_block);
            }
        }
    }
}
