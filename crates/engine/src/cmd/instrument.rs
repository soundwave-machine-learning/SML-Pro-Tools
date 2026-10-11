//! An instrument track's instrument (`mix.instrument`): the plugin that turns the track's MIDI
//! into audio, built in or hosted (CLAP, VST3, Audio Units). The mixer plays `Track::instrument`;
//! the instrument is otherwise only set when the track is created (`track.new`).

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_model::{Insert, TrackKind};

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "mix.instrument",
        "Set Instrument",
        [],
        None,
        "{track?, plugin: id} — an instrument track's instrument: built-in, `clap:`, `vst3:` or `au:` id",
        has_selection,
        set_instrument
    )]
}

fn set_instrument(e: &mut Engine, p: &Value) -> Result<Value> {
    let plugin = str_param(p, "plugin").ok_or_else(|| bad("mix.instrument", "`plugin` required"))?;
    if soundcraft_dsp::is_removed_builtin(plugin) {
        return Err(bad(
            "mix.instrument",
            format!("`{plugin}` was an upstream bundled instrument and is not part of SML Recording; use an installed VST3/CLAP instrument"),
        ));
    }
    let info = super::clap::plugin_info(plugin).ok_or_else(|| bad("mix.instrument", format!("unknown plugin `{plugin}`")))?;
    if !info.is_instrument {
        return Err(bad("mix.instrument", format!("`{plugin}` is not an instrument")));
    }
    let t = tracks_required(e, "mix.instrument", p)?.first().copied().ok_or_else(|| bad("mix.instrument", "no track"))?;
    if e.session().track(t).map(|tr| tr.kind) != Some(TrackKind::Instrument) {
        return Err(bad("mix.instrument", "only instrument tracks have an instrument"));
    }
    let mut ins = Insert::new(info.id);
    for pi in info.params {
        ins.params.insert(pi.id.to_string(), pi.default);
    }
    if let Some(tr) = e.session_mut().track_mut(t) {
        tr.instrument = Some(ins);
    }
    Ok(json!({"track": t, "plugin": info.id}))
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use serde_json::json;
    use soundcraft_model::TrackId;

    fn track(e: &mut Engine, kind: &str) -> TrackId {
        e.execute("track.new", &json!({"kind": kind})).unwrap();
        e.session().tracks.last().unwrap().id
    }

    fn instrument_of(e: &Engine, t: TrackId) -> Option<String> {
        e.session().track(t).and_then(|tr| tr.instrument.as_ref()).map(|i| i.plugin.clone())
    }

    /// Retired upstream test `sets_an_instrument_tracks_instrument_with_defaults_and_undoes`: it
    /// exercised the removed bundled synths. Shared coverage kept: new instrument tracks get no
    /// bundled default, removed instruments are rejected, and assignment of hosted instruments
    /// still goes through the same command (covered by the refusal test below and host tests).
    #[test]
    fn no_bundled_default_and_removed_instruments_rejected() {
        let mut e = Engine::default();
        let t = track(&mut e, "instrument");
        assert_eq!(instrument_of(&e, t), None, "no bundled default instrument");
        let err = e.execute("mix.instrument", &json!({"track": t.0, "plugin": "drum_synth"})).unwrap_err().to_string();
        assert!(err.contains("not part of SML Recording"), "{err}");
        assert_eq!(instrument_of(&e, t), None);
    }

    #[test]
    fn refuses_effects_unknown_plugins_and_other_track_kinds() {
        let mut e = Engine::default();
        let inst = track(&mut e, "instrument");
        let audio = track(&mut e, "audio");
        assert!(e.execute("mix.instrument", &json!({"track": inst.0, "plugin": "eq_7band"})).is_err(), "an effect");
        assert!(e.execute("mix.instrument", &json!({"track": inst.0, "plugin": "vst3:00000000000000000000000000000000"})).is_err());
        assert!(e.execute("mix.instrument", &json!({"track": inst.0})).is_err(), "no plugin");
        assert!(e.execute("mix.instrument", &json!({"track": audio.0, "plugin": "drum_synth"})).is_err(), "an audio track");
        assert_eq!(instrument_of(&e, inst), None);
        assert_eq!(instrument_of(&e, audio), None);
    }
}
