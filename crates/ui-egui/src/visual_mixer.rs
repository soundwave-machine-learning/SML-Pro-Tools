//! Visual mixer (VISUAL_MIXER_SPEC VM01–VM05): a soundstage canvas over the SAME mixer state as
//! the Console strips. Horizontal node position = the channel's existing pan/balance parameter,
//! vertical = its existing fader gain, side handles = native Stereo Width. Every gesture runs the
//! same engine commands (`mix.pan`, `mix.volume`, `mix.width`) the Console uses, merged into one
//! undo step per pointer gesture; nothing here owns audio state. Hiding/filtering only changes
//! the view. Original SML presentation — no claim of another product's layout or DSP.

use crate::SoundApp;
use crate::theme::{Tokens, bold, regular};
use egui::{Align2, Color32, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::json;
use soundcraft_model::{Track, TrackId, TrackKind};

/// Visible vertical range of the canvas (dB). Values outside show an edge indicator; opening the
/// view never clamps a stored value.
pub const TOP_DB: f32 = 12.0;
pub const BOTTOM_DB: f32 = -60.0;

/// Mixer view mode, persisted per session in `edit.values["mixer.view"]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixView {
    Console,
    Visual,
    Split,
}

impl MixView {
    pub fn of(app: &SoundApp) -> MixView {
        match app.engine.session().edit.value("mixer.view", 0.0) as i64 {
            1 => MixView::Visual,
            2 => MixView::Split,
            _ => MixView::Console,
        }
    }
    fn code(self) -> f64 {
        match self {
            MixView::Console => 0.0,
            MixView::Visual => 1.0,
            MixView::Split => 2.0,
        }
    }
}

/// Node x for a channel: mono pan, balance (pan[0]) or the centre of a stereo pair.
pub fn node_x(t: &Track) -> f32 {
    match t.mixer.pan.as_slice() {
        [p] => *p,
        [l, r] if t.channels() == 2 => (l + r) / 2.0,
        _ => 0.0,
    }
}

/// Canvas y (0 = top) for a gain in dB.
pub fn db_to_y(db: f32, r: Rect) -> f32 {
    let d = if db.is_nan() { BOTTOM_DB } else { db.clamp(BOTTOM_DB, TOP_DB) };
    let t = ((d - BOTTOM_DB) / (TOP_DB - BOTTOM_DB)).clamp(0.0, 1.0);
    r.max.y - t * r.height()
}

pub fn y_to_db(y: f32, r: Rect) -> f32 {
    let t = ((r.max.y - y) / r.height().max(1.0)).clamp(0.0, 1.0);
    BOTTOM_DB + t * (TOP_DB - BOTTOM_DB)
}

pub fn x_to_pan(x: f32, r: Rect) -> f32 {
    (((x - r.min.x) / r.width().max(1.0)) * 2.0 - 1.0).clamp(-1.0, 1.0)
}

pub fn pan_to_x(p: f32, r: Rect) -> f32 {
    r.min.x + (p.clamp(-1.0, 1.0) + 1.0) / 2.0 * r.width()
}

/// Commands for moving a node's centre to `pan`: one `mix.pan` per panner index. Stereo pairs
/// keep their span (narrowed at the edges, VM02); the gesture's starting endpoints are passed in.
pub fn pan_commands(t: &Track, start: &[f32], pan: f32) -> Vec<serde_json::Value> {
    match start {
        [l, r] if t.channels() == 2 => {
            let ((nl, nr), _) = soundcraft_dsp::mixmath::drag_stereo_center((f64::from(*l), f64::from(*r)), f64::from(pan));
            vec![json!({"track": t.id.0, "pan": nl, "index": 0}), json!({"track": t.id.0, "pan": nr, "index": 1})]
        }
        _ => vec![json!({"track": t.id.0, "pan": pan, "index": 0})],
    }
}

fn visible_nodes(app: &SoundApp) -> Vec<Track> {
    let filter = app.ui.visual_filter.to_lowercase();
    app.engine
        .session()
        .tracks
        .iter()
        .filter(|t| !t.hidden && !matches!(t.kind, TrackKind::Master | TrackKind::Video | TrackKind::Vca | TrackKind::Folder))
        .filter(|t| filter.is_empty() || t.name.to_lowercase().contains(&filter))
        .cloned()
        .collect()
}

