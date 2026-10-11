//! Easy bus workflow (BUS_WORKFLOW_SPEC B01–B06): one-step Create Bus from Selection, Create FX
//! Return and Sends, safe bus deletion, graph cycle validation and view-only Spill.
//!
//! In this engine a bus channel is an Aux track whose input is a routing bus; sources feed it by
//! output assignment (subgroup) or by sends (FX return). Every action here is a single command,
//! so it is one undo step, and validation happens before anything changes (a rejected command
//! leaves the exact pre-operation graph).

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_model::{BusId, ChannelFormat, Route, SendSlot, Session, TrackKind};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "bus.create_from_selection",
            "Create Bus from Selection",
            ["Mix"],
            None,
            "{tracks?, name?: \"Bus\", destination?: main|<bus track name>}",
            has_tracks,
            create_from_selection
        ),
        cmd!(
            "bus.create_fx_return",
            "Create FX Return",
            ["Mix"],
            None,
            "{tracks?, name?: \"FX Return\", plugin?: id, destination?: main|<bus track name>, pre_fader?: false}",
            has_tracks,
            create_fx_return
        ),
        cmd!("bus.delete", "Delete Bus", [], None, "{track: bus track, sends?: remove|<bus track name>}", has_tracks, delete_bus),
        cmd!(query "bus.validate", "Validate Routing Graph", [], None, "{}", always, |e, _| {
            Ok(match graph_cycle(e.session()) {
                None => json!({"ok": true}),
                Some(path) => json!({"ok": false, "cycle": path}),
            })
        }),
        cmd!(query "bus.sources", "Bus Sources", [], None, "{track}", has_tracks, sources),
        cmd!(noundo "bus.spill", "Spill Bus", [], None, "{track?: bus track (none = exit spill)}", always, spill),
    ]
}

fn route_label(s: &Session, r: &Route) -> String {
    match r {
        Route::Main => "main".into(),
        Route::Bus(b) => s.busses.iter().find(|x| x.id == *b).map_or_else(|| format!("bus {}", b.0), |x| x.name.clone()),
        other => format!("{other:?}"),
    }
}

/// Destination parameter: `main` or the name of a bus *track* (aux) — resolved to its input bus.
fn destination(e: &Engine, cmd: &str, p: &Value) -> Result<Option<Route>> {
    let Some(d) = str_param(p, "destination") else { return Ok(None) };
    if d.eq_ignore_ascii_case("main") {
        return Ok(Some(Route::Main));
    }
    let s = e.session();
    let t = s.tracks.iter().find(|t| t.name == d && t.kind == TrackKind::Aux).ok_or_else(|| bad(cmd, format!("no bus named `{d}`")))?;
    match &t.mixer.input {
        Route::Bus(b) => Ok(Some(Route::Bus(*b))),
        _ => Err(bad(cmd, format!("`{d}` has no bus input"))),
    }
}

/// Aux tracks fed (by output or send) from `t`, transitively.
fn downstream(s: &Session, from: TrackId) -> Vec<TrackId> {
    let mut out: Vec<TrackId> = Vec::new();
    let mut stack = vec![from];
    while let Some(id) = stack.pop() {
        let Some(t) = s.track(id) else { continue };
        let mut targets: Vec<BusId> = Vec::new();
        if let Route::Bus(b) = t.mixer.output {
            targets.push(b);
        }
        targets.extend(t.mixer.sends.iter().flatten().filter_map(|x| if let Route::Bus(b) = x.target { Some(b) } else { None }));
        for b in targets {
            for a in s.tracks.iter().filter(|a| a.kind == TrackKind::Aux && a.mixer.input == Route::Bus(b)) {
                if !out.contains(&a.id) {
                    out.push(a.id);
                    stack.push(a.id);
                }
            }
        }
    }
    out
}

/// A cycle in the output/send graph, as track names, if any.
pub fn graph_cycle(s: &Session) -> Option<Vec<String>> {
    for t in &s.tracks {
        if downstream(s, t.id).contains(&t.id) {
            return Some(vec![t.name.clone()]);
        }
    }
    None
}

