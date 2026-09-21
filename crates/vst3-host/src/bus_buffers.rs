//! Convert each stereo wire slot to its corresponding VST3 bus without changing bus indices.
use crate::{stream_wire::Block, Result};
use vst3_host::audio::BusAudioBuffers;

pub(crate) fn input(buffers: &mut BusAudioBuffers, block: &Block) -> Result<()> {
    let frames = block.frames;
    if block.bus_count != buffers.inputs.len().max(1)
        || frames == 0
        || buffers
            .inputs
            .iter()
            .chain(&buffers.outputs)
            .flat_map(|bus| &bus.channels)
            .any(|c| frames > c.capacity())
    {
        return Err(crate::invalid("VST3 bus packet exceeds prepared shape"));
    }
    for bus in buffers.inputs.iter_mut().chain(&mut buffers.outputs) {
        for channel in &mut bus.channels {
            channel.resize(frames, 0.);
            channel.fill(0.);
        }
    }
    buffers.block_size = frames;
    for (index, samples) in block.audio[..block.bus_count * 2 * frames]
        .chunks_exact(2 * frames)
        .enumerate()
    {
        let Some(bus) = buffers.inputs.get_mut(index).filter(|b| b.active) else {
            if samples.iter().any(|v| *v != 0.) {
                return Err(crate::invalid("inactive VST3 input must be silent"));
            }
            continue;
        };
        for i in 0..frames {
            if bus.channels.len() == 1 {
                bus.channels[0][i] = samples[i] * 0.5 + samples[frames + i] * 0.5;
            } else {
                bus.channels[0][i] = samples[i];
                bus.channels[1][i] = samples[frames + i];
            }
        }
    }
    Ok(())
}

pub(crate) fn output(buffers: &BusAudioBuffers, block: &mut Block) -> Result<()> {
    let frames = block.frames;
    block.bus_count = buffers.outputs.len();
    for (bus, samples) in buffers
        .outputs
        .iter()
        .zip(block.audio.chunks_exact_mut(2 * frames))
    {
        for i in 0..frames {
            let (left, right) = if bus.active {
                (bus.channels[0][i], bus.channels[bus.channels.len() - 1][i])
            } else {
                (0., 0.)
            };
            if !left.is_finite() || !right.is_finite() {
                return Err(crate::invalid("non-finite VST3 bus output"));
            }
            samples[i] = left;
            samples[frames + i] = right;
        }
    }
    Ok(())
}