/// Header shared by Console/Visual/Split: view switch, snapshots and bus actions.
pub fn header(app: &mut SoundApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        let cur = MixView::of(app);
        for (v, label) in [(MixView::Console, "Console"), (MixView::Visual, "Visual"), (MixView::Split, "Split")] {
            if ui.selectable_label(cur == v, label).clicked() && cur != v {
                let _ = app.run("mix.view", json!({"view": v.code()}));
            }
        }
        ui.separator();
        ui.label("Snapshot");
        for k in ["A", "B", "C"] {
            let has = app.engine.session().mix_snapshots.contains_key(k);
            let resp = ui.button(if has { k.to_string() } else { format!("{k}·") }).on_hover_text(format!(
                "Click: recall {k} (stopped). Shift+click: save {k}.\nScope: {}\nExcluded: {}",
                "gain, pan, width, mute, solo, visibility", "plugins, slots, routing, trims, automation"
            ));
            if resp.clicked() {
                let save = ui.input(|i| i.modifiers.shift) || !has;
                let id = if save { "snapshot.save" } else { "snapshot.recall" };
                if let Err(e) = app.run(id, json!({"slot": k})) {
                    app.ui.status = e;
                }
            }
        }
        ui.separator();
        if ui.button("Create Bus from Selection").clicked() {
            match app.run("bus.create_from_selection", json!({"name": "Bus"})) {
                Ok(_) => app.ui.status = "Bus created from the selected channels".into(),
                Err(e) => app.ui.status = e,
            }
        }
        if ui.button("Create FX Return").clicked() {
            match app.run("bus.create_fx_return", json!({"name": "FX Return"})) {
                Ok(_) => app.ui.status = "FX return created; raise the sends to hear it".into(),
                Err(e) => app.ui.status = e,
            }
        }
        if cur != MixView::Console {
            ui.separator();
            ui.add(egui::TextEdit::singleline(&mut app.ui.visual_filter).hint_text("Filter channels").desired_width(120.0));
        }
    });
}

/// The soundstage canvas.
pub fn canvas(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let nodes = visible_nodes(app);
    let avail = ui.available_size();
    let (outer, _) = ui.allocate_exact_size(vec2(avail.x.max(200.0), avail.y.max(200.0)), Sense::hover());
    let r = outer.shrink2(vec2(40.0, 24.0));
    let p = ui.painter_at(outer);
    p.rect_filled(outer, 4.0, t.strip_bg);
    // Guides: centre, 0 dB, edges.
    p.line_segment([pos2(pan_to_x(0.0, r), r.min.y), pos2(pan_to_x(0.0, r), r.max.y)], Stroke::new(1.0, t.strip_section));
    let y0 = db_to_y(0.0, r);
    p.line_segment([pos2(r.min.x, y0), pos2(r.max.x, y0)], Stroke::new(1.0, t.strip_section));
    p.text(pos2(r.min.x - 4.0, y0), Align2::RIGHT_CENTER, "0 dB", regular(10.0), t.counter_label);
    p.text(pos2(r.min.x, r.max.y + 12.0), Align2::LEFT_CENTER, "L", bold(11.0), t.counter_label);
    p.text(pos2(r.max.x, r.max.y + 12.0), Align2::RIGHT_CENTER, "R", bold(11.0), t.counter_label);
    p.rect_stroke(r, 0.0, Stroke::new(1.0, t.strip_section), StrokeKind::Inside);
    let selected = app.engine.session().edit.selected_tracks.clone();
    let meters = app.meters.clone();
    for tr in &nodes {
        let x = pan_to_x(node_x(tr), r);
        let db = tr.mixer.volume_db;
        let y = db_to_y(db, r);
        let width = tr.mixer.width;
        let stereo = tr.channels() == 2;
        let half = if stereo { 10.0 + 18.0 * width } else { 8.0 };
        let node = Rect::from_center_size(pos2(x, y), vec2(half * 2.0, 18.0));
        let id = ui.id().with(("vm-node", tr.id.0));
        let resp = ui.interact(node, id, Sense::click_and_drag());
        let is_sel = selected.contains(&tr.id);
        let c = Color32::from_rgb(tr.color[0], tr.color[1], tr.color[2]);
        let level = meters.get(&tr.id).map_or(0.0, |m| m.level[0].max(m.level[1]));
        p.rect_filled(node, 6.0, c.gamma_multiply(0.35 + 0.65 * level.clamp(0.0, 1.0)));
        p.rect_stroke(node, 6.0, Stroke::new(if is_sel { 2.5 } else { 1.0 }, if is_sel { Color32::WHITE } else { c }), StrokeKind::Outside);
        let mut label = tr.name.clone();
        if tr.mixer.mute {
            label.push_str(" [M]");
        }
        if tr.mixer.solo {
            label.push_str(" [S]");
        }
        p.text(pos2(x, node.min.y - 8.0), Align2::CENTER_BOTTOM, &label, regular(11.0), t.counter_text);
        // Off-canvas values: edge indicator, value untouched.
        if !(BOTTOM_DB..=TOP_DB).contains(&db) {
            p.text(pos2(x, node.center().y), Align2::CENTER_CENTER, if db > TOP_DB { "▲" } else { "▼" }, bold(11.0), Color32::YELLOW);
        }
        let readout = format!(
            "{} | {} | W {:.0}%",
            if db <= -144.0 { "-inf dB".to_string() } else { format!("{db:.1} dB") },
            pan_label(node_x(tr)),
            width * 100.0
        );
        let resp = resp.on_hover_text(format!(
            "{readout}\nDrag: pan (x) / fader (y). Shift: lock axis. Alt: fine. Double-click: reset to 0 dB / centre. Esc: cancel drag."
        ));
        if resp.clicked() {
            // Ctrl/Cmd+click toggles the node in a multi-selection; plain click selects it alone.
            let toggle = ui.input(|i| i.modifiers.command);
            let mut sel: Vec<u64> = if toggle { selected.iter().map(|t| t.0).collect() } else { Vec::new() };
            if let Some(i) = sel.iter().position(|x| *x == tr.id.0) {
                sel.remove(i);
            } else {
                sel.push(tr.id.0);
            }
            let _ = app.run("edit.select", json!({"tracks": sel}));
        }
        if resp.double_clicked() {
            let _ = app.run("mix.volume", json!({"track": tr.id.0, "db": 0.0}));
            for cmd in pan_commands(tr, &[if stereo { -1.0 } else { 0.0 }, 1.0], 0.0) {
                let _ = app.run("mix.pan", cmd);
            }
        }
        node_drag(app, ui, &resp, tr, r);
        if stereo {
            width_handles(app, ui, tr, node);
        }
    }
    if ui.input(|i| i.key_pressed(egui::Key::Escape))
        && let Some(g) = app.ui.visual_gesture.take()
    {
        // Cancel: restore every member's starting values inside the same merged step.
        let key = format!("visual:{}", g.track.0);
        for (id, db, pan) in &g.members {
            let _ = app.engine.execute_merged("mix.volume", &json!({"track": id.0, "db": db}), &key);
            for (i, v) in pan.iter().enumerate() {
                let _ = app.engine.execute_merged("mix.pan", &json!({"track": id.0, "pan": v, "index": i}), &key);
            }
        }
        app.engine.end_merge();
    }
}

