//! Match the loader's processing length to the requested frames, reusing allocated capacity.
use vst3_host::audio::AudioBuffers;

pub(crate) fn set_frames(buffers: &mut AudioBuffers, frames: usize) -> crate::Result<()> {
    if frames == 0
        || buffers
            .inputs
            .iter()
            .chain(&buffers.outputs)
            .any(|b| frames > b.capacity())
    {
        return Err(crate::invalid(
            "VST3 block exceeds prepared buffer capacity",
        ));
    }
    for channel in buffers.inputs.iter_mut().chain(&mut buffers.outputs) {
        channel.resize(frames, 0.0);
    }
    buffers.block_size = frames;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_blocks_change_actual_channel_lengths_without_losing_capacity() {
        let mut buffers = AudioBuffers::new(2, 2, 128, 48000.0);
        let addresses: Vec<_> = buffers
            .inputs
            .iter()
            .chain(&buffers.outputs)
            .map(|b| b.as_ptr())
            .collect();
        for frames in [1, 128, 17, 111, 7, 128] {
            set_frames(&mut buffers, frames).unwrap();
            assert_eq!(buffers.block_size, frames);
            for (buffer, address) in buffers
                .inputs
                .iter()
                .chain(&buffers.outputs)
                .zip(&addresses)
            {
                assert_eq!(buffer.len(), frames);
                assert_eq!(buffer.as_ptr(), *address);
            }
        }
        assert!(set_frames(&mut buffers, 129).is_err());
        assert!(set_frames(&mut buffers, 0).is_err());
        assert_eq!(buffers.outputs[0].len(), 128);
    }
}
