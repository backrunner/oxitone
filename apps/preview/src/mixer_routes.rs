//! Source routing, including the dedicated Master path and detector-only sends.
use oxitone_core::wire::ProjectSnapshot;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteKind {
    Output,
    Auxiliary(usize),
    Midi(Option<usize>),
    InsertInput { insert: usize, bus: usize },
    InsertOutput { insert: usize, bus: usize },
    Master,
    Send,
    Sidechain,
}
#[derive(Clone, Debug)]
pub struct Route {
    pub source: String,
    pub destination: String,
    pub source_name: String,
    pub destination_name: String,
    pub kind: RouteKind,
    pub ratio: f64,
    pub pre_fader: bool,
    pub automated: bool,
}
impl Route {
    pub fn is_output(&self) -> bool {
        matches!(
            self.kind,
            RouteKind::Output
                | RouteKind::Auxiliary(_)
                | RouteKind::Midi(_)
                | RouteKind::InsertInput { .. }
                | RouteKind::InsertOutput { .. }
        )
    }
    pub fn label(&self) -> String {
        match self.kind {
            RouteKind::Midi(None) => "Instrument MIDI → Instrument".into(),
            RouteKind::Midi(Some(index)) => format!("Insert {} MIDI → Instrument", index + 1),
            RouteKind::Output => "Main output · Post-insert/fader".into(),
            RouteKind::Auxiliary(index) => format!("Output {index} · Before inserts, post-fader"),
            RouteKind::InsertInput { insert, bus } => {
                format!("Insert {} input {bus} · Post-fader", insert + 1)
            }
            RouteKind::InsertOutput { insert, bus } => {
                format!("Insert {} output {bus} · Post-fader", insert + 1)
            }
            _ => self.tap().into(),
        }
    }
    pub fn is_send(&self) -> bool {
        matches!(
            self.kind,
            RouteKind::Send
                | RouteKind::Sidechain
                | RouteKind::InsertInput { .. }
                | RouteKind::Midi(_)
        )
    }
    pub fn tap(&self) -> &'static str {
        if self.kind == RouteKind::Sidechain {
            "Detector · pre-fader"
        } else if self.pre_fader {
            "Pre-fader"
        } else {
            "Post-fader"
        }
    }
}
pub fn name(s: &ProjectSnapshot, id: &str) -> String {
    s.channels
        .iter()
        .enumerate()
        .find(|(_, c)| c.id == id)
        .map(|(i, c)| {
            c.name
                .clone()
                .unwrap_or_else(|| format!("Channel {}", i + 1))
        })
        .or_else(|| {
            s.mixer_channels
                .iter()
                .find(|b| b.id == id)
                .and_then(|b| b.name.clone())
        })
        .unwrap_or_else(|| {
            if id == "mix_master" {
                "Master".into()
            } else {
                id.into()
            }
        })
}
pub fn connections(s: &ProjectSnapshot) -> Vec<Route> {
    let mut routes = Vec::new();
    let mut add = |source: &str, destination: &str, kind, ratio, pre_fader, parameter: &str| {
        routes.push(Route {
            source: source.into(),
            destination: destination.into(),
            source_name: name(s, source),
            destination_name: name(s, destination),
            kind,
            ratio,
            pre_fader,
            automated: s
                .automation
                .iter()
                .any(|a| a.target.entity_id == source && a.target.parameter_id == parameter),
        });
    };
    for c in &s.channels {
        add(&c.id, &c.mixer_channel_id, RouteKind::Output, 1., false, "");
        for (instance, destinations) in c.midi_routes.iter().flatten() {
            let insert = c
                .effect_chain
                .iter()
                .position(|effect| effect.instance_id.as_ref() == Some(instance));
            for destination in destinations {
                add(&c.id, destination, RouteKind::Midi(insert), 1., true, "");
            }
        }
        let mut auxiliary: Vec<_> = c
            .output_routes
            .iter()
            .flatten()
            .filter_map(|(index, destination)| {
                index
                    .parse::<usize>()
                    .ok()
                    .map(|index| (index, destination))
            })
            .collect();
        auxiliary.sort_by_key(|(index, _)| *index);
        for (index, destination) in auxiliary {
            add(
                &c.id,
                destination,
                RouteKind::Auxiliary(index),
                1.,
                false,
                "",
            );
        }
    }
    for b in s.mixer_channels.iter().filter(|b| b.id != "mix_master") {
        add(
            &b.id,
            "mix_master",
            RouteKind::Master,
            b.master_send_ratio.unwrap_or(1.),
            false,
            "masterSendRatio",
        );
        for send in &b.sends {
            let sidechain = send.sidechain == Some(true);
            add(
                &b.id,
                &send.destination_id,
                if sidechain {
                    RouteKind::Sidechain
                } else {
                    RouteKind::Send
                },
                send.ratio,
                sidechain || send.pre_fader == Some(true),
                &format!("send.{}.ratio", send.destination_id),
            );
        }
    }
    for owner in &s.mixer_channels {
        for (insert, effect) in owner.inserts.iter().enumerate() {
            let Some(mapping) = effect
                .instance_id
                .as_ref()
                .and_then(|id| owner.insert_routes.as_ref()?.get(id))
            else {
                continue;
            };
            for (input, buses) in [(true, &mapping.inputs), (false, &mapping.outputs)] {
                let mut buses: Vec<_> = buses
                    .iter()
                    .flatten()
                    .filter_map(|(index, id)| index.parse::<usize>().ok().map(|index| (index, id)))
                    .collect();
                buses.sort_by_key(|(index, _)| *index);
                for (bus, target) in buses {
                    if input {
                        add(
                            target,
                            &owner.id,
                            RouteKind::InsertInput { insert, bus },
                            1.,
                            false,
                            "",
                        );
                    } else {
                        add(
                            &owner.id,
                            target,
                            RouteKind::InsertOutput { insert, bus },
                            1.,
                            false,
                            "",
                        );
                    }
                }
            }
        }
    }
    routes
}