fn new_bus_track(s: &mut Session, name: &str, after: usize, output: Route) -> (TrackId, BusId) {
    let base = if name.trim().is_empty() { "Bus" } else { name.trim() };
    // Unique bus name (busses are matched by name).
    let mut bus_name = base.to_string();
    let mut n = 2;
    while s.bus_by_name(&bus_name).is_some() {
        bus_name = format!("{base} {n}");
        n += 1;
    }
    let b = s.add_bus(&bus_name, ChannelFormat::Stereo);
    let id = s.add_track(TrackKind::Aux, ChannelFormat::Stereo, Some(base));
    if let Some(t) = s.track_mut(id) {
        // B01 neutral defaults: unity fader/trim, centred neutral stereo, no plugins.
        t.mixer.input = Route::Bus(b);
        t.mixer.output = output;
        t.mixer.volume_db = 0.0;
        t.mixer.trim_db = 0.0;
        t.mixer.pan = vec![-1.0, 1.0];
    }
    // Adjacent placement: right after the last selected source.
    if let Some(cur) = s.tracks.iter().position(|t| t.id == id) {
        let t = s.tracks.remove(cur);
        let at = (after + 1).min(s.tracks.len());
        s.tracks.insert(at, t);
    }
    (id, b)
}

fn selected_sources(e: &Engine, cmd: &str, p: &Value) -> Result<Vec<TrackId>> {
    let ids = tracks_required(e, cmd, p)?;
    let s = e.session();
    for id in &ids {
        let t = s.track(*id).ok_or_else(|| bad(cmd, "unknown track"))?;
        if matches!(t.kind, TrackKind::Master | TrackKind::Vca | TrackKind::Folder) {
            return Err(bad(cmd, format!("`{}` cannot feed a bus", t.name)));
        }
    }
    // B02: an ancestor and its child together would duplicate signal paths.
    for a in &ids {
        let down = downstream(s, *a);
        if let Some(c) = ids.iter().find(|c| down.contains(c)) {
            let (an, cn) = (s.track(*a).map_or("?", |t| t.name.as_str()), s.track(*c).map_or("?", |t| t.name.as_str()));
            return Err(bad(cmd, format!("`{an}` already feeds `{cn}`: select only one level of the hierarchy")));
        }
    }
    Ok(ids)
}

fn create_from_selection(e: &mut Engine, p: &Value) -> Result<Value> {
    let cmd = "bus.create_from_selection";
    let ids = selected_sources(e, cmd, p)?;
    let dest = destination(e, cmd, p)?;
    let s = e.session();
    let outputs: Vec<Route> = ids.iter().filter_map(|i| s.track(*i).map(|t| t.mixer.output.clone())).collect();
    let shared = outputs.first().filter(|o| outputs.iter().all(|x| x == *o)).cloned();
    let output = match (dest, shared) {
        (Some(d), _) => d,
        (None, Some(o)) => o,
        (None, None) => {
            let names: Vec<String> = outputs.iter().map(|o| route_label(s, o)).collect();
            return Err(bad(
                cmd,
                format!("the selected tracks feed different outputs ({}); choose a destination (suggested: main)", names.join(", ")),
            ));
        }
    };
    let name = str_param(p, "name").unwrap_or("Bus").to_string();
    let last = ids.iter().filter_map(|i| s.tracks.iter().position(|t| t.id == *i)).max().unwrap_or(0);
    let s = e.session_mut();
    let (bus_track, bus) = new_bus_track(s, &name, last, output.clone());
    let mut changed = Vec::new();
    for id in &ids {
        if let Some(t) = s.track_mut(*id) {
            changed.push(json!({"track": t.name, "from": format!("{:?}", t.mixer.output), "to": "bus"}));
            t.mixer.output = Route::Bus(bus);
        }
    }
    if let Some(c) = graph_cycle(s) {
        return Err(bad(cmd, format!("routing would create a cycle through {}", c.join(" → "))));
    }
    s.edit.selected_tracks = vec![bus_track];
    let label = route_label(s, &output);
    let bus_name = s.track(bus_track).map(|t| t.name.clone()).unwrap_or_default();
    Ok(json!({"bus_track": bus_track.0, "bus": bus.0, "name": bus_name, "output": label, "rerouted": changed}))
}

