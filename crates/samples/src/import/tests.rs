use super::*;
use crate::{
    decode_bytes,
    fixtures::{build_aiff, build_wav},
    sha256_hex,
};
use oxitone_core::codes;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    directory: PathBuf,
    request: CacheSampleRequest,
}
impl Fixture {
    fn new(bytes: &[u8]) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "oxitone-cache-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("source 音频.dat");
        fs::write(&path, bytes).unwrap();
        let request = CacheSampleRequest {
            protocol_version: PROTOCOL_VERSION.into(),
            path: path.display().to_string(),
            cache_dir: directory.join("assets").display().to_string(),
        };
        Self { directory, request }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn caches_lossless_float_pcm_and_loop_with_correct_riff_sizes() {
    for channels in [1, 2, 6] {
        let bytes = build_wav(
            channels,
            24000,
            24,
            false,
            &vec![0.75; 128 * channels as usize],
            Some((8, 128)),
        );
        let fixture = Fixture::new(&bytes);
        let info = cache_sample(&fixture.request).unwrap();
        assert_eq!(info.format, SampleFormat::Wav);
        assert_eq!(info.provenance.source_sha256, sha256_hex(&bytes));
        assert_eq!(info.provenance.source_channels, channels as u8);
        assert_eq!(info.provenance.source_bit_depth, Some(24));
        let cached = fs::read(&info.path).unwrap();
        assert_eq!(info.sha256, sha256_hex(&cached));
        assert!(info.path.ends_with(&format!("{}.wav", info.sha256)));
        assert_eq!(
            u32::from_le_bytes(cached[4..8].try_into().unwrap()) as usize,
            cached.len() - 8
        );
        let expected = decode_bytes(&bytes, SampleFormat::Wav).unwrap();
        let decoded = decode_bytes(&cached, SampleFormat::Wav).unwrap();
        assert_eq!(decoded.channels, expected.channels);
        assert_eq!(decoded.loop_points, expected.loop_points);
        assert_eq!(decoded.loop_points.unwrap().end_frame, 128);
        assert_eq!(decoded.sample_rate, 24000);
        assert_eq!(fs::read(&fixture.request.path).unwrap(), bytes);
        let modified = fs::metadata(&info.path).unwrap().modified().unwrap();
        assert_eq!(cache_sample(&fixture.request).unwrap(), info);
        assert_eq!(
            fs::metadata(&info.path).unwrap().modified().unwrap(),
            modified
        );
        assert_eq!(fs::read_dir(&fixture.request.cache_dir).unwrap().count(), 1);
    }
}

#[test]
fn imports_aiff_and_concurrent_publishers_converge() {
    let bytes = build_aiff(2, 44100, 16, &[0.5; 512]);
    let fixture = Fixture::new(&bytes);
    let results = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| scope.spawn(|| cache_sample(&fixture.request).unwrap()))
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert!(results.iter().all(|result| result == &results[0]));
    assert_eq!(results[0].provenance.source_format, SampleFormat::Aiff);
    assert_eq!(results[0].provenance.decoder, "oxitone-aiff-v1");
    assert_eq!(fs::read_dir(&fixture.request.cache_dir).unwrap().count(), 1);
}

#[test]
fn rejects_corruption_symlinks_and_invalid_requests_without_overwriting() {
    let mut fixture = Fixture::new(&build_wav(1, 48000, 16, false, &[0.25; 128], None));
    let info = cache_sample(&fixture.request).unwrap();
    fs::write(&info.path, b"corrupt").unwrap();
    let error = cache_sample(&fixture.request).unwrap_err();
    assert_eq!(error.code, codes::ASSET_UNAVAILABLE);
    assert_eq!(error.path.as_ref(), Some(&info.path));
    assert_eq!(fs::read(&info.path).unwrap(), b"corrupt");
    #[cfg(unix)]
    {
        fs::remove_file(&info.path).unwrap();
        std::os::unix::fs::symlink(&fixture.request.path, &info.path).unwrap();
        assert_eq!(
            cache_sample(&fixture.request).unwrap_err().code,
            codes::ASSET_UNAVAILABLE
        );
        assert!(fs::symlink_metadata(&info.path)
            .unwrap()
            .file_type()
            .is_symlink());
    }
    assert_eq!(fs::read_dir(&fixture.request.cache_dir).unwrap().count(), 1);
    let request = &mut fixture.request;
    request.protocol_version = "99.0".into();
    assert_eq!(
        cache_sample(&request).unwrap_err().code,
        codes::PROTOCOL_VERSION_UNSUPPORTED
    );
    request.protocol_version = PROTOCOL_VERSION.into();
    for path in ["", "bad\0path"] {
        request.cache_dir = path.into();
        assert_eq!(
            cache_sample(&request).unwrap_err().code,
            codes::INVALID_PROJECT
        );
    }
}

#[test]
fn rejects_nonfinite_pcm_before_creating_cache() {
    let mut bytes = build_wav(1, 48000, 32, true, &[0.25; 128], None);
    bytes[44..48].copy_from_slice(&f32::NAN.to_le_bytes());
    let fixture = Fixture::new(&bytes);
    assert_eq!(
        cache_sample(&fixture.request).unwrap_err().code,
        codes::SAMPLE_FORMAT_UNSUPPORTED
    );
    assert!(!Path::new(&fixture.request.cache_dir).exists());
}