fn pan_label(p: f32) -> String {
    if p.abs() < 0.005 {
        "C".into()
    } else if p < 0.0 {
        format!("L{:.0}", -p * 100.0)
    } else {
        format!("R{:.0}", p * 100.0)
    }
}

/// Gesture bookkeeping for one node drag.
#[derive(Debug, Clone)]
pub struct VisualGesture {
    pub track: TrackId,
    pub start_db: f32,
    pub start_pan: Vec<f32>,
    pub start_pos: egui::Pos2,
    /// Every node moved by this gesture (the dragged one plus the rest of a multi-selection):
    /// (id, start dB, start pan endpoints).
    pub members: Vec<(TrackId, f32, Vec<f32>)>,
}

/// Engine fader limits (dB).
pub const FADER_MIN_DB: f32 = -144.0;
pub const FADER_MAX_DB: f32 = 12.0;

/// VM02 group vertical drag: one common dB delta for every member, clamped so no member leaves
/// the engine range (relative finite-dB offsets are preserved). A member at −∞ has no finite
/// offset, so a multi-member vertical drag containing one is rejected.
pub fn common_db_delta(starts: &[f32], want: f32) -> Result<f32, &'static str> {
    if starts.len() > 1 && starts.iter().any(|s| *s <= FADER_MIN_DB) {
        return Err("a selected channel is at -inf: set its level numerically, then drag the group");
    }
    let lo = starts.iter().map(|s| FADER_MIN_DB - s).fold(f32::NEG_INFINITY, f32::max);
    let hi = starts.iter().map(|s| FADER_MAX_DB - s).fold(f32::INFINITY, f32::min);
    Ok(if want.is_finite() { want.clamp(lo, hi.max(lo)) } else { 0.0 })
}

/// VM02 group horizontal drag: a common pan delta keeping every mono member inside [-1, 1]
/// (stereo members narrow their own span at the edges instead).
pub fn common_pan_delta(mono_starts: &[f32], want: f32) -> f32 {
    let lo = mono_starts.iter().map(|p| -1.0 - p).fold(-2.0f32, f32::max);
    let hi = mono_starts.iter().map(|p| 1.0 - p).fold(2.0f32, f32::min);
    if want.is_finite() { want.clamp(lo, hi.max(lo)) } else { 0.0 }
}