fn create_fx_return(e: &mut Engine, p: &Value) -> Result<Value> {
    let cmd = "bus.create_fx_return";
    let ids = selected_sources(e, cmd, p)?;
    let output = destination(e, cmd, p)?.unwrap_or(Route::Main);
    let plugin = str_param(p, "plugin").map(str::to_string);
    if let Some(pl) = &plugin
        && !soundcraft_mix::is_third_party(pl)
    {
        return Err(bad(cmd, format!("`{pl}` is not an installed external plugin (SML Recording ships no bundled effects)")));
    }
    let pre = p.get("pre_fader").and_then(Value::as_bool).unwrap_or(false);
    let name = str_param(p, "name").unwrap_or("FX Return").to_string();
    let s = e.session();
    let last = ids.iter().filter_map(|i| s.tracks.iter().position(|t| t.id == *i)).max().unwrap_or(0);
    // Every source needs a free send slot.
    for id in &ids {
        let t = s.track(*id).ok_or_else(|| bad(cmd, "unknown track"))?;
        if !t.mixer.sends.iter().any(Option::is_none) {
            return Err(bad(cmd, format!("`{}` has no free send", t.name)));
        }
    }
    let s = e.session_mut();
    let (ret, bus) = new_bus_track(s, &name, last, output);
    if let (Some(pl), Some(t)) = (plugin, s.track_mut(ret))
        && let Some(slot) = t.mixer.inserts.first_mut()
    {
        *slot = Some(soundcraft_model::Insert::new(&pl));
    }
    let mut sends = Vec::new();
    for id in &ids {
        if let Some(t) = s.track_mut(*id)
            && let Some((i, slot)) = t.mixer.sends.iter_mut().enumerate().find(|(_, x)| x.is_none())
        {
            // Dry output preserved; send starts at -inf (silent) until raised.
            let mut snd = SendSlot::new(Route::Bus(bus));
            snd.pre_fader = pre;
            *slot = Some(snd);
            sends.push(json!({"track": t.name, "slot": i}));
        }
    }
    if let Some(c) = graph_cycle(s) {
        return Err(bad(cmd, format!("routing would create a cycle through {}", c.join(" → "))));
    }
    s.edit.selected_tracks = vec![ret];
    Ok(
        json!({"return_track": ret.0, "bus": bus.0, "sends": sends, "note": "sends start at -inf; an empty return duplicates the dry signal once raised"}),
    )
}

fn delete_bus(e: &mut Engine, p: &Value) -> Result<Value> {
    let cmd = "bus.delete";
    let id = track_param(e, cmd, p, "track")?.ok_or_else(|| bad(cmd, "`track` required"))?;
    let s = e.session();
    let t = s.track(id).ok_or_else(|| bad(cmd, "unknown track"))?;
    if t.kind != TrackKind::Aux {
        return Err(bad(cmd, "not a bus track"));
    }
    let Route::Bus(bus) = t.mixer.input else { return Err(bad(cmd, "this aux has no bus input")) };
    let parent = t.mixer.output.clone();
    let sends_to = match str_param(p, "sends") {
        None | Some("remove") => None,
        Some(_) => match destination(e, cmd, &json!({"destination": str_param(p, "sends")}))? {
            Some(Route::Bus(b)) if b != bus => Some(b),
            _ => return Err(bad(cmd, "sends can be removed or reassigned to another bus")),
        },
    };
    let s = e.session_mut();
    let mut rerouted = 0;
    let mut sends = 0;
    for tr in s.tracks.iter_mut().filter(|x| x.id != id) {
        if tr.mixer.output == Route::Bus(bus) {
            tr.mixer.output = parent.clone();
            rerouted += 1;
        }
        for slot in tr.mixer.sends.iter_mut() {
            if slot.as_ref().is_some_and(|x| x.target == Route::Bus(bus)) {
                sends += 1;
                match sends_to {
                    Some(b) => {
                        if let Some(x) = slot.as_mut() {
                            x.target = Route::Bus(b);
                        }
                    }
                    None => *slot = None,
                }
            }
        }
    }
    s.tracks.retain(|x| x.id != id);
    if !s.tracks.iter().any(|x| x.mixer.input == Route::Bus(bus)) {
        s.busses.retain(|b| b.id != bus);
    }
    s.edit.selected_tracks.retain(|x| *x != id);
    if let Some(c) = graph_cycle(s) {
        return Err(bad(cmd, format!("rerouting would create a cycle through {}", c.join(" → "))));
    }
    Ok(json!({"deleted": id.0, "rerouted_sources": rerouted, "sends_changed": sends}))
}

