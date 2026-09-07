//! Resolve Slicer tempo following off-thread; runtime keeps only scalars/indices.
use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::InstrumentRef;
use oxitone_instruments::SampleProvider;
use oxitone_transport::CompiledTempoMap;

use crate::assets::SampleStore;

pub struct SlicerTempo {
    pub parameter_index: usize,
    factor_per_bpm: f64,
}

impl SlicerTempo {
    pub fn resolve(
        reference: &InstrumentRef,
        samples: &SampleStore,
        tempo: &CompiledTempoMap,
        parameter_ids: &[String],
    ) -> Result<Option<Self>, OxitoneError> {
        if reference.plugin_id != oxitone_instruments::SLICER_PLUGIN_ID {
            return Ok(None);
        }
        let Some(state) = &reference.state else {
            return Ok(None);
        };
        if state.get("tempoSync").and_then(|v| v.as_str()) != Some("repitch") {
            return Ok(None);
        }
        // The graph and configured instance already validated/resolved this state.
        let sample = samples
            .prepared_sample(state["sampleId"].as_str().unwrap())
            .expect("configured Slicer has a prepared sample");
        let seconds = sample.channels[0].len() as f64 / f64::from(sample.sample_rate);
        let native_bpm = sample
            .musical_length_beats
            .map(|beats| beats.to_f64() * 60.0 / seconds)
            .unwrap_or_else(|| tempo.bpm_at_frame(0));
        let factor_per_bpm = native_bpm.recip();
        let (lo, hi) = tempo.bpm_range(0.0, f64::MAX);
        if !factor_per_bpm.is_finite()
            || lo * factor_per_bpm < 0.25 - 1e-12
            || hi * factor_per_bpm > 4.0 + 1e-12
        {
            return Err(OxitoneError::with_path(codes::INVALID_PROJECT,
                "Slicer repitch tempo factor must stay within 0.25..=4; check musicalLengthBeats and tempo",
                "$.instrument.state.tempoSync"));
        }
        Ok(Some(Self {
            parameter_index: parameter_ids
                .iter()
                .position(|p| p == "tempoFactor")
                .expect("Slicer descriptor declares tempoFactor"),
            factor_per_bpm,
        }))
    }

    pub fn factor(&self, bpm: f64) -> f64 {
        (bpm * self.factor_per_bpm).clamp(0.25, 4.0)
    }
}