fn node_drag(app: &mut SoundApp, ui: &Ui, resp: &egui::Response, tr: &Track, r: Rect) {
    if resp.drag_started() {
        let sel = app.engine.session().edit.selected_tracks.clone();
        let members: Vec<(TrackId, f32, Vec<f32>)> = if sel.contains(&tr.id) && sel.len() > 1 {
            sel.iter()
                .filter_map(|id| app.engine.session().track(*id))
                .filter(|t| t.kind != TrackKind::Master)
                .map(|t| (t.id, t.mixer.volume_db, t.mixer.pan.clone()))
                .collect()
        } else {
            vec![(tr.id, tr.mixer.volume_db, tr.mixer.pan.clone())]
        };
        app.ui.visual_gesture = Some(VisualGesture {
            track: tr.id,
            start_db: tr.mixer.volume_db,
            start_pan: tr.mixer.pan.clone(),
            start_pos: resp.interact_pointer_pos().unwrap_or_default(),
            members,
        });
    }
    if resp.dragged()
        && let Some(g) = app.ui.visual_gesture.clone()
        && g.track == tr.id
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let m = ui.input(|i| i.modifiers);
        let fine = if m.alt { 0.2 } else { 1.0 };
        let d = (pos - g.start_pos) * fine;
        let (lock_x, lock_y) = if m.shift { (d.y.abs() > d.x.abs(), d.x.abs() >= d.y.abs()) } else { (false, false) };
        let key = format!("visual:{}", tr.id.0);
        let group = g.members.len() > 1;
        if !lock_y {
            // -inf has no finite dB offset: vertical drag from -inf starts at the bottom.
            let base_y = db_to_y(g.start_db.max(BOTTOM_DB), r);
            let db = y_to_db(base_y + d.y, r);
            let db = if db <= BOTTOM_DB + 0.01 { g.start_db.min(BOTTOM_DB) } else { (db * 10.0).round() / 10.0 };
            if group {
                let starts: Vec<f32> = g.members.iter().map(|m| m.1).collect();
                match common_db_delta(&starts, db - g.start_db) {
                    Ok(delta) => {
                        for (id, s, _) in &g.members {
                            let _ = app.engine.execute_merged("mix.volume", &json!({"track": id.0, "db": s + delta}), &key);
                        }
                    }
                    Err(e) => app.ui.status = e.into(),
                }
            } else {
                let _ = app.engine.execute_merged("mix.volume", &json!({"track": tr.id.0, "db": db}), &key);
            }
        }
        if !lock_x {
            let start_center = node_x(&Track { mixer: soundcraft_model::Mixer { pan: g.start_pan.clone(), ..tr.mixer.clone() }, ..tr.clone() });
            let pan = x_to_pan(pan_to_x(start_center, r) + d.x, r);
            if group {
                let monos: Vec<f32> = g.members.iter().filter(|m| m.2.len() == 1).map(|m| m.2[0]).collect();
                let delta = common_pan_delta(&monos, pan - start_center);
                for (id, _, sp) in &g.members {
                    let Some(mt) = app.engine.session().track(*id).cloned() else { continue };
                    let c0 = node_x(&Track { mixer: soundcraft_model::Mixer { pan: sp.clone(), ..mt.mixer.clone() }, ..mt.clone() });
                    for cmd in pan_commands(&mt, sp, c0 + delta) {
                        let _ = app.engine.execute_merged("mix.pan", &cmd, &key);
                    }
                }
            } else {
                for cmd in pan_commands(tr, &g.start_pan, pan) {
                    let _ = app.engine.execute_merged("mix.pan", &cmd, &key);
                }
            }
        }
    }
    if resp.drag_stopped() {
        app.ui.visual_gesture = None;
        app.engine.end_merge();
    }
}

fn width_handles(app: &mut SoundApp, ui: &Ui, tr: &Track, node: Rect) {
    for (side, x) in [(-1.0f32, node.min.x), (1.0, node.max.x)] {
        let h = Rect::from_center_size(pos2(x, node.center().y), vec2(8.0, 18.0));
        let id = ui.id().with(("vm-width", tr.id.0, side as i32));
        let resp = ui.interact(h, id, Sense::drag()).on_hover_text(format!("Stereo Width {:.0}% (drag)", tr.mixer.width * 100.0));
        ui.painter().rect_filled(h, 2.0, if resp.hovered() || resp.dragged() { Color32::WHITE } else { Color32::GRAY });
        if resp.dragged() {
            let dx = resp.drag_delta().x * side;
            let w = (tr.mixer.width + dx / 36.0).clamp(0.0, 2.0);
            let w = if (w - 1.0).abs() < 0.02 { 1.0 } else { w };
            let _ = app.engine.execute_merged("mix.width", &json!({"tracks": [tr.id.0], "width": w}), &format!("width:{}", tr.id.0));
        }
        if resp.drag_stopped() {
            app.engine.end_merge();
        }
    }
}

