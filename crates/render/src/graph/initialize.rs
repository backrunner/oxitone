//! Control-thread graph initialization and preallocated execution storage.
use super::*;

impl RenderGraph {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        plan: RenderPlan,
        channels: Vec<ChannelNode>,
        channel_tracks: BTreeMap<String, Vec<String>>,
        clips: Vec<ClipNode>,
        mixer: MixerEngine,
        mixer_beat_params: Vec<MixerBeatParam>,
        mixer_sends: BTreeSet<(String, String)>,
        limiter: Option<Box<dyn PluginInstance>>,
        limiter_latency: u64,
        channel_latency_max: u64,
        metronome: Option<Metronome>,
        respect_solo: bool,
        controls: crate::plugin_controls::ControlGraph,
    ) -> Self {
        let block_size = plan.block_size as usize;
        let sample_rate = f64::from(plan.sample_rate);
        let channel_index = channels
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id.clone(), i))
            .collect();
        let graph_latency = mixer.graph_latency_frames() + channel_latency_max + limiter_latency;
        let dispatcher = Dispatcher::new(
            sample_rate as u32,
            plan.scheduler.events_in_range(0, u64::MAX).len(),
        );
        let isolated = plan.midi_routing.is_some()
            || mixer.requires_isolation()
            || channels.iter().any(|c| {
                c.instrument.requires_isolation()
                    || c.inserts.iter().any(|i| i.instance.requires_isolation())
            });
        let mut graph = Self {
            midi_clip_l: vec![
                0.;
                if plan.midi_routing.is_some() {
                    block_size
                } else {
                    0
                }
            ],
            midi_clip_r: vec![
                0.;
                if plan.midi_routing.is_some() {
                    block_size
                } else {
                    0
                }
            ],
            controls,
            isolated,
            continuous_frame: 0,
            plan,
            sample_rate,
            block_size,
            channels,
            channel_index,
            channel_tracks,
            clips,
            mixer,
            mixer_beat_params,
            mixer_sends,
            preview: None,
            effect_targets: Default::default(),
            limiter,
            metronome,
            transport: Transport::new(),
            dispatcher,
            rt_bindings: Vec::new(),
            param_queue: VecDeque::with_capacity(4096),
            respect_solo,
            eval_ctx: EvalContext::default(),
            faulted: false,
            graph_latency,
            master_l: vec![0.0; block_size],
            master_r: vec![0.0; block_size],
            limited_l: vec![0.0; block_size],
            limited_r: vec![0.0; block_size],
            metro_block_l: vec![0.0; block_size],
            metro_block_r: vec![0.0; block_size],
            metro_l: vec![0.0; block_size],
            metro_r: vec![0.0; block_size],
            clip_l: vec![0.0; block_size],
            clip_r: vec![0.0; block_size],
            clip_gain: vec![0.0; block_size],
            clip_pan: vec![0.0; block_size],
        };
        for beat in &mut graph.mixer_beat_params {
            beat.bus_index = graph
                .mixer
                .bus_index(&beat.bus)
                .expect("validated mixer bus");
        }
        graph.effect_targets = crate::effect_targets::EffectTargetIndex::from_graph(&graph);
        graph.rt_bindings = resolve_bindings(&graph);
        graph
    }
}
