//! Programmatic WAV/AIFF fixture builders shared by unit tests.

use oxitone_core::wire::{SampleFormat, SampleRef};

pub fn sine_interleaved(frames: usize, channels: usize, rate: u32, freq: f64) -> Vec<f32> {
    let mut out = Vec::with_capacity(frames * channels);
    for i in 0..frames {
        let s = (2.0 * std::f64::consts::PI * freq * i as f64 / rate as f64).sin() as f32 * 0.5;
        for _ in 0..channels {
            out.push(s);
        }
    }
    out
}

fn quantize(sample: f32, bits: u16, float: bool) -> Vec<u8> {
    let s = sample.clamp(-1.0, 1.0);
    match (float, bits) {
        (true, 32) => s.to_le_bytes().to_vec(),
        (true, 64) => (s as f64).to_le_bytes().to_vec(),
        (false, 8) => vec![(s * 127.0 + 128.0).round().clamp(0.0, 255.0) as u8],
        (false, 16) => ((s * 32767.0).round() as i16).to_le_bytes().to_vec(),
        (false, 24) => {
            let v = (s * 8_388_607.0).round() as i32;
            vec![v as u8, (v >> 8) as u8, (v >> 16) as u8]
        }
        (false, 32) => ((s * 2_147_483_647.0).round() as i32)
            .to_le_bytes()
            .to_vec(),
        _ => panic!("unsupported fixture format"),
    }
}

pub fn build_wav(
    channels: u16,
    sample_rate: u32,
    bits: u16,
    float: bool,
    samples: &[f32],
    loop_points: Option<(u32, u32)>,
) -> Vec<u8> {
    let tag: u16 = if float { 3 } else { 1 };
    let bytes_per = (bits / 8) as usize;
    let block_align = channels as usize * bytes_per;
    let mut data = Vec::with_capacity(samples.len() * bytes_per);
    for &s in samples {
        data.extend_from_slice(&quantize(s, bits, float));
    }
    let mut fmt = Vec::with_capacity(16);
    fmt.extend_from_slice(&tag.to_le_bytes());
    fmt.extend_from_slice(&channels.to_le_bytes());
    fmt.extend_from_slice(&sample_rate.to_le_bytes());
    fmt.extend_from_slice(&(sample_rate * block_align as u32).to_le_bytes());
    fmt.extend_from_slice(&(block_align as u16).to_le_bytes());
    fmt.extend_from_slice(&bits.to_le_bytes());

    let mut body = Vec::new();
    push_chunk(&mut body, b"fmt ", &fmt);
    if let Some((start, end)) = loop_points {
        let mut smpl = vec![0u8; 36];
        smpl[28..32].copy_from_slice(&1u32.to_le_bytes());
        smpl.extend_from_slice(&0u32.to_le_bytes());
        smpl.extend_from_slice(&0u32.to_le_bytes());
        smpl.extend_from_slice(&start.to_le_bytes());
        smpl.extend_from_slice(&end.to_le_bytes());
        smpl.extend_from_slice(&0u32.to_le_bytes());
        smpl.extend_from_slice(&0u32.to_le_bytes());
        push_chunk(&mut body, b"smpl", &smpl);
    }
    push_chunk(&mut body, b"data", &data);

    let mut out = Vec::with_capacity(body.len() + 12);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(&body);
    out
}

pub fn extended80(rate: f64) -> [u8; 10] {
    let exp = rate.log2().floor() as i32;
    let mut mantissa = (rate / 2f64.powi(exp) * 2f64.powi(63)).round() as u64;
    let mut biased = (16383 + exp) as u16;
    if mantissa == 0 {
        mantissa = 1u64 << 63;
        biased = 16383;
    }
    let mut out = [0u8; 10];
    out[0..2].copy_from_slice(&biased.to_be_bytes());
    out[2..10].copy_from_slice(&mantissa.to_be_bytes());
    out
}

pub fn build_aiff(channels: u16, sample_rate: u32, bits: u16, samples: &[f32]) -> Vec<u8> {
    let bytes_per = (bits / 8) as usize;
    let frames = samples.len() / channels as usize;
    let mut comm = Vec::new();
    comm.extend_from_slice(&channels.to_be_bytes());
    comm.extend_from_slice(&(frames as u32).to_be_bytes());
    comm.extend_from_slice(&bits.to_be_bytes());
    comm.extend_from_slice(&extended80(sample_rate as f64));

    let mut ssnd = Vec::new();
    ssnd.extend_from_slice(&[0u8; 8]);
    let scale = ((1u64 << (bits - 1)) - 1) as f32;
    for &s in samples {
        let v = (s.clamp(-1.0, 1.0) * scale).round() as i32;
        let be = v.to_be_bytes();
        ssnd.extend_from_slice(&be[4 - bytes_per..]);
    }

    let mut body = Vec::new();
    push_chunk_be(&mut body, b"COMM", &comm);
    push_chunk_be(&mut body, b"SSND", &ssnd);
    let mut out = Vec::with_capacity(body.len() + 12);
    out.extend_from_slice(b"FORM");
    out.extend_from_slice(&(body.len() as u32 + 4).to_be_bytes());
    out.extend_from_slice(b"AIFF");
    out.extend_from_slice(&body);
    out
}

fn push_chunk(out: &mut Vec<u8>, id: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(id);
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
    if body.len() % 2 == 1 {
        out.push(0);
    }
}

fn push_chunk_be(out: &mut Vec<u8>, id: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(id);
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(body);
    if body.len() % 2 == 1 {
        out.push(0);
    }
}

pub fn sample_ref_for(
    bytes: &[u8],
    format: SampleFormat,
    sample_rate: u32,
    channels: u8,
    frames: u64,
) -> SampleRef {
    SampleRef {
        id: "smp_test".to_owned(),
        asset_uri: "file:///fixtures/test-asset".to_owned(),
        sha256: crate::sha256_hex(bytes),
        format,
        sample_rate,
        channels,
        frames,
        edits: None,
        musical_length_beats: None,
    }
}