/// Channel list beside the canvas: select, hide/show (view only), numeric gain/pan/width.
pub fn channel_list(app: &mut SoundApp, ui: &mut Ui) {
    let tracks: Vec<Track> = app
        .engine
        .session()
        .tracks
        .iter()
        .filter(|t| !matches!(t.kind, TrackKind::Master | TrackKind::Video | TrackKind::Vca | TrackKind::Folder))
        .cloned()
        .collect();
    egui::ScrollArea::vertical().id_salt("vm-list").show(ui, |ui| {
        for tr in tracks {
            ui.horizontal(|ui| {
                let mut vis = !tr.hidden;
                if ui.checkbox(&mut vis, "").on_hover_text("Show in the mixer (view only; never mutes)").changed() {
                    let _ = app.run("track.hide", json!({"tracks": [tr.id.0], "value": !vis}));
                }
                let sel = app.engine.session().edit.selected_tracks.contains(&tr.id);
                if ui.selectable_label(sel, &tr.name).clicked() {
                    let _ = app.run("edit.select", json!({"tracks": [tr.id.0]}));
                }
                let mut db = tr.mixer.volume_db;
                if ui.add(egui::DragValue::new(&mut db).range(-144.0..=12.0).speed(0.1).suffix(" dB").max_decimals(1)).changed() {
                    let _ = app.engine.execute_merged("mix.volume", &json!({"track": tr.id.0, "db": db}), &format!("vmlist:{}", tr.id.0));
                }
                if tr.channels() == 2 {
                    let mut w = tr.mixer.width * 100.0;
                    if ui.add(egui::DragValue::new(&mut w).range(0.0..=200.0).speed(1.0).suffix(" %W")).changed() {
                        let _ = app.engine.execute_merged(
                            "mix.width",
                            &json!({"tracks": [tr.id.0], "width": w / 100.0}),
                            &format!("vmlistw:{}", tr.id.0),
                        );
                    }
                }
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinate_mappings_round_trip_and_never_clamp_stored_values() {
        let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, 720.0));
        for db in [-60.0f32, -12.0, 0.0, 6.0, 12.0] {
            assert!((y_to_db(db_to_y(db, r), r) - db).abs() < 1e-3);
        }
        for p in [-1.0f32, -0.5, 0.0, 0.25, 1.0] {
            assert!((x_to_pan(pan_to_x(p, r), r) - p).abs() < 1e-5);
        }
        // +18 dB is drawn at the top edge but the mapping is not used to rewrite it.
        assert_eq!(db_to_y(18.0, r), r.min.y);
    }

    /// VM-T4 group rules: common delta keeps relative offsets, clamps at the engine limits,
    /// rejects a −∞ member for vertical moves; pan delta keeps mono members in range.
    #[test]
    fn group_drag_deltas() {
        assert_eq!(common_db_delta(&[-6.0, -12.0], 3.0), Ok(3.0));
        assert_eq!(common_db_delta(&[6.0, -12.0], 10.0), Ok(6.0), "clamped by the member nearest +12");
        assert!(common_db_delta(&[-6.0, -144.0], 3.0).is_err());
        assert_eq!(common_db_delta(&[-144.0], 6.0), Ok(6.0), "single node may leave -inf");
        assert_eq!(common_pan_delta(&[0.5, -0.2], 0.8), 0.5);
        assert_eq!(common_pan_delta(&[0.5, -0.2], -1.0), -0.8);
        assert_eq!(common_pan_delta(&[], 0.3), 0.3);
    }

    #[test]
    fn stereo_drag_keeps_span_and_narrows_at_edges() {
        let mut s = soundcraft_model::Session::default();
        let id = s.add_track(TrackKind::Audio, soundcraft_model::ChannelFormat::Stereo, None);
        let t = s.track(id).unwrap().clone();
        let cmds = pan_commands(&t, &[-1.0, 1.0], 0.5);
        assert_eq!(cmds[0]["pan"], 0.0);
        assert_eq!(cmds[1]["pan"], 1.0);
        let mono = s.add_track(TrackKind::Audio, soundcraft_model::ChannelFormat::Mono, None);
        let m = s.track(mono).unwrap().clone();
        assert_eq!(pan_commands(&m, &[0.0], -0.3).len(), 1);
    }
}
