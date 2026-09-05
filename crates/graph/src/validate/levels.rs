//! Numeric domains of channels, effect inserts, and mixer buses.

use oxitone_core::error::OxitoneError;
use oxitone_core::wire::ProjectSnapshot;

use super::numeric::range;

pub(super) fn validate_levels(snapshot: &ProjectSnapshot) -> Result<(), OxitoneError> {
    for (i, channel) in snapshot.channels.iter().enumerate() {
        let path = format!("$.channels[{i}]");
        range(&format!("{path}.level"), "level", channel.level, 0.0, 2.0)?;
        range(&format!("{path}.pan"), "pan", channel.pan, -1.0, 1.0)?;
        if let Some(swing) = channel.swing {
            range(&format!("{path}.swing"), "swing", swing, 0.0, 1.0)?;
        }
        for (e, effect) in channel.effect_chain.iter().enumerate() {
            if let Some(mix) = effect.mix {
                range(
                    &format!("{path}.effectChain[{e}].mix"),
                    "mix",
                    mix,
                    0.0,
                    1.0,
                )?;
            }
        }
    }
    for (i, bus) in snapshot.mixer_channels.iter().enumerate() {
        let path = format!("$.mixerChannels[{i}]");
        range(&format!("{path}.level"), "level", bus.level, 0.0, 2.0)?;
        range(
            &format!("{path}.balance"),
            "balance",
            bus.balance,
            -1.0,
            1.0,
        )?;
        if let Some(ratio) = bus.master_send_ratio {
            range(
                &format!("{path}.masterSendRatio"),
                "masterSendRatio",
                ratio,
                0.0,
                1.0,
            )?;
        }
        for (e, insert) in bus.inserts.iter().enumerate() {
            if let Some(mix) = insert.mix {
                range(&format!("{path}.inserts[{e}].mix"), "mix", mix, 0.0, 1.0)?;
            }
        }
        for (s, send) in bus.sends.iter().enumerate() {
            range(
                &format!("{path}.sends[{s}].ratio"),
                "ratio",
                send.ratio,
                0.0,
                1.0,
            )?;
        }
    }
    Ok(())
}
