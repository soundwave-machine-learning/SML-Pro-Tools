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
    let t = crate::theme::Tokens::current();
    egui::Frame::NONE.inner_margin(egui::Margin::symmetric(4, 3)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = crate::theme::metric::GAP;
            let cur = MixView::of(app);
            // View tabs: one segmented group; the active view is filled.
            crate::theme::group_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for (v, label) in [(MixView::Console, "Console"), (MixView::Visual, "Visual"), (MixView::Split, "Split")] {
                        if crate::control_bar::chip(ui, None, label, label, cur == v, t.accent, true).clicked() && cur != v {
                            let _ = app.run("mix.view", json!({"view": v.code()}));
                        }
                    }
                });
            });
            // Strip density (Console and Split): one persisted UI preference, the same one as
            // View › Narrow Mix (`window.narrow_mix`) and `ui.mix_density`.
            if cur != MixView::Visual {
                crate::theme::group_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for (compact, label) in [(false, "Comfortable"), (true, "Compact")] {
                            let on = app.ui.narrow_mix == compact;
                            let r = crate::control_bar::chip(ui, None, label, &format!("{label} density"), on, t.accent, true).on_hover_text(
                                if compact {
                                    "Compact: narrower strips, short captions — more channels on screen"
                                } else {
                                    "Comfortable: wider strips with full captions and values"
                                },
                            );
                            if r.clicked() && !on {
                                let _ = app.run("ui.mix_density", json!({"density": if compact { "compact" } else { "comfortable" }}));
                            }
                        }
                    });
                });
            }
            ui.label(egui::RichText::new("Snapshots").font(crate::theme::regular(crate::theme::size::SMALL)).color(t.text_dim));
            for k in ["A", "B", "C"] {
                let has = app.engine.session().mix_snapshots.contains_key(k);
                // Stored slots are solid; empty slots are drawn dim and say so (clicking one stores).
                let text = if has { k.to_string() } else { format!("{k}·") };
                let resp = crate::control_bar::chip(ui, None, &text, &text, has, t.button_hi, true).on_hover_text(format!(
                    "{}Click: recall {k} (stopped). Shift+click: save {k}.\nScope: {}\nExcluded: {}",
                    if has { "" } else { "Empty — a click stores the current mix here.\n" },
                    "gain, pan, width, mute, solo, visibility",
                    "plugins, slots, routing, trims, automation"
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
            if ui.button("Create Bus from Selection").on_hover_text("Route the selected channels to a new bus (one undo step)").clicked() {
                match app.run("bus.create_from_selection", json!({"name": "Bus"})) {
                    Ok(_) => app.ui.status = "Bus created from the selected channels".into(),
                    Err(e) => app.ui.status = e,
                }
            }
            if ui.button("Create FX Return").on_hover_text("A new aux return fed by sends from the selected channels (one undo step)").clicked() {
                match app.run("bus.create_fx_return", json!({"name": "FX Return"})) {
                    Ok(_) => app.ui.status = "FX return created; raise the sends to hear it".into(),
                    Err(e) => app.ui.status = e,
                }
            }
            if cur != MixView::Console {
                ui.separator();
                ui.add(egui::TextEdit::singleline(&mut app.ui.visual_filter).hint_text("Filter channels").desired_width(120.0));
            }
        })
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
    // Geometry comes only from gain, pan and width; nothing drawn here writes them back.
    let rects: Vec<Rect> = nodes.iter().map(|tr| node_rect(tr, r)).collect();
    // Z-order: unselected first, selected last — the topmost node gets the pointer, so a channel
    // chosen from a cluster list is the one a drag at that spot moves.
    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by_key(|&i| selected.contains(&nodes[i].id));
    let mut hovered: Option<usize> = None;
    for &i in &order {
        let tr = &nodes[i];
        let node = rects[i];
        let x = node.center().x;
        let db = tr.mixer.volume_db;
        let width = tr.mixer.width;
        let stereo = tr.channels() == 2;
        let id = ui.id().with(("vm-node", tr.id.0));
        let resp = ui.interact(node, id, Sense::click_and_drag());
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected.contains(&tr.id), format!("{} node", tr.name)));
        let is_sel = selected.contains(&tr.id);
        let c = Color32::from_rgb(tr.color[0], tr.color[1], tr.color[2]);
        let level = meters.get(&tr.id).map_or(0.0, |m| m.level[0].max(m.level[1]));
        p.rect_filled(node, 6.0, c.gamma_multiply(0.35 + 0.65 * level.clamp(0.0, 1.0)));
        p.rect_stroke(node, 6.0, Stroke::new(if is_sel { 2.5 } else { 1.0 }, if is_sel { t.selection_stroke } else { c }), StrokeKind::Outside);
        // Off-canvas values: edge indicator, value untouched.
        if !(BOTTOM_DB..=TOP_DB).contains(&db) {
            p.text(pos2(x, node.center().y), Align2::CENTER_CENTER, if db > TOP_DB { "▲" } else { "▼" }, bold(11.0), t.warning);
        }
        let resp = resp.on_hover_text(format!(
            "{}\nDrag: pan (x) / fader (y). Shift: lock axis. Alt: fine. Double-click: reset to 0 dB / centre. Esc: cancel drag.",
            node_readout(tr)
        ));
        if resp.hovered() {
            hovered = Some(i);
        }
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
        let _ = width;
    }
    // Labels: one per channel, or one chip per cluster of coincident nodes, placed where they
    // overlap neither another label nor a node.
    let clusters = clusters(&rects);
    let mut items: Vec<(Vec<usize>, String)> = clusters
        .iter()
        .map(|members| {
            let mut m = members.clone();
            m.sort_by_key(|&i| (!selected.contains(&nodes[i].id), nodes[i].name.to_lowercase()));
            let text = if m.len() == 1 { label_text(&nodes[m[0]]) } else { format!("{} +{}", nodes[m[0]].name, m.len() - 1) };
            (m, text)
        })
        .collect();
    // Priority: clusters holding a selected channel, then bigger clusters, then top to bottom.
    items.sort_by(|a, b| {
        let sa = a.0.iter().any(|&i| selected.contains(&nodes[i].id));
        let sb = b.0.iter().any(|&i| selected.contains(&nodes[i].id));
        sb.cmp(&sa).then(b.0.len().cmp(&a.0.len())).then(rects[a.0[0]].min.y.total_cmp(&rects[b.0[0]].min.y))
    });
    let font = regular(11.0);
    let sizes: Vec<egui::Vec2> =
        items.iter().map(|(_, text)| p.layout_no_wrap(text.clone(), font.clone(), t.text).size() + vec2(10.0, 4.0)).collect();
    let anchors: Vec<Rect> = items.iter().map(|(m, _)| m.iter().fold(rects[m[0]], |acc, &i| acc.union(rects[i]))).collect();
    // Labels keep clear of the status band at the top of the canvas.
    let label_bounds = Rect::from_min_max(pos2(outer.min.x + 2.0, outer.min.y + 22.0), outer.max - vec2(2.0, 2.0));
    let placed = place_labels(&anchors, &sizes, &rects, label_bounds);
    for (k, (members, text)) in items.iter().enumerate() {
        let Some(lr) = placed[k] else { continue };
        let anchor = anchors[k];
        let sel = members.iter().any(|&i| selected.contains(&nodes[i].id));
        // A leader line when the label had to move away from its node.
        if lr.min.y > anchor.max.y + 8.0 || lr.max.y < anchor.min.y - 8.0 || lr.min.x > anchor.max.x + 16.0 || lr.max.x < anchor.min.x - 16.0 {
            let to = pos2(anchor.center().x, if lr.center().y < anchor.center().y { anchor.min.y } else { anchor.max.y });
            p.line_segment([lr.center(), to], Stroke::new(1.0, t.text_dim));
        }
        p.rect(lr, 3.0, t.panel_bg2, Stroke::new(1.0, if sel { t.selection_stroke } else { t.border_light }), StrokeKind::Inside);
        p.text(lr.center(), Align2::CENTER_CENTER, text, font.clone(), if sel { t.text } else { t.text_dim.lerp_to_gamma(t.text, 0.5) });
        let lid = ui.id().with(("vm-label", nodes[members[0]].id.0));
        let names: Vec<&str> = members.iter().map(|&i| nodes[i].name.as_str()).collect();
        let lresp = ui.interact(lr, lid, Sense::click());
        if members.len() == 1 {
            let i = members[0];
            lresp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("{} label", nodes[i].name)));
            let lresp = lresp.on_hover_text(format!("{}\nClick to select.", node_readout(&nodes[i])));
            if lresp.hovered() {
                hovered = Some(i);
            }
            if lresp.clicked() {
                let _ = app.run("edit.select", json!({"tracks": [nodes[i].id.0]}));
            }
        } else {
            // Coincident channels: the chip lists every one with its values; choosing one
            // selects it (and puts it on top, so a drag here moves it).
            let summary = format!("{} channels at this position: {}", members.len(), names.join(", "));
            lresp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, summary.clone()));
            let lresp = lresp
                .on_hover_text(format!("{}\nClick to choose one.", members.iter().map(|&i| node_readout(&nodes[i])).collect::<Vec<_>>().join("\n")));
            egui::Popup::menu(&lresp).show(|ui| {
                ui.label(egui::RichText::new("CHANNELS AT THIS POSITION").font(bold(crate::theme::size::CAPTION)).color(t.section_label));
                for &i in members {
                    let tr = &nodes[i];
                    let is_sel = selected.contains(&tr.id);
                    if ui.selectable_label(is_sel, node_readout(tr)).on_hover_text("Select this channel").clicked() {
                        let _ = app.run("edit.select", json!({"tracks": [tr.id.0]}));
                        ui.close();
                    }
                }
            });
        }
    }
    // Readable details: a card beside the hovered node, and the selection on the bottom line.
    if let Some(i) = hovered {
        detail_card(&p, &nodes[i], rects[i], outer, ui.ctx().pixels_per_point());
    }
    let sel_nodes: Vec<&Track> = nodes.iter().filter(|n| selected.contains(&n.id)).collect();
    let line = match sel_nodes.as_slice() {
        [] => "Click a node or label to select · labels of channels at the same position are grouped — click one to choose".to_string(),
        [one] => format!("Selected: {}", node_readout(one)),
        many => format!("{} selected: {}", many.len(), many.iter().map(|n| n.name.as_str()).collect::<Vec<_>>().join(", ")),
    };
    // In the free band above the stage (the L/R and 0 dB marks stay clear).
    p.text(pos2(outer.min.x + 8.0, outer.min.y + 5.0), Align2::LEFT_TOP, line, regular(11.0), t.text_dim);
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

