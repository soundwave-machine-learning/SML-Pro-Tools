//! Third-party CLAP plugins: discovery (`engine.clap_plugins`) and id lookup shared by the mixer
//! commands. Hosting itself lives in `soundcraft-clap-host`.

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_dsp::PluginInfo;

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        query "engine.clap_plugins",
        "List CLAP Plugins",
        [],
        None,
        "{rescan?: bool} → list, or with rescan in SML Studio a scan job {job, coalesced} (see engine.plugin_scan_status; a process without the scan service rescans in-process and returns the list) — list: [{id: \"clap:…\", plugin_id, name, vendor, version, description, features, category, is_instrument, path}]",
        always,
        |_, p| {
            if p.get("rescan").and_then(Value::as_bool).unwrap_or(false) {
                // HF12: in the app a rescan is an isolated scan job (returns at once with its
                // state); only a process without the scan service scans here.
                if let Some(job) = super::plugin_scan::submit_if_installed(true) {
                    return Ok(job);
                }
                return Ok(json!(soundcraft_clap_host::rescan()));
            }
            Ok(json!(soundcraft_clap_host::scan()))
        }
    )]
}

/// The installed version of a hosted plugin from the scan cache (`None` for unknown ids).
pub fn installed_version(id: &str) -> Option<String> {
    let id = soundcraft_mix::hosted_id(id);
    if id.starts_with(soundcraft_clap_host::ID_PREFIX) {
        return soundcraft_clap_host::scan().into_iter().find(|d| d.id == id).map(|d| d.version);
    }
    if id.starts_with(soundcraft_vst3_host::ID_PREFIX) {
        return soundcraft_vst3_host::scan().into_iter().find(|d| d.id == id).map(|d| d.version);
    }
    if id.starts_with(soundcraft_au_host::ID_PREFIX) {
        return soundcraft_au_host::scan().into_iter().find(|d| d.id == id).map(|d| d.version);
    }
    None
}

/// A built-in plugin's description, else a hosted CLAP (`clap:<id>`), VST3 (`vst3:<class id>`) or
/// Audio Units (`au:<type>:<subtype>:<manufacturer>`) plugin's.
pub fn plugin_info(id: &str) -> Option<&'static PluginInfo> {
    // Removed upstream built-ins are not insertable (SML Recording ships no bundled effects).
    // A `sandbox:` id describes the same plugin (hosted out of process).
    let id = soundcraft_mix::hosted_id(id);
    soundcraft_clap_host::plugin_info(id).or_else(|| soundcraft_vst3_host::plugin_info(id)).or_else(|| soundcraft_au_host::plugin_info(id))
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use serde_json::json;

    #[test]
    fn clap_plugins_query_returns_a_list() {
        let mut e = Engine::default();
        let v = e.execute("engine.clap_plugins", &json!({})).unwrap();
        assert!(v.is_array());
    }

    #[test]
    fn unknown_clap_insert_is_an_error() {
        let mut e = Engine::default();
        e.execute("track.new", &json!({})).unwrap();
        let t = e.session().tracks[0].id.0;
        let r = e.execute("mix.insert", &json!({"track": t, "plugin": "clap:no.such.plugin"}));
        assert!(r.is_err());
        assert!(super::plugin_info("eq_7band").is_none(), "removed bundled processors are not insertable");
        let r = e.execute("mix.insert", &json!({"track": t, "plugin": "eq_7band"}));
        assert!(r.unwrap_err().to_string().contains("not part of SML Recording"));
        assert!(super::plugin_info("clap:no.such.plugin").is_none());
    }

    /// Apple's own Audio Units ship with every Mac, so `mix.insert` can be checked against one.
    #[cfg(target_os = "macos")]
    #[test]
    fn audio_units_insert_by_id() {
        let mut e = Engine::default();
        e.execute("track.new", &json!({})).unwrap();
        let t = e.session().tracks[0].id.0;
        let r = e.execute("mix.insert", &json!({"track": t, "plugin": "au:aufx:bpas:appl"}));
        assert_eq!(r.map(|v| v["plugin"].clone()), Ok(json!("au:aufx:bpas:appl")), "Apple AUBandpass");
    }
}
