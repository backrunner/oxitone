//! Preallocated routing deliveries after the owner's fader. All destinations follow the source.
use super::Bus;

pub(super) fn deliver(
    buses: &mut [Bus],
    source: usize,
    frames: usize,
    mut stem_tap: Option<&mut [Vec<f32>; 2]>,
) {
    for insert in 0..buses[source].inserts.len() {
        for output in 0..buses[source].inserts[insert].outputs.len() {
            let dest = buses[source].inserts[insert].outputs[output].dest_index;
            debug_assert!(dest > source);
            let (head, tail) = buses.split_at_mut(dest);
            let route = &mut head[source].inserts[insert].outputs[output];
            let (left, right) = if tail[0].is_master {
                match stem_tap.as_deref_mut() {
                    Some([left, right]) => (left, right),
                    None => (&mut tail[0].sum_l, &mut tail[0].sum_r),
                }
            } else {
                (&mut tail[0].sum_l, &mut tail[0].sum_r)
            };
            route.route_delay.process_add(
                &route.aligned[0][..frames],
                &route.aligned[1][..frames],
                1.,
                &mut left[..frames],
                &mut right[..frames],
            );
        }
    }
    for index in 0..buses[source].input_sends.len() {
        let dest = buses[source].input_sends[index].dest_index;
        debug_assert!(dest > source);
        let (head, tail) = buses.split_at_mut(dest);
        let bus = &mut head[source];
        let send = &mut bus.input_sends[index];
        let [left, right] =
            &mut tail[0].inserts[send.insert_index].inputs[send.input_index].incoming;
        send.delay.process_add(
            &bus.out_l[..frames],
            &bus.out_r[..frames],
            1.,
            &mut left[..frames],
            &mut right[..frames],
        );
    }
}

pub(super) fn reset(bus: &mut Bus) {
    bus.input_delay.reset();
    for send in &mut bus.input_sends {
        send.delay.reset();
    }
    for slot in &mut bus.inserts {
        for input in &mut slot.inputs {
            input.delay.reset();
            for channel in &mut input.incoming {
                channel.fill(0.);
            }
        }
        if let Some(sc) = &mut slot.sidechain {
            sc.delay.reset();
        }
        for output in &mut slot.outputs {
            output.catchup.reset();
            output.route_delay.reset();
            for channel in &mut output.aligned {
                channel.fill(0.);
            }
        }
    }
}