/// A node's rectangle on the canvas: x from pan, y from gain, width from Stereo Width.
pub fn node_rect(tr: &Track, r: Rect) -> Rect {
    let x = pan_to_x(node_x(tr), r);
    let y = db_to_y(tr.mixer.volume_db, r);
    let half = if tr.channels() == 2 { 10.0 + 18.0 * tr.mixer.width } else { 8.0 };
    Rect::from_center_size(pos2(x, y), vec2(half * 2.0, 18.0))
}

/// "−3.0 dB · C · W 100% · MUTE" — the values a node stands for.
pub fn node_readout(tr: &Track) -> String {
    let db = tr.mixer.volume_db;
    let mut s = format!("{} — {} · {}", tr.name, if db <= -144.0 { "-inf dB".to_string() } else { format!("{db:.1} dB") }, pan_label(node_x(tr)));
    if tr.channels() == 2 {
        s.push_str(&format!(" · W {:.0}%", tr.mixer.width * 100.0));
    }
    if tr.mixer.mute {
        s.push_str(" · MUTE");
    }
    if tr.mixer.solo {
        s.push_str(" · SOLO");
    }
    s
}

fn label_text(tr: &Track) -> String {
    let mut label = tr.name.clone();
    if tr.mixer.mute {
        label.push_str(" [M]");
    }
    if tr.mixer.solo {
        label.push_str(" [S]");
    }
    label
}

