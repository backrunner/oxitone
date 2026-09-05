//! Prepared-sample cache keyed by (content hash, edit spec hash, target
//! rate). Control thread only: a plain `Mutex<HashMap>` behind `Arc`, never
//! touched from the audio callback.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use oxitone_core::canonical::to_canonical_json;
use oxitone_core::wire::SampleRef;
use oxitone_core::OxitoneError;

use crate::edit::prepare;
use crate::types::{DecodedSample, PreparedSample};

/// Cache key: identical assets with identical edits at the same engine rate
/// share one prepared segment.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    /// Lowercase hex SHA-256 of the original asset bytes.
    pub content_sha256: String,
    /// Lowercase hex SHA-256 of the canonical JSON of the edit spec.
    pub edit_sha256: String,
    pub target_sample_rate: u32,
}

impl CacheKey {
    pub fn new(sample_ref: &SampleRef, target_sample_rate: u32) -> Result<Self, OxitoneError> {
        Ok(Self {
            content_sha256: sample_ref.sha256.to_ascii_lowercase(),
            edit_sha256: edit_spec_hash(sample_ref)?,
            target_sample_rate,
        })
    }
}

fn edit_spec_hash(sample_ref: &SampleRef) -> Result<String, OxitoneError> {
    let canonical = to_canonical_json(&sample_ref.edits)?;
    Ok(crate::sha256_hex(canonical.as_bytes()))
}

/// Memoized [`PreparedSample`] store for the compile/prepare phase.
#[derive(Default)]
pub struct SampleCache {
    inner: Mutex<HashMap<CacheKey, Arc<PreparedSample>>>,
}

impl SampleCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &CacheKey) -> Option<Arc<PreparedSample>> {
        self.inner
            .lock()
            .expect("sample cache poisoned")
            .get(key)
            .cloned()
    }

    pub fn insert(&self, key: CacheKey, sample: PreparedSample) -> Arc<PreparedSample> {
        let shared = Arc::new(sample);
        self.inner
            .lock()
            .expect("sample cache poisoned")
            .insert(key, shared.clone());
        shared
    }

    pub fn len(&self) -> usize {
        self.inner.lock().expect("sample cache poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&self) {
        self.inner.lock().expect("sample cache poisoned").clear();
    }
}

/// Cache-aware [`prepare`]: returns the shared prepared segment when the
/// (content, edits, rate) triple was prepared before.
pub fn prepare_cached(
    cache: &SampleCache,
    sample_ref: &SampleRef,
    decoded: &DecodedSample,
    target_sample_rate: u32,
) -> Result<Arc<PreparedSample>, OxitoneError> {
    let key = CacheKey::new(sample_ref, target_sample_rate)?;
    if let Some(hit) = cache.get(&key) {
        return Ok(hit);
    }
    let prepared = prepare(sample_ref, decoded, target_sample_rate)?;
    Ok(cache.insert(key, prepared))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{build_wav, sample_ref_for, sine_interleaved};
    use oxitone_core::wire::{SampleEditSpec, SampleFormat};

    fn decoded_fixture() -> (Vec<u8>, DecodedSample) {
        let bytes = build_wav(
            1,
            44100,
            16,
            false,
            &sine_interleaved(256, 1, 44100, 440.0),
            None,
        );
        let decoded = crate::decode_bytes(&bytes, SampleFormat::Wav).unwrap();
        (bytes, decoded)
    }

    #[test]
    fn key_changes_with_edit_spec() {
        let (bytes, _) = decoded_fixture();
        let plain = sample_ref_for(&bytes, SampleFormat::Wav, 44100, 1, 256);
        let mut trimmed = plain.clone();
        trimmed.edits = Some(SampleEditSpec {
            start_frame: Some(10),
            end_frame: None,
            level: None,
            tone: None,
            normalize: None,
            fade_in: None,
            fade_out: None,
            crossfade: None,
        });
        let a = CacheKey::new(&plain, 48000).unwrap();
        let b = CacheKey::new(&trimmed, 48000).unwrap();
        let c = CacheKey::new(&plain, 44100).unwrap();
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_eq!(a, CacheKey::new(&plain, 48000).unwrap());
    }

    #[test]
    fn prepare_cached_returns_shared_arc() {
        let (bytes, decoded) = decoded_fixture();
        let reference = sample_ref_for(&bytes, SampleFormat::Wav, 44100, 1, 256);
        let cache = SampleCache::new();
        assert!(cache.is_empty());
        let first = prepare_cached(&cache, &reference, &decoded, 44100).unwrap();
        let second = prepare_cached(&cache, &reference, &decoded, 44100).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(cache.len(), 1);
        cache.clear();
        assert!(cache.is_empty());
    }
}
