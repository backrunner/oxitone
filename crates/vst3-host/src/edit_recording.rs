//! Control-side recording override. Kept inside the isolated helper, never the device callback.
use crate::{
    edit_wire::{Recording, RecordingMode},
    native,
    wire::Event,
    Result,
};
use vst3_host::{ParameterEdit, ParameterEditKind, Plugin};

pub(super) struct Recorder {
    pub info: Recording,
    parameters: Vec<Parameter>,
}
struct Parameter {
    id: u32,
    value: f64,
    scheduled: f64,
    pressed: bool,
    touched: bool,
    overridden: bool,
}
impl Recorder {
    pub fn new(info: Recording, plugin: &Plugin) -> Result<Self> {
        let parameters = info
            .parameter_ids
            .iter()
            .map(|id| {
                let value = plugin.get_parameter(*id).map_err(native)?;
                crate::wire::normalized(value)?;
                Ok(Parameter {
                    id: *id,
                    value,
                    scheduled: value,
                    pressed: false,
                    touched: false,
                    overridden: false,
                })
            })
            .collect::<Result<_>>()?;
        Ok(Self { info, parameters })
    }
    pub fn collect(&mut self, edit: &ParameterEdit) -> Result<()> {
        let Some(parameter) = self.parameters.iter_mut().find(|p| p.id == edit.id) else {
            return Ok(());
        };
        let touch = self.info.mode == RecordingMode::Touch;
        match edit.kind {
            ParameterEditKind::BeginGesture => {
                if touch && parameter.pressed {
                    return Err(crate::invalid("VST3 recording received duplicate begin"));
                }
                if !parameter.overridden && !parameter.touched {
                    parameter.value = parameter.scheduled;
                }
                parameter.pressed = true;
                parameter.touched = true;
            }
            ParameterEditKind::ValueChange => {
                if touch && !parameter.pressed {
                    return Err(crate::invalid("VST3 Touch value requires begin"));
                }
                parameter.value = edit
                    .value
                    .ok_or_else(|| crate::invalid("VST3 recording value missing"))?;
                parameter.touched = true;
            }
            ParameterEditKind::EndGesture => {
                if touch && !parameter.pressed {
                    return Err(crate::invalid("VST3 Touch end requires begin"));
                }
                parameter.pressed = false;
                parameter.touched = true;
            }
        }
        Ok(())
    }
    pub fn values(&self) -> impl Iterator<Item = (u32, f64)> + '_ {
        self.parameters
            .iter()
            .filter(|p| self.info.mode == RecordingMode::Write || p.pressed || p.touched)
            .map(|p| (p.id, p.value))
    }
    pub fn apply(
        &mut self,
        plugin: &mut Plugin,
        events: &mut [Event],
        enabled: bool,
    ) -> Result<usize> {
        for parameter in &mut self.parameters {
            let active = enabled
                && (self.info.mode == RecordingMode::Write
                    || parameter.pressed
                    || parameter.touched);
            if parameter.overridden && !active {
                plugin
                    .set_parameter_at(parameter.id, parameter.scheduled, 0)
                    .map_err(native)?;
            }
            parameter.overridden = active;
        }
        let mut count = 0;
        for index in 0..events.len() {
            let event = events[index];
            if let Event::Parameter {
                parameter_id,
                value,
                ..
            } = event
            {
                if let Some(parameter) = self.parameters.iter_mut().find(|p| p.id == parameter_id) {
                    parameter.scheduled = value;
                    if parameter.overridden {
                        continue;
                    }
                }
            }
            events[count] = event;
            count += 1;
        }
        for parameter in &mut self.parameters {
            if parameter.overridden {
                plugin
                    .set_parameter_at(parameter.id, parameter.value, 0)
                    .map_err(native)?;
            }
            if enabled {
                parameter.touched = false;
            }
        }
        Ok(count)
    }
    pub fn restore(&self, plugin: &mut Plugin) -> Result<()> {
        for parameter in &self.parameters {
            if parameter.overridden {
                plugin
                    .set_parameter(parameter.id, parameter.scheduled)
                    .map_err(native)?;
            }
        }
        Ok(())
    }
}