/// Groups of nodes whose rectangles overlap (directly or through a chain): they share one label.
pub fn clusters(rects: &[Rect]) -> Vec<Vec<usize>> {
    let mut group: Vec<usize> = (0..rects.len()).collect();
    fn find(g: &mut [usize], i: usize) -> usize {
        let mut i = i;
        while g[i] != i {
            g[i] = g[g[i]];
            i = g[i];
        }
        i
    }
    for a in 0..rects.len() {
        for b in a + 1..rects.len() {
            if rects[a].intersects(rects[b]) {
                let (ra, rb) = (find(&mut group, a), find(&mut group, b));
                group[ra] = rb;
            }
        }
    }
    let mut out: Vec<Vec<usize>> = Vec::new();
    let mut root_of: Vec<(usize, usize)> = Vec::new();
    for i in 0..rects.len() {
        let root = find(&mut group, i);
        match root_of.iter().find(|(r, _)| *r == root) {
            Some((_, k)) => out[*k].push(i),
            None => {
                root_of.push((root, out.len()));
                out.push(vec![i]);
            }
        }
    }
    out
}

/// Collision-aware label placement. For each anchor (in priority order) try above, below,
/// right and left of it, then further above/below (drawn with a leader line). A slot is taken
/// only if it stays inside `bounds` and overlaps no placed label and no node. `None`: no room
/// (the channel is still in the list beside the canvas and in its node's tooltip).
pub fn place_labels(anchors: &[Rect], sizes: &[egui::Vec2], nodes: &[Rect], bounds: Rect) -> Vec<Option<Rect>> {
    let gap = 3.0;
    let mut placed: Vec<Rect> = Vec::new();
    let mut out = Vec::with_capacity(anchors.len());
    for (a, size) in anchors.iter().zip(sizes) {
        let (w, h) = (size.x, size.y);
        let c = a.center();
        let mut candidates = vec![
            Rect::from_center_size(pos2(c.x, a.min.y - gap - h / 2.0), *size),
            Rect::from_center_size(pos2(c.x, a.max.y + gap + h / 2.0), *size),
            Rect::from_min_size(pos2(a.max.x + gap + 6.0, c.y - h / 2.0), *size),
            Rect::from_min_size(pos2(a.min.x - gap - 6.0 - w, c.y - h / 2.0), *size),
        ];
        for k in 1..=6 {
            let dy = k as f32 * (h + gap);
            for dx in [0.0, -(w / 2.0 + gap), w / 2.0 + gap] {
                candidates.push(Rect::from_center_size(pos2(c.x + dx, a.min.y - gap - h / 2.0 - dy), *size));
                candidates.push(Rect::from_center_size(pos2(c.x + dx, a.max.y + gap + h / 2.0 + dy), *size));
            }
        }
        let free = |r: &Rect| {
            bounds.contains_rect(*r) && !placed.iter().any(|p| p.expand(1.0).intersects(*r)) && !nodes.iter().any(|n| n.expand(4.0).intersects(*r))
        };
        let slot = candidates.into_iter().find(free);
        if let Some(r) = slot {
            placed.push(r);
        }
        out.push(slot);
    }
    out
}

