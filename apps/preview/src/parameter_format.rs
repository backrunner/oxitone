use oxitone_core::wire::ParameterUnit;

pub fn number(value: f64) -> String {
    if value == 0. {
        return "0".into();
    }
    if value.abs() < 0.0001 || value.abs() >= 1e8 {
        return format!("{value:.4e}");
    }
    format!("{value:.4}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}
pub fn unit(value: ParameterUnit) -> &'static str {
    match value {
        ParameterUnit::Normalized => "normalized",
        ParameterUnit::Db => "dB",
        ParameterUnit::Hz => "Hz",
        ParameterUnit::Semitones => "semitones",
        ParameterUnit::Seconds => "s",
        ParameterUnit::Beats => "beats",
        ParameterUnit::Enum => "index",
    }
}
