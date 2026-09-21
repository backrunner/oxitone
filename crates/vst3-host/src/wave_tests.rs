use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "oxitone-vst3-wave-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn input(dir: &Directory, channels: u16, samples: &[f32]) -> std::path::PathBuf {
    let path = dir.0.join("input.wav");
    let mut writer = WavWriter::create(
        &path,
        WavSpec {
            channels,
            sample_rate: 48000,
            bits_per_sample: 32,
            sample_format: SampleFormat::Float,
        },
    )
    .unwrap();
    for sample in samples {
        writer.write_sample(*sample).unwrap();
    }
    writer.finalize().unwrap();
    path
}
#[test]
fn mono_is_duplicated_and_stereo_is_averaged_for_mono_input() {
    let dir = Directory::new();
    let path = input(&dir, 1, &[0.25, -0.5]);
    let pcm = Input::open(&path, 48000).unwrap();
    let mut channels = vec![vec![9.; 4]; 2];
    pcm.fill(&mut channels, 0, 4);
    assert_eq!(channels, vec![vec![0.25, -0.5, 0., 0.]; 2]);
    let path = input(&dir, 2, &[0.5, 0., -0.5, 0.5]);
    let pcm = Input::open(&path, 48000).unwrap();
    let mut channels = vec![vec![0.; 2]];
    pcm.fill(&mut channels, 0, 2);
    assert_eq!(channels[0], [0.25, 0.]);
}
#[test]
fn rejects_rate_mismatch_nonfinite_input_and_output_overwrite() {
    let dir = Directory::new();
    let path = input(&dir, 1, &[0.1]);
    assert!(Input::open(&path, 44100).is_err());
    assert!(Output::create(&path, 48000).is_err());
    let path = input(&dir, 1, &[f32::NAN]);
    assert!(Input::open(&path, 48000).is_err());
}
