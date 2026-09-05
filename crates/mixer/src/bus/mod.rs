//! Mixer bus engine (02-domain-spec.md §Mixer 与 routing, 03-audio-runtime
//! §DSP 处理顺序). Buses are processed in `build_mixer_routing` topological
//! order: deterministic input summation → insert chain → pre-fader send
//! taps → fader (level/balance, mute/solo) → post-fader send taps →
//! `masterSendRatio` → meter. Sidechain sends feed only the destination
//! insert chain's detector inputs and never enter the audio sum; every send
//! edge (audio and detector) carries its PDC compensation delay. Summation
//! order is fixed by the topological order and edge sort order, so output
//! is bitwise deterministic for the same inputs.

use std::collections::BTreeMap;

mod process;

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::{EntityId, MixerChannelSpec};
use oxitone_dsp::gain_pan::OnePoleSmoother;
use oxitone_graph::abi::{HostContext, PluginInstance};
use oxitone_graph::registry::PluginRegistry;
use oxitone_graph::topology::{build_mixer_routing, MASTER_MIXER_CHANNEL_ID};

use crate::meter::{BusMeter, TruePeakMeter};
use crate::pdc::{plan_pdc, DelayLine, PdcPlan};

/// Max block-boundary parameter events applied per insert per block.
const MAX_PENDING_EVENTS: usize = 256;

/// Stereo input of one source channel summed into a bus for one block.
/// The caller (compiler) fixes the slice order — it is part of the
/// deterministic summation order.
pub struct ChannelInput<'a> {
    pub bus_id: &'a str,
    pub left: &'a [f32],
    pub right: &'a [f32],
}

struct InsertSlot {
    instance: Box<dyn PluginInstance>,
    accepts_sidechain: bool,
    param_ids: Vec<String>,
    pending: Vec<(usize, f64)>,
    mix: OnePoleSmoother,
    bypass: bool,
    dry_delay: DelayLine,
    dry_l: Vec<f32>,
    dry_r: Vec<f32>,
}

struct SendSlot {
    dest_index: usize,
    sidechain: bool,
    pre_fader: bool,
    ratio: f32,
    delay: DelayLine,
}

struct Bus {
    id: EntityId,
    is_master: bool,
    inserts: Vec<InsertSlot>,
    level: OnePoleSmoother,
    balance: f32,
    mute: bool,
    solo: bool,
    master_send_ratio: f32,
    sends: Vec<SendSlot>,
    master_delay: DelayLine,
    sum_l: Vec<f32>,
    sum_r: Vec<f32>,
    sc_l: Vec<f32>,
    sc_r: Vec<f32>,
    work_l: Vec<f32>,
    work_r: Vec<f32>,
    out_l: Vec<f32>,
    out_r: Vec<f32>,
    meter: BusMeter,
}

/// Compiled mixer: owns bus state, insert instances and PDC delays.
pub struct MixerEngine {
    sample_rate: f64,
    max_block: usize,
    buses: Vec<Bus>,
    index: BTreeMap<EntityId, usize>,
    respect_solo: bool,
    pdc: PdcPlan,
    true_peak: TruePeakMeter,
    /// Optional per-bus tap of the delayed, ratio-scaled contribution each
    /// bus delivers to Master (post-fader, post-PDC). Disabled by default;
    /// enabling it adds one extra mix pass per non-master bus per block.
    /// Used by offline stem export so `sum(stems) == master` holds
    /// sample-aligned. Purely additive: `process_block` output is identical
    /// whether or not taps are enabled.
    stem_taps: Option<Vec<[Vec<f32>; 2]>>,
    stem_block: Option<Vec<[Vec<f32>; 2]>>,
}