/// The card beside a hovered node: name and values in readable type on a solid surface.
fn detail_card(p: &egui::Painter, tr: &Track, node: Rect, bounds: Rect, _ppp: f32) {
    let t = Tokens::current();
    let title = p.layout_no_wrap(tr.name.clone(), bold(12.0), t.text);
    let db = tr.mixer.volume_db;
    let mut lines = vec![
        format!("Gain   {}", if db <= -144.0 { "-inf dB".to_string() } else { format!("{db:.1} dB") }),
        format!("Pan    {}", pan_label(node_x(tr))),
    ];
    if tr.channels() == 2 {
        lines.push(format!("Width  {:.0}%", tr.mixer.width * 100.0));
    }
    let states: Vec<&str> =
        [(tr.mixer.mute, "MUTE"), (tr.mixer.solo, "SOLO"), (tr.mixer.record_arm, "REC")].iter().filter(|s| s.0).map(|s| s.1).collect();
    if !states.is_empty() {
        lines.push(states.join(" · "));
    }
    let body: Vec<_> = lines.iter().map(|l| p.layout_no_wrap(l.clone(), crate::theme::mono(11.0), t.text)).collect();
    let w = body.iter().map(|g| g.size().x).fold(title.size().x, f32::max) + 16.0;
    let h = title.size().y + body.iter().map(|g| g.size().y + 1.0).sum::<f32>() + 12.0;
    let mut card = Rect::from_min_size(pos2(node.max.x + 14.0, node.center().y - h / 2.0), vec2(w, h));
    if card.max.x > bounds.max.x {
        card = card.translate(vec2(-(card.width() + node.width() + 28.0), 0.0));
    }
    let card = card.translate(vec2(0.0, (bounds.min.y + 2.0 - card.min.y).max(0.0) + (bounds.max.y - 2.0 - card.max.y).min(0.0)));
    p.rect(card, 4.0, t.panel_bg2, Stroke::new(1.0, t.border_light), StrokeKind::Inside);
    let mut y = card.min.y + 6.0;
    let th = title.size().y;
    p.galley(pos2(card.min.x + 8.0, y), title, t.text);
    y += th + 2.0;
    for g in body {
        let gh = g.size().y;
        p.galley(pos2(card.min.x + 8.0, y), g, t.text);
        y += gh + 1.0;
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
/// VM01 list badge: where the channel goes ("→ Main", "→ Vox", "→ none") plus active sends.
pub fn route_badge(s: &soundcraft_model::Session, t: &Track) -> String {
    use soundcraft_model::Route;
    let out = match &t.mixer.output {
        Route::None => "none".to_string(),
        Route::Main => "Main".to_string(),
        Route::Bus(b) => s.bus(*b).map_or_else(|| "missing bus".to_string(), |b| b.name.clone()),
        Route::Hardware(h) => h.clone(),
    };
    let sends = t.mixer.sends.iter().flatten().filter(|x| !x.mute).count();
    if sends > 0 { format!("→ {out} +{sends} send{}", if sends == 1 { "" } else { "s" }) } else { format!("→ {out}") }
}

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
                // Double-click renames in place (same `track.rename` command as the Console).
                let key = egui::Id::new(("vm-rename", tr.id.0));
                let editing: Option<String> = ui.ctx().memory(|m| m.data.get_temp(key));
                if let Some(mut text) = editing {
                    let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(110.0));
                    r.request_focus();
                    if r.lost_focus() {
                        ui.ctx().memory_mut(|m| m.data.remove::<String>(key));
                        if !ui.input(|i| i.key_pressed(egui::Key::Escape)) && text.trim() != tr.name && !text.trim().is_empty() {
                            let _ = app.run("track.rename", json!({"track": tr.id.0, "name": text}));
                        }
                    } else {
                        ui.ctx().memory_mut(|m| m.data.insert_temp(key, text));
                    }
                } else {
                    let r = ui.selectable_label(sel, &tr.name).on_hover_text("Click to select, double-click to rename");
                    if r.double_clicked() {
                        ui.ctx().memory_mut(|m| m.data.insert_temp(key, tr.name.clone()));
                    } else if r.clicked() {
                        let _ = app.run("edit.select", json!({"tracks": [tr.id.0]}));
                    }
                }
                if app.meters.get(&tr.id).is_some_and(|m| m.clip) {
                    ui.label(egui::RichText::new("CLIP").small().strong().color(Color32::from_rgb(220, 60, 60)))
                        .on_hover_text("This channel's meter clipped (≥ 0 dBFS); clears with the meter hold");
                }
            });
            // Second line: route badge, then numeric gain/width.
            ui.horizontal(|ui| {
                ui.add_space(24.0);
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
                let badge = route_badge(app.engine.session(), &tr);
                ui.add(egui::Label::new(egui::RichText::new(badge).small().color(Tokens::current().text_dim)).truncate());
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
    fn route_badge_names_destination_and_active_sends() {
        use soundcraft_model::{ChannelFormat, Route, SendSlot, Session};
        let mut s = Session::default();
        let b = s.add_bus("Vox", ChannelFormat::Stereo);
        let fx = s.add_bus("Verb", ChannelFormat::Stereo);
        let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, None);
        let tr = s.track(t).unwrap().clone();
        assert_eq!(route_badge(&s, &tr), "→ Main");
        let tr = s.track_mut(t).unwrap();
        tr.mixer.output = Route::Bus(b);
        tr.mixer.sends[0] = Some(SendSlot::new(Route::Bus(fx)));
        let mut muted = SendSlot::new(Route::Bus(fx));
        muted.mute = true;
        tr.mixer.sends[1] = Some(muted);
        let tr = s.track(t).unwrap().clone();
        assert_eq!(route_badge(&s, &tr), "→ Vox +1 send", "muted sends are not counted");
        let mut tr = tr;
        tr.mixer.output = Route::None;
        tr.mixer.sends = Default::default();
        assert_eq!(route_badge(&s, &tr), "→ none");
    }

    #[test]
    fn label_placement_never_overlaps_labels_or_nodes() {
        // A dense stage: a column of near-identical nodes, a row of neighbours, and outliers.
        let bounds = Rect::from_min_size(pos2(0.0, 0.0), vec2(600.0, 400.0));
        let mut nodes: Vec<Rect> = Vec::new();
        for k in 0..12 {
            nodes.push(Rect::from_center_size(pos2(300.0, 120.0 + k as f32 * 9.0), vec2(40.0, 18.0)));
        }
        for k in 0..8 {
            nodes.push(Rect::from_center_size(pos2(80.0 + k as f32 * 30.0, 300.0), vec2(16.0, 18.0)));
        }
        nodes.push(Rect::from_center_size(pos2(590.0, 10.0), vec2(16.0, 18.0)));
        let groups = clusters(&nodes);
        assert!(groups.iter().any(|g| g.len() >= 12), "the column is one cluster");
        let anchors: Vec<Rect> = groups.iter().map(|g| g.iter().fold(nodes[g[0]], |a, &i| a.union(nodes[i]))).collect();
        let sizes: Vec<egui::Vec2> = anchors.iter().map(|_| vec2(70.0, 16.0)).collect();
        let placed = place_labels(&anchors, &sizes, &nodes, bounds);
        let got: Vec<Rect> = placed.iter().flatten().copied().collect();
        assert!(got.len() >= anchors.len() - 1, "nearly every label finds room ({} of {})", got.len(), anchors.len());
        for (i, a) in got.iter().enumerate() {
            assert!(bounds.contains_rect(*a), "{a:?} inside the canvas");
            assert!(!nodes.iter().any(|n| n.intersects(*a)), "{a:?} never covers a node");
            for b in &got[i + 1..] {
                assert!(!a.intersects(*b), "{a:?} overlaps {b:?}");
            }
        }
        // Separate nodes stay separate clusters; a chain of overlaps is one.
        let apart = [Rect::from_min_size(pos2(0.0, 0.0), vec2(10.0, 10.0)), Rect::from_min_size(pos2(50.0, 0.0), vec2(10.0, 10.0))];
        assert_eq!(clusters(&apart).len(), 2);
        let chain = [
            Rect::from_min_size(pos2(0.0, 0.0), vec2(10.0, 10.0)),
            Rect::from_min_size(pos2(8.0, 0.0), vec2(10.0, 10.0)),
            Rect::from_min_size(pos2(16.0, 0.0), vec2(10.0, 10.0)),
        ];
        assert_eq!(clusters(&chain).len(), 1);
    }

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
