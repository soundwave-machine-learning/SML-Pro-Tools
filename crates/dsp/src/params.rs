//! Parameter storage and const constructors for static parameter tables.

use crate::{ParamInfo, Taper, Unit};

/// A continuous parameter.
pub(crate) const fn p(id: &'static str, name: &'static str, min: f32, max: f32, default: f32, unit: Unit, taper: Taper) -> ParamInfo {
    ParamInfo { id, name, min, max, default, unit, taper, choices: &[] }
}

/// A linear-taper parameter.
pub(crate) const fn lin(id: &'static str, name: &'static str, min: f32, max: f32, default: f32, unit: Unit) -> ParamInfo {
    p(id, name, min, max, default, unit, Taper::Linear)
}

/// A log-taper parameter (frequencies, times, ratios).
pub(crate) const fn log(id: &'static str, name: &'static str, min: f32, max: f32, default: f32, unit: Unit) -> ParamInfo {
    p(id, name, min, max, default, unit, Taper::Log)
}

/// An on/off switch.
pub(crate) const fn toggle(id: &'static str, name: &'static str, default_on: bool) -> ParamInfo {
    ParamInfo { id, name, min: 0.0, max: 1.0, default: if default_on { 1.0 } else { 0.0 }, unit: Unit::Toggle, taper: Taper::Linear, choices: &[] }
}

/// A list choice; the value is the index.
pub(crate) const fn choice(id: &'static str, name: &'static str, choices: &'static [&'static str], default: usize) -> ParamInfo {
    let max = if choices.is_empty() { 0.0 } else { (choices.len() - 1) as f32 };
    ParamInfo { id, name, min: 0.0, max, default: default as f32, unit: Unit::Choice, taper: Taper::Linear, choices }
}