impl MixerEngine {
    /// Compile from validated mixer channel specs. Insert plugins are
    /// created through the registry (ABI v1), prepared, and seeded with the
    /// `EffectRef` parameter values. Control thread; allocates.
    pub fn build(
        sample_rate: f64,
        max_block_size: u32,
        channels: &[MixerChannelSpec],
        registry: &PluginRegistry,
    ) -> Result<Self, OxitoneError> {
        let routing = build_mixer_routing(channels)?;
        let specs: BTreeMap<&str, &MixerChannelSpec> =
            channels.iter().map(|c| (c.id.as_str(), c)).collect();
        let max_block = max_block_size as usize;
        let host = HostContext {
            sample_rate,
            max_block_size,
        };

        let mut insert_latency: BTreeMap<EntityId, u64> = BTreeMap::new();
        let mut insert_chains: BTreeMap<EntityId, Vec<InsertSlot>> = BTreeMap::new();
        for id in &routing.order {
            let mut slots = Vec::new();
            let mut latency = 0u64;
            if let Some(spec) = specs.get(id.as_str()) {
                for effect in &spec.inserts {
                    let plugin = registry
                        .lookup(&effect.plugin_id, &effect.plugin_version)
                        .ok_or_else(|| {
                            OxitoneError::with_path(
                                codes::INVALID_PROJECT,
                                format!(
                                    "unknown plugin {}@{}",
                                    effect.plugin_id, effect.plugin_version
                                ),
                                format!("$.mixerChannels.{id}.inserts"),
                            )
                        })?;
                    let descriptor = plugin.descriptor();
                    let mut instance = plugin.create(&host);
                    instance.prepare(sample_rate, max_block_size);
                    let param_ids: Vec<String> =
                        descriptor.parameters.iter().map(|p| p.id.clone()).collect();
                    let mut pending = Vec::with_capacity(MAX_PENDING_EVENTS);
                    for (param_id, value) in &effect.parameters {
                        let index =
                            param_ids
                                .iter()
                                .position(|id| id == param_id)
                                .ok_or_else(|| {
                                    OxitoneError::with_path(
                                        codes::INVALID_PROJECT,
                                        format!(
                                            "unknown parameter {param_id:?} on {}",
                                            effect.plugin_id
                                        ),
                                        format!("$.mixerChannels.{id}.inserts"),
                                    )
                                })?;
                        pending.push((index, *value));
                    }
                    latency += instance.latency_frames();
                    let mut mix = OnePoleSmoother::new(sample_rate, 20.0);
                    mix.snap(effect.mix.unwrap_or(1.0) as f32);
                    let mut dry_delay =
                        DelayLine::new(instance.latency_frames() as usize, max_block);
                    dry_delay.set_delay(instance.latency_frames() as usize);
                    slots.push(InsertSlot {
                        mix,
                        bypass: effect.bypass.unwrap_or(false),
                        dry_delay,
                        dry_l: vec![0.0; max_block],
                        dry_r: vec![0.0; max_block],
                        instance,
                        accepts_sidechain: descriptor.capabilities.sidechain_input,
                        param_ids,
                        pending,
                    });
                }
            }
            insert_latency.insert(id.clone(), latency);
            insert_chains.insert(id.clone(), slots);
        }

        let pdc = plan_pdc(&routing, &insert_latency);
        let max_edge_delay = pdc.edge_delays.values().copied().max().unwrap_or(0) as usize;

        let mut buses = Vec::new();
        let mut index = BTreeMap::new();
        for (position, id) in routing.order.iter().enumerate() {
            let is_master = id == MASTER_MIXER_CHANNEL_ID;
            let spec = specs.get(id.as_str());
            let mut level = OnePoleSmoother::new(sample_rate, 20.0);
            level.snap(spec.map(|s| s.level as f32).unwrap_or(1.0));
            let sends = spec
                .map(|s| {
                    s.sends
                        .iter()
                        .filter_map(|send| {
                            let dest = routing
                                .order
                                .iter()
                                .position(|d| d == &send.destination_id)?;
                            let sidechain = send.sidechain.unwrap_or(false);
                            let mut delay = DelayLine::new(max_edge_delay, max_block);
                            delay.set_delay(
                                pdc.edge_delays
                                    .get(&(id.clone(), send.destination_id.clone(), sidechain))
                                    .copied()
                                    .unwrap_or(0) as usize,
                            );
                            Some(SendSlot {
                                dest_index: dest,
                                sidechain,
                                pre_fader: send.pre_fader.unwrap_or(false),
                                ratio: send.ratio as f32,
                                delay,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            let mut master_delay = DelayLine::new(max_edge_delay, max_block);
            master_delay.set_delay(
                pdc.edge_delays
                    .get(&(id.clone(), MASTER_MIXER_CHANNEL_ID.to_string(), false))
                    .copied()
                    .unwrap_or(0) as usize,
            );
            index.insert(id.clone(), position);
            buses.push(Bus {
                id: id.clone(),
                is_master,
                inserts: insert_chains.remove(id).unwrap_or_default(),
                level,
                balance: spec.map(|s| s.balance as f32).unwrap_or(0.0),
                mute: spec.and_then(|s| s.mute).unwrap_or(false),
                solo: spec.and_then(|s| s.solo).unwrap_or(false),
                master_send_ratio: spec.and_then(|s| s.master_send_ratio).unwrap_or(1.0) as f32,
                sends,
                master_delay,
                sum_l: vec![0.0; max_block],
                sum_r: vec![0.0; max_block],
                sc_l: vec![0.0; max_block],
                sc_r: vec![0.0; max_block],
                work_l: vec![0.0; max_block],
                work_r: vec![0.0; max_block],
                out_l: vec![0.0; max_block],
                out_r: vec![0.0; max_block],
                meter: BusMeter::new(),
            });
        }

        Ok(Self {
            sample_rate,
            max_block,
            buses,
            index,
            respect_solo: false,
            pdc,
            true_peak: TruePeakMeter::new(max_block),
            stem_taps: None,
            stem_block: None,
        })
    }

    /// Total internal graph latency in frames (Master output latency).
    pub fn graph_latency_frames(&self) -> u64 {
        self.pdc.graph_latency_frames
    }

    pub fn pdc_plan(&self) -> &PdcPlan {
        &self.pdc
    }

    /// Bus IDs in processing (topological) order.
    pub fn bus_order(&self) -> Vec<EntityId> {
        self.buses.iter().map(|b| b.id.clone()).collect()
    }

    /// Solo is a monitoring policy (02-domain-spec.md); exports leave this
    /// off unless render options request `respectSolo`.
    pub fn set_respect_solo(&mut self, respect: bool) {
        self.respect_solo = respect;
    }

    fn bus_mut(&mut self, bus_id: &str) -> Result<&mut Bus, OxitoneError> {
        let index = self.index.get(bus_id).copied().ok_or_else(|| {
            OxitoneError::new(
                codes::INVALID_PROJECT,
                format!("unknown mixer bus {bus_id:?}"),
            )
        })?;
        Ok(&mut self.buses[index])
    }

    pub fn set_level(&mut self, bus_id: &str, value: f64) -> Result<(), OxitoneError> {
        self.bus_mut(bus_id)?.level.set_target(value as f32);
        Ok(())
    }

    pub fn set_balance(&mut self, bus_id: &str, value: f64) -> Result<(), OxitoneError> {
        self.bus_mut(bus_id)?.balance = value as f32;
        Ok(())
    }

    pub fn set_mute(&mut self, bus_id: &str, mute: bool) -> Result<(), OxitoneError> {
        self.bus_mut(bus_id)?.mute = mute;
        Ok(())
    }

    pub fn set_solo(&mut self, bus_id: &str, solo: bool) -> Result<(), OxitoneError> {
        self.bus_mut(bus_id)?.solo = solo;
        Ok(())
    }

    pub fn set_master_send_ratio(&mut self, bus_id: &str, ratio: f64) -> Result<(), OxitoneError> {
        self.bus_mut(bus_id)?.master_send_ratio = ratio as f32;
        Ok(())
    }

    pub fn set_send_ratio(
        &mut self,
        bus_id: &str,
        destination_id: &str,
        ratio: f64,
    ) -> Result<(), OxitoneError> {
        let dest = self.index.get(destination_id).copied().ok_or_else(|| {
            OxitoneError::new(
                codes::INVALID_PROJECT,
                format!("unknown mixer bus {destination_id:?}"),
            )
        })?;
        let bus = self.bus_mut(bus_id)?;
        for slot in &mut bus.sends {
            if slot.dest_index == dest {
                slot.ratio = ratio as f32;
                return Ok(());
            }
        }
        Err(OxitoneError::new(
            codes::INVALID_PROJECT,
            format!("no send from {bus_id:?} to {destination_id:?}"),
        ))
    }

    /// Queue a block-boundary parameter event for one insert. Control
    /// thread. Unknown parameter IDs are rejected; when the pending queue
    /// is full the latest value for the same parameter wins.
    pub fn set_insert_parameter(
        &mut self,
        bus_id: &str,
        insert: usize,
        parameter_id: &str,
        value: f64,
    ) -> Result<(), OxitoneError> {
        let bus = self.bus_mut(bus_id)?;
        let slot = bus.inserts.get_mut(insert).ok_or_else(|| {
            OxitoneError::new(
                codes::INVALID_PROJECT,
                format!("no insert #{insert} on bus {bus_id:?}"),
            )
        })?;
        let index = slot
            .param_ids
            .iter()
            .position(|id| id == parameter_id)
            .ok_or_else(|| {
                OxitoneError::new(
                    codes::INVALID_PROJECT,
                    format!("unknown parameter {parameter_id:?} on bus {bus_id:?}"),
                )
            })?;
        if let Some(entry) = slot.pending.iter_mut().find(|(i, _)| *i == index) {
            entry.1 = value;
        } else if slot.pending.len() < MAX_PENDING_EVENTS {
            slot.pending.push((index, value));
        }
        Ok(())
    }

    pub fn meter(&self, bus_id: &str) -> Option<&BusMeter> {
        self.index.get(bus_id).map(|&i| &self.buses[i].meter)
    }

    /// Enable/disable per-bus stem taps of the Master-route contribution.
    /// Control thread; allocates on first enable. Tap buffers are indexed by
    /// bus position in [`MixerEngine::bus_order`] (Master's slot stays zero).
    pub fn enable_stem_taps(&mut self, enabled: bool) {
        if enabled {
            self.stem_block.get_or_insert_with(|| {
                vec![[vec![0.0; self.max_block], vec![0.0; self.max_block]]; self.buses.len()]
            });
            let taps = self.stem_taps.get_or_insert_with(|| {
                vec![[vec![0.0; self.max_block], vec![0.0; self.max_block]]; self.buses.len()]
            });
            for tap in taps.iter_mut() {
                tap[0].fill(0.0);
                tap[1].fill(0.0);
            }
        } else {
            self.stem_taps = None;
            self.stem_block = None;
        }
    }

    pub fn capture_stem_segment(&mut self, offset: usize, frames: usize) {
        if let (Some(taps), Some(block)) = (&self.stem_taps, &mut self.stem_block) {
            for (src, dst) in taps.iter().zip(block.iter_mut()) {
                for c in 0..2 {
                    dst[c][offset..offset + frames].copy_from_slice(&src[c][..frames]);
                }
            }
        }
    }

    /// Last block's tapped Master-route contribution of `bus_id`
    /// (post-fader, `masterSendRatio` applied, PDC delay applied). Sum of all
    /// taps equals the Master bus input for the same block. `None` when stem
    /// taps are disabled or the bus is unknown/Master.
    pub fn stem_output(&self, bus_id: &str) -> Option<(&[f32], &[f32])> {
        let taps = self.stem_block.as_ref()?;
        let &index = self.index.get(bus_id)?;
        if self.buses[index].is_master {
            return None;
        }
        Some((&taps[index][0], &taps[index][1]))
    }

    /// Master-only 4x oversampled true-peak estimate.
    pub fn master_true_peak(&self) -> &TruePeakMeter {
        &self.true_peak
    }

    /// Control thread: clear insert state, compensation delays and meters.
    pub fn reset(&mut self) {
        for bus in &mut self.buses {
            for slot in &mut bus.inserts {
                slot.instance.reset();
                slot.dry_delay.reset();
            }
            for slot in &mut bus.sends {
                slot.delay.reset();
            }
            bus.master_delay.reset();
            bus.meter.reset();
        }
        if let Some(taps) = &mut self.stem_taps {
            for tap in taps.iter_mut() {
                tap[0].fill(0.0);
                tap[1].fill(0.0);
            }
        }
        self.true_peak.reset();
    }
}
