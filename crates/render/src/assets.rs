//! Sample asset store: resolves `SampleRef.assetUri` against an optional
//! base directory, decodes and edit-bakes through `oxitone-samples`, and
//! memoizes prepared segments in a `SampleCache`. Implements both provider
//! traits used at compile time: `oxitone_graph::PlanSampleProvider`
//! (sample-clip plans) and `oxitone_instruments::SampleProvider`
//! (Sampler/Slicer resources). Control thread only.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::{ProjectSnapshot, SampleRef};
use oxitone_graph::compile::PlanSampleProvider;
use oxitone_samples::{load_asset, prepare_cached, PreparedSample, SampleCache};

/// Control-thread sample store shared by the plan compiler and instrument
/// creation.
pub struct SampleStore {
    base_dir: Option<PathBuf>,
    cache: SampleCache,
    target_rate: Cell<u32>,
    prepared: RefCell<BTreeMap<String, Arc<PreparedSample>>>,
}

impl SampleStore {
    /// `base_dir` resolves relative asset URIs; absolute URIs are used
    /// as-is. Without a base dir, relative URIs resolve against the process
    /// working directory.
    pub fn new(base_dir: Option<PathBuf>) -> Self {
        Self {
            base_dir,
            cache: SampleCache::new(),
            target_rate: Cell::new(48_000),
            prepared: RefCell::new(BTreeMap::new()),
        }
    }

    /// In-memory store for tests/renders that inject prepared samples
    /// directly; asset URIs are never read.
    pub fn from_prepared(samples: Vec<(String, Arc<PreparedSample>)>, sample_rate: u32) -> Self {
        let store = Self::new(None);
        store.target_rate.set(sample_rate);
        for (id, sample) in samples {
            store.prepared.borrow_mut().insert(id, sample);
        }
        store
    }

    /// Engine sample rate prepared segments are converted to. Set by
    /// `RenderGraph::compile` before plan compilation.
    pub fn set_target_rate(&self, sample_rate: u32) {
        self.target_rate.set(sample_rate);
    }

    /// Eagerly prepare every sample referenced by the snapshot so both
    /// provider traits hit the memo table. Failures are `AssetUnavailable`.
    pub fn prepare_all(
        &self,
        snapshot: &ProjectSnapshot,
        sample_rate: u32,
    ) -> Result<(), OxitoneError> {
        self.set_target_rate(sample_rate);
        for sample in &snapshot.samples {
            self.prepared_sample(sample)?;
        }
        Ok(())
    }

    fn resolve(&self, asset_uri: &str) -> PathBuf {
        let path = Path::new(asset_uri);
        if path.is_absolute() {
            return path.to_path_buf();
        }
        match &self.base_dir {
            Some(base) => base.join(path),
            None => path.to_path_buf(),
        }
    }
}

impl PlanSampleProvider for SampleStore {
    fn prepared_sample(&self, sample: &SampleRef) -> Result<Arc<PreparedSample>, OxitoneError> {
        if let Some(hit) = self.prepared.borrow().get(&sample.id) {
            return Ok(hit.clone());
        }
        if self.target_rate.get() == 0 {
            return Err(OxitoneError::new(
                codes::INVALID_PROJECT,
                "sample store target rate is not set",
            ));
        }
        let path = self.resolve(&sample.asset_uri);
        let decoded = load_asset(&path, sample)?;
        let prepared = prepare_cached(&self.cache, sample, &decoded, self.target_rate.get())?;
        self.prepared
            .borrow_mut()
            .insert(sample.id.clone(), prepared.clone());
        Ok(prepared)
    }
}

impl oxitone_instruments::SampleProvider for SampleStore {
    fn prepared_sample(&self, sample_id: &str) -> Option<Arc<PreparedSample>> {
        self.prepared.borrow().get(sample_id).cloned()
    }
}

impl oxitone_mixer::effects::convolver::ImpulseProvider for SampleStore {
    fn impulse(
        &self,
        sample_id: &str,
    ) -> Result<oxitone_mixer::effects::convolver::Impulse, OxitoneError> {
        let samples = self.prepared.borrow();
        let sample = samples.get(sample_id).ok_or_else(|| {
            OxitoneError::with_path(
                codes::ASSET_UNAVAILABLE,
                "impulse sample is unavailable",
                "effect.resources.impulse",
            )
        })?;
        let first = sample
            .channels
            .first()
            .ok_or_else(|| OxitoneError::new(codes::INVALID_PROJECT, "impulse has no channels"))?;
        if first.is_empty() || first.len() > oxitone_mixer::effects::convolver::MAX_IMPULSE_FRAMES {
            return Err(OxitoneError::with_path(
                codes::INVALID_PROJECT,
                "impulse must contain 1..262144 resampled frames",
                "effect.resources.impulse",
            ));
        }
        Ok(oxitone_mixer::effects::convolver::Impulse {
            channels: [
                first.clone(),
                sample.channels.get(1).unwrap_or(first).clone(),
            ],
            sample_rate: sample.sample_rate as f64,
        })
    }
}
