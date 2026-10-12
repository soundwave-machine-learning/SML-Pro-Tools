//! Plugin hosting capability matrix (R31): what every discovered CLAP/VST3/AU plugin actually
//! exposes to this host, read from a live instance, plus the truthful status of the seven SML
//! external-project integration targets (R23–R29). Facts about interfaces, never about sound.

use super::*;
use crate::cmd;
use serde_json::json;

/// The seven separately developed SML plugins: host work this DAW prepares for each, and the
/// status until an actual artifact is supplied. Never a plugin to build here.
pub const EXTERNAL_TARGETS: &[(&str, &str, &str)] = &[
    ("R23", "Channel EQ", "generic parameter/state/automation, stereo layout, editor lifecycle"),
    ("R24", "Gate/Expander", "internal/external sidechain routing, detector source visibility, lookahead/PDC, state"),
    ("R25", "Flexible Compressor", "automation/smoothing boundaries, plugin-defined wet/dry, latency/state, sidechain"),
    ("R26", "Optical Leveler", "save/reload, declared latency/tail, reset lifecycle, long processing"),
    ("R27", "Tune", "low-latency monitoring, declared delay, dry-capture integrity, offline processing"),
    (
        "R28",
        "Vocal Editor",
        "interface to be investigated on delivery: a regular insert does not establish clip-linked note editing (native/ARA/other: undetermined)",
    ),
    ("R29", "Align", "guide/dub input contract, sidechain/multi-input, non-destructive render handoff where provided"),
];

/// Deterministic host fixtures (test builds only): prove routing, order and compensation
/// without a sound model. Never listed in a browser.
pub const FIXTURES: &[(&str, &str)] = &[
    ("test:identity", "pass-through"),
    ("test:gain:<dB>", "fixed gain (settable `gain` parameter)"),
    ("test:delay:<samples>", "declared latency of that many samples"),
    ("test:affine:<a>:<b>", "y = a*x + b (non-commuting with gain: proves order)"),
    ("test:sidegate:<dB>", "passes the main input only while the sidechain input exceeds the threshold (proves sidechain routing)"),
];

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "engine.host_faults", "Plugin Faults", [], None, "{} → {faults: [{plugin, fault, at, restarted, restarts}], auto_restart, sandboxed: [{plugin, pid, fault, blocks, late_blocks, restarts, rss_kib, added_latency_samples}]}", always, |_, _| {
            #[cfg(not(target_arch = "wasm32"))]
            {
                use soundcraft_mix::sandbox;
                Ok(json!({"faults": sandbox::faults(), "auto_restart": sandbox::auto_restart(), "sandboxed": sandbox::status(),
                    "provenance": "sandboxed (out-of-process) instances only; in-process plugins have no containment (E03)"}))
            }
            #[cfg(target_arch = "wasm32")]
            Ok(json!({"faults": [], "auto_restart": false, "sandboxed": []}))
        }),
        cmd!(query "session.plugin_versions", "Plugin Versions", [], None, "{} → [{track, at, plugin, saved_version, installed_version, status: same|changed|unrecorded|missing}]", always, |e, _| {
            let s = e.session();
            let mut rows = Vec::new();
            for t in &s.tracks {
                let mut row = |at: String, ins: &soundcraft_model::Insert| {
                    if !soundcraft_mix::is_third_party(&ins.plugin) {
                        return;
                    }
                    let installed = super::clap::installed_version(&ins.plugin);
                    let status = match (&ins.plugin_version, &installed) {
                        (_, None) => "missing",
                        (None, Some(_)) => "unrecorded",
                        (Some(a), Some(b)) if a == b => "same",
                        _ => "changed",
                    };
                    rows.push(json!({"track": t.name, "at": at, "plugin": ins.plugin, "saved_version": ins.plugin_version, "installed_version": installed, "status": status}));
                };
                for (i, ins) in t.mixer.inserts.iter().enumerate() {
                    if let Some(ins) = ins {
                        row(format!("insert {}", (b'a' + i as u8) as char), ins);
                    }
                }
                if let Some(ins) = &t.instrument {
                    row("instrument".into(), ins);
                }
                for k in soundcraft_model::SlotKind::ALL {
                    if let Some(p) = t.mixer.slots.get(k).and_then(|x| x.provider.as_ref()) {
                        row(k.id().into(), p);
                    }
                }
            }
            Ok(json!(rows))
        }),
        cmd!(
            query "engine.plugin_capabilities",
            "Plugin Capabilities",
            [],
            None,
            "{id?: \"clap:…\"|\"vst3:…\"} → {plugins: [HostCapabilities | {id, error}], external_targets: [{requirement, project, status, host_work}], fixtures: [{id, purpose}], provenance}",
            always,
            |_, p| {
                let only = p.get("id").and_then(Value::as_str);
                let mut plugins = Vec::new();
                for d in soundcraft_clap_host::scan() {
                    if only.is_none_or(|o| o == d.id) {
                        plugins.push(match soundcraft_clap_host::capabilities(&d.id) {
                            Ok(c) => json!(c),
                            Err(e) => json!({"id": d.id, "format": "CLAP", "error": e.to_string()}),
                        });
                    }
                }
                for d in soundcraft_vst3_host::scan() {
                    if only.is_none_or(|o| o == d.id) {
                        plugins.push(match soundcraft_vst3_host::capabilities(&d.id) {
                            Ok(c) => json!(c),
                            Err(e) => json!({"id": d.id, "format": "VST3", "error": e.to_string()}),
                        });
                    }
                }
                for d in soundcraft_au_host::scan() {
                    if only.is_none_or(|o| o == d.id) {
                        plugins.push(json!({"id": d.id, "format": "AU", "name": d.name, "vendor": d.vendor, "version": d.version, "is_instrument": d.is_instrument, "error": "capabilities are not probed for Audio Units by this build"}));
                    }
                }
                if let Some(o) = only
                    && plugins.is_empty()
                {
                    return Err(bad("engine.plugin_capabilities", format!("no discovered plugin `{o}`")));
                }
                let external: Vec<Value> = EXTERNAL_TARGETS
                    .iter()
                    .map(|(r, name, work)| json!({"requirement": r, "project": name, "status": "AWAITING_EXTERNAL_PLUGIN", "host_work": work}))
                    .collect();
                let fixtures: Vec<Value> = FIXTURES.iter().map(|(id, what)| json!({"id": id, "purpose": what, "kind": "deterministic host fixture (test builds only)"})).collect();
                Ok(json!({
                    "plugins": plugins,
                    "external_targets": external,
                    "fixtures": fixtures,
                    "provenance": "read from live instances on this machine at 48 kHz / 1024 frames / stereo; a row proves the interface as hosted here, not the plugin's sound or its behaviour on another platform",
                }))
            }
        ),
    ]
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use serde_json::json;

    #[test]
    fn capability_matrix_always_names_the_seven_targets_as_awaiting() {
        let mut e = Engine::default();
        let v = e.execute("engine.plugin_capabilities", &json!({})).unwrap();
        let ext = v["external_targets"].as_array().unwrap();
        assert_eq!(ext.len(), 7);
        assert!(ext.iter().all(|r| r["status"] == "AWAITING_EXTERNAL_PLUGIN"));
        assert!(ext.iter().any(|r| r["requirement"] == "R28" && r["host_work"].as_str().unwrap().contains("undetermined")));
        assert!(v["fixtures"].as_array().unwrap().iter().any(|f| f["id"] == "test:sidegate:<dB>"));
        assert!(v["plugins"].is_array());
        assert!(e.execute("engine.plugin_capabilities", &json!({"id": "clap:no.such"})).is_err());
    }
}
