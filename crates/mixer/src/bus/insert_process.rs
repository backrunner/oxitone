//! One serial insert chain, with preallocated auxiliary buses and sample-aligned sidechain inputs.
use super::Bus;
use oxitone_core::OxitoneError;
use oxitone_graph::{
    execution::{Execution, ProcessPosition},
    ProcessContext,
};

pub(super) fn process<E: Execution>(
    bus: &mut Bus,
    frames: usize,
    sample_rate: f64,
    position: &ProcessPosition,
) -> Result<(), OxitoneError> {
    let mut in_sum = true;
    for slot in &mut bus.inserts {
        let (input_l, input_r, output_l, output_r) = if in_sum {
            (&bus.sum_l, &bus.sum_r, &mut bus.work_l, &mut bus.work_r)
        } else {
            (&bus.work_l, &bus.work_r, &mut bus.sum_l, &mut bus.sum_r)
        };
        for route in &mut slot.inputs {
            route.stage(slot.instance.as_mut(), frames)?;
        }
        if let Some(sidechain) = &mut slot.sidechain {
            sidechain.stage(&bus.sc_l[..frames], &bus.sc_r[..frames]);
        }
        let sc = slot
            .sidechain
            .as_ref()
            .map(|sc| [&sc.aligned[0][..frames], &sc.aligned[1][..frames]]);
        slot.pending.with_events(|events| {
            let inputs = [&input_l[..frames], &input_r[..frames]];
            let mut outputs = [&mut output_l[..frames], &mut output_r[..frames]];
            let mut ctx = ProcessContext {
                frames,
                sample_rate,
                inputs: &inputs,
                outputs: &mut outputs,
                note_events: &[],
                parameter_events: events,
                sidechain: sc.as_ref().map(|sc| &sc[..]),
            };
            E::process(slot.instance.as_mut(), &mut ctx, position)
        })?;
        for route in &mut slot.outputs {
            route.capture(slot.instance.as_ref(), frames)?;
        }
        slot.dry_l[..frames].fill(0.);
        slot.dry_r[..frames].fill(0.);
        slot.dry_delay.process_add(
            &input_l[..frames],
            &input_r[..frames],
            1.,
            &mut slot.dry_l[..frames],
            &mut slot.dry_r[..frames],
        );
        for n in 0..frames {
            let smoothed = slot.mix.next_sample();
            let mix = if slot.bypass { 0. } else { smoothed };
            output_l[n] = output_l[n] * mix + slot.dry_l[n] * (1. - mix);
            output_r[n] = output_r[n] * mix + slot.dry_r[n] * (1. - mix);
            for route in &mut slot.outputs {
                route.wet[0][n] *= mix;
                route.wet[1][n] *= mix;
            }
        }
        for route in &mut slot.outputs {
            route.align(frames);
        }
        in_sum = !in_sum;
    }
    if !in_sum {
        bus.sum_l[..frames].copy_from_slice(&bus.work_l[..frames]);
        bus.sum_r[..frames].copy_from_slice(&bus.work_r[..frames]);
    }
    Ok(())
}
