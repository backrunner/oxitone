//! Fixed-size context record used exclusively by the stream IO/helper threads.
use crate::transport_wire::Transport;
use std::io;

pub(crate) const BYTES: usize = 72;
fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid VST3 transport record")
}
pub(crate) fn encode(value: Option<Transport>, bytes: &mut [u8]) -> io::Result<()> {
    bytes.fill(0);
    let Some(value) = value else { return Ok(()) };
    if !value.valid() || bytes.len() != BYTES {
        return Err(invalid());
    }
    let flags = 1u64 | ((value.playing as u64) << 1) | ((value.cycle.is_some() as u64) << 2);
    let words = [
        flags,
        value.project_frame,
        value.continuous_frame,
        value.project_beat.to_bits(),
        value.bar_beat.to_bits(),
        value.tempo.to_bits(),
        value.time_signature[0] as u64 | ((value.time_signature[1] as u64) << 32),
        value.cycle.unwrap_or([0.0; 2])[0].to_bits(),
        value.cycle.unwrap_or([0.0; 2])[1].to_bits(),
    ];
    for (target, word) in bytes.chunks_exact_mut(8).zip(words) {
        target.copy_from_slice(&word.to_le_bytes());
    }
    Ok(())
}
pub(crate) fn decode(bytes: &[u8]) -> io::Result<Option<Transport>> {
    if bytes.len() != BYTES {
        return Err(invalid());
    }
    let word = |n: usize| u64::from_le_bytes(bytes[n * 8..n * 8 + 8].try_into().unwrap());
    let flags = word(0);
    if flags == 0 {
        return if bytes.iter().all(|b| *b == 0) {
            Ok(None)
        } else {
            Err(invalid())
        };
    }
    if flags & !7 != 0 || flags & 1 == 0 || (flags & 4 == 0 && (word(7) != 0 || word(8) != 0)) {
        return Err(invalid());
    }
    let value = Transport {
        project_frame: word(1),
        continuous_frame: word(2),
        project_beat: f64::from_bits(word(3)),
        bar_beat: f64::from_bits(word(4)),
        tempo: f64::from_bits(word(5)),
        time_signature: [word(6) as i32, (word(6) >> 32) as i32],
        playing: flags & 2 != 0,
        cycle: (flags & 4 != 0).then(|| [f64::from_bits(word(7)), f64::from_bits(word(8))]),
    };
    if !value.valid() {
        return Err(invalid());
    }
    Ok(Some(value))
}