fn sources(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = track_param(e, "bus.sources", p, "track")?.ok_or_else(|| bad("bus.sources", "`track` required"))?;
    let s = e.session();
    let Some(Route::Bus(bus)) = s.track(id).map(|t| t.mixer.input.clone()) else { return Err(bad("bus.sources", "not a bus track")) };
    let direct: Vec<String> = s.tracks.iter().filter(|t| t.mixer.output == Route::Bus(bus)).map(|t| t.name.clone()).collect();
    let sends: Vec<String> =
        s.tracks.iter().filter(|t| t.mixer.sends.iter().flatten().any(|x| x.target == Route::Bus(bus))).map(|t| t.name.clone()).collect();
    Ok(json!({"direct": direct, "sends": sends}))
}

/// View-only Spill: shows the bus plus its direct and send sources, hiding the rest of the
/// mixer; exiting restores the exact previous visibility. Never reroutes or mutes.
fn spill(e: &mut Engine, p: &Value) -> Result<Value> {
    let root = track_param(e, "bus.spill", p, "track")?;
    let s = e.session_mut();
    // Exit any previous spill first (restoring saved visibility).
    let saved: Vec<u64> = s.edit.values.keys().filter_map(|k| k.strip_prefix("spill.hidden.").and_then(|x| x.parse().ok())).collect();
    let was_spilled = s.edit.values.contains_key("spill.root");
    if was_spilled {
        for t in s.tracks.iter_mut() {
            t.hidden = saved.contains(&t.id.0);
        }
        s.edit.values.retain(|k, _| !k.starts_with("spill."));
    }
    let Some(root) = root else { return Ok(json!({"spill": null})) };
    let bus = match s.track(root).map(|t| t.mixer.input.clone()) {
        Some(Route::Bus(b)) => b,
        _ => return Err(bad("bus.spill", "spill needs a bus track")),
    };
    for t in &s.tracks {
        if t.hidden {
            s.edit.values.insert(format!("spill.hidden.{}", t.id.0), 1.0);
        }
    }
    s.edit.values.insert("spill.root".into(), root.0 as f64);
    let mut shown = Vec::new();
    for t in s.tracks.iter_mut() {
        let direct = t.mixer.output == Route::Bus(bus);
        let send = t.mixer.sends.iter().flatten().any(|x| x.target == Route::Bus(bus));
        let visible = t.id == root || direct || send || t.kind == TrackKind::Master;
        t.hidden = !visible;
        if visible && t.id != root && t.kind != TrackKind::Master {
            shown.push(json!({"track": t.name, "via": if direct { "output" } else { "send" }}));
        }
    }
    Ok(json!({"spill": root.0, "sources": shown}))
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use serde_json::json;
    use soundcraft_model::{ChannelFormat, Clip, Route, TrackId, TrackKind};
    use soundcraft_time::Range;

    /// Three DC-ish tracks feeding Main, distinct levels and pans.
    fn three() -> (Engine, Vec<TrackId>) {
        let mut e = Engine::default();
        let mut ids = Vec::new();
        for (k, (name, pan)) in [("Lead", 0.0f32), ("Double L", -0.6), ("Double R", 0.6)].into_iter().enumerate() {
            let s = e.session_mut();
            let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, Some(name));
            let data: Vec<f32> = (0..4800).map(|i| ((i as f32 * 0.013 * (k as f32 + 1.0)).sin()) * 0.3).collect();
            let src = crate::io::add_source(
                s,
                name,
                soundcraft_audio_io::AudioBuffer { sample_rate: 48_000, channels: vec![data] },
                None,
                soundcraft_audio_io::FileFormat::Wav,
            );
            let id = s.new_clip_id();
            let tr = s.track_mut(t).unwrap();
            tr.playlist_mut().unwrap().clips.push(Clip::audio(id, name, src, 0, 0, 4800));
            tr.mixer.pan = vec![pan];
            tr.mixer.volume_db = -2.0 * k as f32;
            ids.push(t);
        }
        (e, ids)
    }

    fn render(e: &Engine) -> Vec<Vec<f32>> {
        soundcraft_mix::render_range(e.session(), Range::new(0, 4800), 256)
    }

    fn max_diff(a: &[Vec<f32>], b: &[Vec<f32>]) -> f32 {
        a.iter().zip(b).flat_map(|(x, y)| x.iter().zip(y).map(|(p, q)| (p - q).abs())).fold(0.0, f32::max)
    }

    /// B-T1: bus from three Main-fed tracks renders the same within 1e-5, no duplicate path,
    /// one undo restores everything.
    #[test]
    fn bt1_create_bus_from_selection_is_transparent_and_one_undo() {
        let (mut e, ids) = three();
        let before_graph = e.session().clone();
        let a = render(&e);
        let r = e.execute("bus.create_from_selection", &json!({"tracks": ids.iter().map(|t| t.0).collect::<Vec<_>>(), "name": "Vocal Bus"})).unwrap();
        assert_eq!(r["output"], "main");
        let bus_track = TrackId(r["bus_track"].as_u64().unwrap());
        let b = render(&e);
        assert!(max_diff(&a, &b) <= 1e-5, "diff {}", max_diff(&a, &b));
        let s = e.session();
        assert!(ids.iter().all(|t| matches!(s.track(*t).unwrap().mixer.output, Route::Bus(_))), "sources moved, no parallel dry path");
        assert_eq!(s.track(bus_track).unwrap().name, "Vocal Bus");
        assert_eq!(s.edit.selected_tracks, vec![bus_track]);
        // Adjacent placement.
        let pos = s.tracks.iter().position(|t| t.id == bus_track).unwrap();
        assert_eq!(s.tracks[pos - 1].id, ids[2]);
        e.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(e.session().tracks, before_graph.tracks);
        assert_eq!(e.session().busses, before_graph.busses);
    }

    /// B-T2: mixed destinations need an explicit choice; nested ancestor selection is rejected;
    /// rejection leaves the exact graph.
    #[test]
    fn bt2_mixed_destinations_and_nested_selection() {
        let (mut e, ids) = three();
        let all: Vec<u64> = ids.iter().map(|t| t.0).collect();
        let r = e.execute("bus.create_from_selection", &json!({"tracks": [all[0]], "name": "Mix Bus"})).unwrap();
        let mixbus = r["bus_track"].as_u64().unwrap();
        let snapshot = e.session().clone();
        let err = e.execute("bus.create_from_selection", &json!({"tracks": [all[0], all[1]]})).unwrap_err().to_string();
        assert!(err.contains("different outputs"), "{err}");
        assert_eq!(e.session().tracks, snapshot.tracks);
        let err = e.execute("bus.create_from_selection", &json!({"tracks": [all[0], mixbus]})).unwrap_err().to_string();
        assert!(err.contains("already feeds"), "{err}");
        assert_eq!(e.session().tracks, snapshot.tracks);
        let r = e.execute("bus.create_from_selection", &json!({"tracks": [all[1], all[2]], "destination": "Mix Bus"})).unwrap();
        assert_eq!(r["output"], "Mix Bus");
    }

    /// B-T3: Vocal → Mix → Main nesting; cycles over outputs and sends fail atomically;
    /// save/reopen keeps graph, levels and ids.
    #[test]
    fn bt3_nesting_cycles_and_reload() {
        let (mut e, ids) = three();
        let all: Vec<u64> = ids.iter().map(|t| t.0).collect();
        let vocal = e.execute("bus.create_from_selection", &json!({"tracks": all, "name": "Vocal Bus"})).unwrap()["bus_track"].as_u64().unwrap();
        let mix = e.execute("bus.create_from_selection", &json!({"tracks": [vocal], "name": "Mix Bus"})).unwrap()["bus_track"].as_u64().unwrap();
        // A send from Mix Bus back into Vocal Bus would loop.
        let snapshot = e.session().clone();
        let r = e.execute("mix.send", &json!({"track": mix, "bus": "Vocal Bus", "level_db": 0.0}));
        if r.is_ok() {
            assert!(e.execute("bus.validate", &json!({})).unwrap()["ok"] == false, "cycle detected");
            e.execute("edit.undo", &json!({})).unwrap();
        }
        assert_eq!(e.session().tracks, snapshot.tracks);
        assert!(
            e.execute("bus.create_fx_return", &json!({"tracks": [mix], "destination": "Vocal Bus"})).is_err(),
            "FX return into an upstream bus is a cycle"
        );
        assert_eq!(e.session().tracks, snapshot.tracks);
        let json = serde_json::to_string(e.session()).unwrap();
        let back: soundcraft_model::Session = serde_json::from_str(&json).unwrap();
        assert_eq!(back.tracks, e.session().tracks);
        assert_eq!(back.busses, e.session().busses);
    }

    /// B-T4: post-fader send follows the source fader (-6 dB → 10^(-6/20)); pre-fader ignores it.
    #[test]
    fn bt4_send_scaling_post_and_pre_fader() {
        let (mut e, ids) = three();
        let lead = ids[0];
        for t in &ids[1..] {
            e.session_mut().track_mut(*t).unwrap().mixer.mute = true;
        }
        e.session_mut().track_mut(lead).unwrap().mixer.volume_db = 0.0;
        let r = e.execute("bus.create_fx_return", &json!({"tracks": [lead.0], "name": "Verb"})).unwrap();
        let ret = TrackId(r["return_track"].as_u64().unwrap());
        e.session_mut().track_mut(lead).unwrap().mixer.sends[0].as_mut().unwrap().level_db = 0.0;
        // Return only: mute the dry output by routing the source nowhere audible except the send.
        let measure = |e: &Engine| -> f32 {
            let mut s = e.session().clone();
            // Listen to the return alone: dry lead path muted via output to an unused bus.
            let dead = s.add_bus("dead", ChannelFormat::Stereo);
            s.track_mut(lead).unwrap().mixer.output = Route::Bus(dead);
            let out = soundcraft_mix::render_range(&s, Range::new(0, 4800), 256);
            out[0].iter().map(|x| x.abs()).fold(0.0, f32::max)
        };
        let a = measure(&e);
        assert!(a > 0.0);
        e.session_mut().track_mut(lead).unwrap().mixer.volume_db = -6.0;
        let b = measure(&e);
        assert!((b / a - 10f32.powf(-6.0 / 20.0)).abs() < 1e-5, "{}", b / a);
        e.session_mut().track_mut(lead).unwrap().mixer.sends[0].as_mut().unwrap().pre_fader = true;
        let c = measure(&e);
        assert!((c / a - 1.0).abs() < 1e-5, "pre-fader ignores the source fader: {}", c / a);
        let _ = ret;
    }

    /// B-T5: hide/show and spill are view-only (identical render); moving a track to a named bus
    /// changes only that edge.
    #[test]
    fn bt5_view_only_spill_and_single_edge_change() {
        let (mut e, ids) = three();
        let all: Vec<u64> = ids.iter().map(|t| t.0).collect();
        let bus =
            e.execute("bus.create_from_selection", &json!({"tracks": [all[0], all[1]], "name": "Vocal Bus"})).unwrap()["bus_track"].as_u64().unwrap();
        let a = render(&e);
        e.session_mut().track_mut(ids[2]).unwrap().hidden = true;
        let r = e.execute("bus.spill", &json!({"track": bus})).unwrap();
        assert_eq!(r["sources"].as_array().unwrap().len(), 2);
        assert!(e.session().track(ids[2]).unwrap().hidden);
        assert_eq!(max_diff(&a, &render(&e)), 0.0, "spill never changes audio");
        e.execute("bus.spill", &json!({})).unwrap();
        assert!(e.session().track(ids[2]).unwrap().hidden, "prior visibility restored");
        assert!(!e.session().track(ids[0]).unwrap().hidden);
        let before = e.session().clone();
        let Route::Bus(b) = e.session().track(TrackId(bus)).unwrap().mixer.input else { panic!() };
        e.session_mut().track_mut(ids[2]).unwrap().mixer.output = Route::Bus(b);
        let diff: Vec<_> = before.tracks.iter().zip(e.session().tracks.iter()).filter(|(x, y)| x != y).map(|(x, _)| x.id).collect();
        assert_eq!(diff, vec![ids[2]]);
    }

    /// B-T6: deleting a bus reroutes children to its parent, removes/reassigns sends, keeps
    /// sources/media; undo restores the exact graph.
    #[test]
    fn bt6_delete_bus_and_undo() {
        let (mut e, ids) = three();
        let all: Vec<u64> = ids.iter().map(|t| t.0).collect();
        let vocal =
            e.execute("bus.create_from_selection", &json!({"tracks": [all[0], all[1]], "name": "Vocal Bus"})).unwrap()["bus_track"].as_u64().unwrap();
        e.execute("bus.create_fx_return", &json!({"tracks": [all[2]], "name": "Verb"})).unwrap();
        e.execute("mix.send", &json!({"track": all[2], "slot": 1, "bus": "Vocal Bus", "level_db": -3.0})).unwrap();
        let before = e.session().clone();
        let r = e.execute("bus.delete", &json!({"track": vocal})).unwrap();
        assert_eq!(r["rerouted_sources"], 2);
        assert_eq!(r["sends_changed"], 1);
        let s = e.session();
        assert!(s.track(ids[0]).unwrap().mixer.output == Route::Main && s.track(ids[1]).unwrap().mixer.output == Route::Main);
        assert_eq!(s.tracks.len(), before.tracks.len() - 1);
        assert_eq!(s.sources.len(), before.sources.len(), "no media deleted");
        e.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(e.session().tracks, before.tracks);
        assert_eq!(e.session().busses, before.busses);
    }

    #[test]
    fn fx_return_rejects_bundled_plugins() {
        let (mut e, ids) = three();
        let err = e.execute("bus.create_fx_return", &json!({"tracks": [ids[0].0], "plugin": "plate_reverb"})).unwrap_err().to_string();
        assert!(err.contains("no bundled effects"), "{err}");
    }
}

#[cfg(test)]
mod missing_tests {
    use crate::Engine;
    use serde_json::json;
    use soundcraft_model::{ChannelFormat, Insert, TrackKind};

    /// R05/A1 migration: an old session's removed built-in stays as a named missing placeholder
    /// with its parameters through save/reopen; export requires explicit acknowledgment.
    #[test]
    fn missing_processors_survive_reload_and_block_silent_export() {
        let mut e = Engine::default();
        let t = e.session_mut().add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Vox"));
        let mut ins = Insert::new("compressor");
        ins.params.insert("ratio".into(), 4.0);
        ins.state = Some("AAEC".into());
        e.session_mut().track_mut(t).unwrap().mixer.inserts[1] = Some(ins.clone());
        let missing = e.execute("session.missing_plugins", &json!({})).unwrap();
        assert_eq!(missing[0]["plugin"], "compressor");
        let dir = std::env::temp_dir().join(format!("sml-missing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("old.scraft");
        e.execute("session.save", &json!({"path": path.to_string_lossy()})).unwrap();
        let mut f = Engine::default();
        f.execute("session.open", &json!({"path": path.to_string_lossy()})).unwrap();
        let back = f.session().track_by_name("Vox").unwrap().mixer.inserts[1].clone().unwrap();
        assert_eq!(back, ins, "id, parameters and state retained, nothing substituted");
        let wav = dir.join("mix.wav");
        let err = f.execute("file.bounce_mix", &json!({"path": wav.to_string_lossy(), "end": 4800})).unwrap_err().to_string();
        assert!(err.contains("acknowledge_missing"), "{err}");
        assert!(!wav.exists());
        f.execute("file.bounce_mix", &json!({"path": wav.to_string_lossy(), "end": 4800, "acknowledge_missing": true})).unwrap();
        assert!(wav.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
