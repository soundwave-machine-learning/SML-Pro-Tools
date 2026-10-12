//! Side panels (Tracks, Groups, Clips) and floating windows.

use crate::SoundApp;
use crate::theme::{Tokens, bold, mono, regular, rgb};
use egui::{Align2, Color32, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;
use soundcraft_time::format_position;

fn panel_header(ui: &mut Ui, title: &str) {
    let t = Tokens::current();
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, t.panel_bg2);
    ui.painter().text(pos2(r.min.x + 8.0, r.center().y), Align2::LEFT_CENTER, title, bold(11.5), t.header_text);
    ui.painter().line_segment([pos2(r.min.x, r.max.y), pos2(r.max.x, r.max.y)], Stroke::new(1.0, t.border));
}

pub fn tracks_and_groups(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let total = ui.available_height();
    panel_header(ui, "TRACKS");
    let tracks: Vec<(u64, String, bool, [u8; 3], bool)> = {
        let s = app.engine.session();
        s.tracks.iter().map(|x| (x.id.0, x.name.clone(), !x.hidden, x.color, s.edit.selected_tracks.contains(&x.id))).collect()
    };
    egui::ScrollArea::vertical().id_salt("tracks_scroll").max_height(total * 0.62).auto_shrink([false, false]).show(ui, |ui| {
        for (id, name, shown, color, sel) in tracks {
            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 17.0), Sense::click());
            if sel {
                ui.painter().rect_filled(r, 0.0, t.row_selected);
            }
            let dot = Rect::from_center_size(pos2(r.min.x + 10.0, r.center().y), vec2(8.0, 8.0));
            let dresp = ui.interact(dot.expand(3.0), ui.id().with(("vis", id)), Sense::click());
            ui.painter().circle(dot.center(), 3.5, if shown { t.visible_dot } else { Color32::TRANSPARENT }, Stroke::new(1.0, t.text_dim));
            if dresp.clicked() {
                let _ = app.run("track.hide", json!({"tracks": [id], "hidden": shown}));
            }
            ui.painter().rect_filled(Rect::from_min_size(pos2(r.min.x + 20.0, r.min.y + 3.0), vec2(4.0, 11.0)), 0.0, rgb(color));
            ui.painter().text(pos2(r.min.x + 30.0, r.center().y), Align2::LEFT_CENTER, &name, regular(11.5), if shown { t.text } else { t.text_dim });
            if resp.clicked() {
                let _ = app.run("edit.select", json!({"tracks": [id]}));
            }
        }
    });
    ui.add_space(4.0);
    panel_header(ui, "GROUPS");
    let groups: Vec<(u64, char, String, bool, [u8; 3])> =
        app.engine.session().groups.iter().map(|g| (g.id.0, g.letter, g.name.clone(), g.active, g.color)).collect();
    egui::ScrollArea::vertical().id_salt("groups_scroll").auto_shrink([false, false]).show(ui, |ui| {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 17.0), Sense::hover());
        ui.painter().text(pos2(r.min.x + 30.0, r.center().y), Align2::LEFT_CENTER, "<ALL>", regular(11.5).clone(), t.text);
        let all = ui.interact(r, ui.id().with("group_all"), Sense::click());
        if all.double_clicked() {
            let _ = app.run("edit.select_all", json!({}));
        }
        all.context_menu(|ui| {
            if ui.button("New Group…").clicked() {
                let _ = app.dialogs.open_for_command(&app.engine, "track.group");
                ui.close();
            }
        });
        for (id, letter, name, active, color) in groups {
            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 17.0), Sense::click());
            if active {
                ui.painter().rect_filled(Rect::from_min_size(pos2(r.min.x + 4.0, r.min.y + 2.0), vec2(14.0, 13.0)), 1.0, rgb(color));
            }
            ui.painter().text(
                pos2(r.min.x + 11.0, r.center().y),
                Align2::CENTER_CENTER,
                letter.to_string(),
                bold(10.0),
                if active { t.text_dark } else { t.text_dim },
            );
            ui.painter().text(pos2(r.min.x + 30.0, r.center().y), Align2::LEFT_CENTER, &name, regular(11.5), t.text);
            if resp.clicked() {
                let _ = app.run("track.group_toggle", json!({"group": id}));
            }
            let members: Vec<u64> =
                app.engine.session().groups.iter().find(|g| g.id.0 == id).map(|g| g.members.iter().map(|m| m.0).collect()).unwrap_or_default();
            if resp.double_clicked() {
                let _ = app.run("edit.select", json!({"tracks": members, "exact": true}));
            }
            resp.context_menu(|ui| {
                if ui.button("Select Tracks in Group").clicked() {
                    let _ = app.run("edit.select", json!({"tracks": members, "exact": true}));
                    ui.close();
                }
                if ui.button(if active { "Disable" } else { "Enable" }).clicked() {
                    let _ = app.run("track.group_toggle", json!({"group": id}));
                    ui.close();
                }
                if ui.button("Delete Group").clicked() {
                    let _ = app.run("track.ungroup", json!({"group": id}));
                    ui.close();
                }
            });
        }
    });
}

pub fn clip_list(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    panel_header(ui, "CLIPS");
    let items: Vec<(Option<u64>, String, bool, [u8; 3])> = {
        let s = app.engine.session();
        let mut v: Vec<(Option<u64>, String, bool, [u8; 3])> = Vec::new();
        // Whole files first (bold), then clips on tracks.
        for src in &s.sources {
            v.push((Some(src.id.0), format!("{} ({}ch)", src.name, src.channels), true, [150, 150, 150]));
        }
        for tr in &s.tracks {
            for c in tr.clips() {
                v.push((Some(c.id.0), c.name.clone(), false, c.color.unwrap_or(tr.color)));
            }
        }
        v
    };
    let selected: Vec<u64> = app.engine.session().edit.selected_clips.iter().map(|c| c.0).collect();
    let fkey = egui::Id::new("clip_filter");
    let mut filter: String = ui.ctx().memory(|m| m.data.get_temp(fkey)).unwrap_or_default();
    ui.add(egui::TextEdit::singleline(&mut filter).hint_text("🔍 Find clips").desired_width(f32::INFINITY));
    ui.ctx().memory_mut(|m| m.data.insert_temp(fkey, filter.clone()));
    let needle = filter.to_lowercase();
    let items: Vec<_> = items.into_iter().filter(|(_, n, _, _)| needle.is_empty() || n.to_lowercase().contains(&needle)).collect();
    egui::ScrollArea::vertical().id_salt("clips_scroll").auto_shrink([false, false]).show(ui, |ui| {
        for (id, name, whole, color) in items {
            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 17.0), if whole { Sense::click_and_drag() } else { Sense::click() });
            if whole && let Some(src) = id {
                resp.context_menu(|ui| {
                    if ui.button("Place on New Track").clicked() {
                        let _ = app.run("clip.place_source", json!({"source": src}));
                        ui.close();
                    }
                    let sel_track = app.engine.session().edit.selected_tracks.first().map(|t| t.0);
                    if let Some(t) = sel_track
                        && ui.button("Place on Selected Track").clicked()
                    {
                        let _ = app.run("clip.place_source", json!({"source": src, "track": t}));
                        ui.close();
                    }
                });
                // Whole files drag onto tracks.
                resp.dnd_set_drag_payload(crate::DragSource(src));
                if resp.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                }
            }
            if !whole && id.is_some_and(|i| selected.contains(&i)) {
                ui.painter().rect_filled(r, 0.0, t.row_selected);
            }
            ui.painter().rect_filled(Rect::from_min_size(pos2(r.min.x + 6.0, r.min.y + 4.0), vec2(9.0, 9.0)), 1.0, rgb(color));
            ui.painter().with_clip_rect(r).text(
                pos2(r.min.x + 22.0, r.center().y),
                Align2::LEFT_CENTER,
                &name,
                if whole { bold(11.0) } else { regular(11.0) },
                t.text,
            );
            if resp.clicked()
                && !whole
                && let Some(i) = id
            {
                let _ = app.run("edit.select", json!({"clips": [i]}));
            }
        }
    });
}

/// Floating windows.
pub fn floating(app: &mut SoundApp, ctx: &egui::Context) {
    transport_window(app, ctx);
    big_counter(app, ctx);
    plugin_windows(app, ctx);
    about(app, ctx);
    session_info(app, ctx);
    recording_panel(app, ctx);
}

/// Window › Recording (R08): the essentials for a vocal take in one place — actual input
/// device, the selected track's arm/monitor and meter, monitoring mode, transport and the mixer.
fn recording_panel(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_recording;
    egui::Window::new("Recording").open(&mut open).default_pos(egui::pos2(260.0, 160.0)).default_width(320.0).resizable(false).show(ctx, |ui| {
        recording_essentials(app, ui);
    });
    app.ui.show_recording = open;
}

/// Monitor-only controls (R37): volume, dim and mono on the speaker path; headphone control
/// only when a dedicated cue path exists, otherwise shown unavailable (never the Main fader).
pub(crate) fn monitor_controls(app: &mut SoundApp, ui: &mut egui::Ui) {
    ui.label(egui::RichText::new("Monitor (speaker path only — never in a bounce)").small());
    ui.horizontal(|ui| {
        let mut db = app.ui.monitor.volume_db;
        if ui.add(egui::Slider::new(&mut db, -60.0..=0.0).suffix(" dB").text("Volume")).changed() {
            let _ = app.run("monitor.volume", json!({"db": db}));
        }
        if ui.selectable_label(app.ui.monitor.dim, "Dim −20").clicked() {
            let _ = app.run("monitor.dim", json!({}));
        }
        if ui.selectable_label(app.ui.monitor.mono, "Mono").clicked() {
            let _ = app.run("monitor.mono", json!({}));
        }
    });
    match app.cue_path() {
        Some(path) => {
            ui.label(
                egui::RichText::new(format!("Headphones: cue path “{path}” detected; its separate level is not implemented in this build.")).small(),
            );
        }
        None => {
            ui.add_enabled_ui(false, |ui| {
                ui.button("Headphones: unavailable")
                    .on_disabled_hover_text("No dedicated cue output path in I/O Setup. The Main fader is never relabelled as headphones.");
            });
        }
    }
    let hw = app.engine.session().edit.value("monitor.mode", 1.0) as i64 == 2;
    ui.label(
        egui::RichText::new(if hw {
            "Direct hardware monitoring: the interface mixes the input; software monitoring is off."
        } else {
            "Software monitoring through the mixer (latency applies)."
        })
        .small(),
    );
}

/// The Recording essentials (R08) shared by the Recording window and the Record layout.
pub(crate) fn recording_essentials(app: &mut SoundApp, ui: &mut egui::Ui) {
    {
        let input = app
            .recorder
            .as_ref()
            .map_or("opened on first record / arm".to_string(), |r| format!("{} — {} ch @ {} Hz", r.device_name, r.channels, r.sample_rate));
        ui.label(format!("Input: {input}"));
        let output = app.player.as_ref().map_or("none".to_string(), |p| {
            if p.silent { "Offline / no audio device".to_string() } else { format!("{} @ {} Hz", p.device_name, p.device_rate) }
        });
        ui.label(format!("Output: {output}"));
        ui.separator();
        let sel = app.engine.session().edit.selected_tracks.first().copied();
        let track = sel.and_then(|t| app.engine.session().track(t).cloned()).filter(|t| t.kind == soundcraft_model::TrackKind::Audio);
        match track {
            Some(t) => {
                ui.horizontal(|ui| {
                    ui.strong(&t.name);
                    if ui.selectable_label(t.mixer.record_arm, "● Arm").clicked() {
                        let _ = app.run("mix.record_arm", json!({"track": t.id.0}));
                    }
                    if ui.selectable_label(t.mixer.input_monitor, "Input monitor").on_hover_text("Software monitoring (off by default)").clicked() {
                        let _ = app.run("mix.input_monitor", json!({"track": t.id.0}));
                    }
                });
                let lvl = app.meters.get(&t.id).map_or(0.0, |m| m.level[0].max(m.level[1]));
                ui.add(egui::ProgressBar::new(lvl.clamp(0.0, 1.0)).text(if lvl > 0.0 {
                    format!("{:.1} dBFS", 20.0 * lvl.max(1e-6).log10())
                } else {
                    "-inf".into()
                }));
            }
            None => {
                ui.label("Select an audio track to arm it here.");
            }
        }
        ui.horizontal(|ui| {
            ui.label("Monitoring:");
            let mode = app.engine.session().edit.value("monitor.mode", 1.0) as i64;
            for (m, label) in [(0, "Tracking"), (1, "Mixing"), (2, "Hardware")] {
                if ui.selectable_label(mode == m, label).clicked() {
                    let id = ["tracking", "mixing", "hardware"][m as usize];
                    if let Err(e) = app.run("mix.monitor_mode", json!({"mode": id})) {
                        app.ui.status = e;
                    }
                }
            }
        });
        ui.horizontal(|ui| {
            // Count Off (same setting as the toolbar): whole bars of click before the take.
            let (mut on, mut bars) = (app.engine.session().edit.countoff, app.engine.session().edit.countoff_bars);
            if ui.checkbox(&mut on, "Count Off").changed() {
                let _ = app.run("options.countoff", json!({"value": on}));
            }
            if ui.add(egui::DragValue::new(&mut bars).range(1..=16).suffix(" bars")).changed() {
                let _ = app.run("setup.click_countoff", json!({"countoff_bars": bars}));
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("■ Stop").clicked() {
                let _ = app.run("transport.stop", json!({}));
            }
            if ui.button("▶ Play").clicked() {
                let _ = app.run("transport.play", json!({}));
            }
            if ui.button("● Record").clicked() {
                let _ = app.run("transport.record", json!({}));
            }
            if ui.button("Mixer").clicked() {
                crate::menus::invoke_menu(app, "window.mix", "");
            }
        });
        if let Some(r) = &app.recorder
            && r.is_armed()
        {
            ui.label(format!("Capturing: {} frames to disk (dry)", r.captured_frames()));
        }
        ui.label(egui::RichText::new("Recording is always dry: clip, insert, slot and fader processing never reach the file.").small());
        ui.label(egui::RichText::new(app.autosave_summary()).small());
        ui.separator();
        monitor_controls(app, ui);
    }
}

fn transport_window(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_transport;
    if !open {
        return;
    }
    let t = Tokens::current();
    egui::Window::new("Transport").open(&mut open).resizable(false).default_pos(pos2(400.0, 500.0)).show(ctx, |ui| {
        let flags = app.engine.session().edit.flags.clone();
        let expanded = flags.contains("view.transport.expanded");
        ui.horizontal(|ui| {
            for (icon, cmd) in [
                ("rtz", "transport.rtz"),
                ("rewind", "transport.rewind"),
                ("stop", "transport.stop"),
                ("play", "transport.play"),
                ("ffwd", "transport.fast_forward"),
                ("end", "transport.go_to_end"),
                ("record", "transport.record"),
            ] {
                if crate::widgets::icon_button(
                    ui,
                    vec2(34.0, 26.0),
                    icon,
                    (cmd == "transport.play" && app.is_playing()) || (cmd == "transport.record" && app.engine.transport.recording),
                    cmd,
                )
                .clicked()
                {
                    let _ = app.run(cmd, json!({}));
                }
            }
            if flags.contains("view.transport.output_meters") {
                let (r, _) = ui.allocate_exact_size(vec2(22.0, 26.0), Sense::hover());
                let m = app.main_meter;
                crate::widgets::meter(ui, Rect::from_min_size(r.min, vec2(10.0, 26.0)), m.level[0], m.hold[0], m.clip);
                crate::widgets::meter(ui, Rect::from_min_size(pos2(r.min.x + 12.0, r.min.y), vec2(10.0, 26.0)), m.level[1], m.hold[1], m.clip);
            }
        });
        let s = app.engine.session();
        let fmt = |x: i64| format_position(x, s.edit.main_counter, s.sample_rate, &s.tempo, s.frame_rate, s.timecode_start);
        if flags.contains("view.transport.counters") || expanded || flags.is_empty() {
            ui.label(egui::RichText::new(fmt(app.position())).font(mono(20.0)).color(t.counter_text));
        }
        if expanded {
            let e = s.edit.clone();
            egui::Grid::new("tx_exp").num_columns(4).show(ui, |ui| {
                ui.label("Start");
                ui.label(egui::RichText::new(fmt(e.selection.start)).font(mono(11.0)));
                ui.label("Pre-roll");
                ui.label(egui::RichText::new(format!("{:.2} s", s.sample_rate.seconds(e.pre_roll))).font(mono(11.0)));
                ui.end_row();
                ui.label("End");
                ui.label(egui::RichText::new(fmt(e.selection.end)).font(mono(11.0)));
                ui.label("Post-roll");
                ui.label(egui::RichText::new(format!("{:.2} s", s.sample_rate.seconds(e.post_roll))).font(mono(11.0)));
                ui.end_row();
            });
        }
        if flags.contains("view.transport.midi_controls") || expanded {
            let (click, countoff, merge, loop_rec, prepost) = {
                let e = &app.engine.session().edit;
                (e.click, e.countoff, e.midi_merge, e.loop_record, e.pre_post_roll)
            };
            ui.horizontal(|ui| {
                for (label, on, cmd) in [
                    ("Click", click, "options.click"),
                    ("Count Off", countoff, "options.countoff"),
                    ("MIDI Merge", merge, "options.midi_merge"),
                    ("Loop Rec", loop_rec, "options.loop_record"),
                    ("Pre/Post", prepost, "options.pre_post_roll"),
                ] {
                    if ui.selectable_label(on, label).clicked() {
                        let _ = app.run(cmd, json!({}));
                    }
                }
            });
            let tick = app.engine.session().tempo.samples_to_ticks(app.position(), app.engine.session().sample_rate);
            let bpm = app.engine.session().tempo.tempo_at_tick(tick);
            let m = app.engine.session().tempo.meter_at_tick(tick);
            ui.label(egui::RichText::new(format!("♩ = {bpm:.2}   {}/{}", m.numerator, m.denominator)).font(mono(12.0)).color(t.counter_text));
        }
    });
    app.ui.show_transport = open;
}

fn big_counter(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_big_counter;
    if !open {
        return;
    }
    egui::Window::new("Big Counter").open(&mut open).default_size(vec2(520.0, 110.0)).show(ctx, |ui| {
        let s = app.engine.session();
        let txt = format_position(app.position(), s.edit.main_counter, s.sample_rate, &s.tempo, s.frame_rate, s.timecode_start);
        let r = ui.available_rect_before_wrap();
        let t = Tokens::current();
        ui.painter().rect_filled(r, 4.0, t.big_counter_bg);
        ui.painter().text(r.center(), Align2::CENTER_CENTER, txt, mono((r.height() * 0.6).clamp(20.0, 120.0)), t.counter_text);
    });
    app.ui.show_big_counter = open;
}

pub(crate) fn memory_locations(app: &mut SoundApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        if ui.button("+ Marker").clicked() {
            let _ = app.run("markers.add", json!({}));
        }
        if ui.button("+ Selection").clicked() {
            let _ = app.run("markers.add", json!({"kind": "selection"}));
        }
    });
    let rows: Vec<(u32, String, String, String, String)> = {
        let s = app.engine.session();
        let f = |x: i64| format_position(x, s.edit.main_counter, s.sample_rate, &s.tempo, s.frame_rate, s.timecode_start);
        s.markers
            .iter()
            .map(|m| {
                let kind = match m.kind {
                    soundcraft_model::MarkerKind::Marker => "marker",
                    soundcraft_model::MarkerKind::Selection => "selection",
                    soundcraft_model::MarkerKind::None => "none",
                };
                let len = if m.kind == soundcraft_model::MarkerKind::Selection { f(m.end - m.start) } else { String::new() };
                (m.number, m.name.clone(), f(m.start), len, kind.to_string())
            })
            .collect()
    };
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("mem_grid").striped(true).num_columns(6).show(ui, |ui| {
            for h in ["#", "Name", "Location", "Length", "Type", ""] {
                ui.label(egui::RichText::new(h).strong());
            }
            ui.end_row();
            for (n, name, loc, len, kind) in rows {
                if ui.button(n.to_string()).on_hover_text("Recall").clicked() {
                    let _ = app.run("markers.recall", json!({"number": n}));
                }
                let key = egui::Id::new(("mem_name", n));
                let mut edit: String = ui.ctx().memory(|m| m.data.get_temp(key)).unwrap_or_else(|| name.clone());
                let r = ui.add(egui::TextEdit::singleline(&mut edit).desired_width(140.0));
                if r.changed() {
                    ui.ctx().memory_mut(|m| m.data.insert_temp(key, edit.clone()));
                }
                if r.lost_focus() && edit != name {
                    let _ = app.run("markers.edit", json!({"number": n, "rename": edit}));
                    ui.ctx().memory_mut(|m| m.data.remove::<String>(key));
                }
                ui.label(egui::RichText::new(loc).font(mono(11.0)));
                ui.label(egui::RichText::new(len).font(mono(11.0)));
                ui.label(kind);
                if ui.small_button("✕").on_hover_text("Delete").clicked() {
                    let _ = app.run("markers.delete", json!({"number": n}));
                }
                ui.end_row();
            }
        });
    });
}

pub(crate) fn undo_history(app: &mut SoundApp, ui: &mut Ui) {
    let hist = app.engine.undo_history();
    let n = hist.len();
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (i, h) in hist.iter().enumerate() {
            if ui.selectable_label(i + 1 == n, format!("{}  {h}", i + 1)).clicked() {
                for _ in 0..(n - i - 1) {
                    let _ = app.run("edit.undo", json!({}));
                }
            }
        }
        if let Some(r) = app.engine.redo_label() {
            ui.label(egui::RichText::new(format!("(redo) {r}")).italics().color(Tokens::current().text_dim));
        }
    });
}

fn plugin_windows(app: &mut SoundApp, ctx: &egui::Context) {
    let wins = app.ui.plugin_windows.clone();
    let mut keep = Vec::new();
    for (tid, slot) in wins {
        // The instrument slot (an instrument track's instrument) or an insert slot.
        let instrument = slot == soundcraft_mix::INSTRUMENT_SLOT;
        let Some((tname, ins)) = app.engine.session().track(tid).and_then(|t| {
            let ins = if instrument { t.instrument.clone() } else { t.mixer.inserts.get(slot).cloned().flatten() };
            ins.map(|i| (t.name.clone(), i))
        }) else {
            continue;
        };
        let Some(info) = crate::mix_window::plugin_info(&ins.plugin) else { continue };
        let place = if instrument { "instrument".to_string() } else { char::from(b'a'.saturating_add(slot as u8)).to_string() };
        let mut open = true;
        egui::Window::new(format!("{tname} · {place} · {}", info.name))
            .id(egui::Id::new(("plugin", tid.0, slot)))
            .open(&mut open)
            .default_width(340.0)
            .default_pos(egui::pos2(ctx.content_rect().width() - 380.0, 120.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Bypass and presets act on insert slots.
                    if !instrument {
                        ui.label(egui::RichText::new(&ins.preset).color(Tokens::current().text_dim));
                        let label = if ins.bypass { "BYPASSED" } else { "Bypass" };
                        if ui.button(label).clicked() {
                            let _ = app.run("mix.insert_bypass", json!({"track": tid.0, "slot": slot}));
                        }
                        ui.menu_button("Presets ▾", |ui| preset_menu(app, ui, tid, slot, info, &ins));
                    }
                    if soundcraft_mix::is_third_party(&ins.plugin) {
                        let open = app.player.as_ref().is_some_and(|p| p.editor_open(tid, slot));
                        let available = app.player.as_ref().is_some_and(|p| p.has_editor(tid, slot));
                        let label = if open { "Close Plugin Editor" } else { "Open Plugin Editor" };
                        let resp = ui.add_enabled(available, egui::Button::new(label));
                        let resp =
                            if available { resp } else { resp.on_disabled_hover_text("This plugin has no editor of its own, or it is not loaded") };
                        if resp.clicked()
                            && let Some(p) = &app.player
                        {
                            if open {
                                p.close_editor(tid, slot);
                            } else if let Err(e) = p.open_editor(tid, slot) {
                                app.ui.status = format!("{}: {e}", info.name);
                            }
                        }
                    }
                });
                if instrument {
                    // The parameter commands address insert slots: an instrument is played and
                    // edited in its own editor.
                    ui.label(egui::RichText::new("Load and edit sounds in the plugin's own editor.").color(Tokens::current().text_dim));
                    return;
                }
                if soundcraft_dsp::is_removed_builtin(info.id) {
                    ui.label(
                        egui::RichText::new(
                            "Missing: this upstream bundled processor is not part of SML Recording. Its settings are kept; audio passes through.",
                        )
                        .color(Tokens::current().text_dim),
                    );
                }
                egui::Grid::new(("pg", tid.0, slot)).num_columns(3).show(ui, |ui| {
                    for p in info.params {
                        let v = ins.params.get(p.id).copied().unwrap_or(p.default);
                        ui.label(p.name);
                        let mut x = v;
                        let resp = if !p.choices.is_empty() {
                            let mut idx = x.round().max(0.0) as usize;
                            let r = egui::ComboBox::from_id_salt(("pc", tid.0, slot, p.id))
                                .selected_text(p.choices.get(idx).copied().unwrap_or("?"))
                                .show_ui(ui, |ui| {
                                    for (i, c) in p.choices.iter().enumerate() {
                                        ui.selectable_value(&mut idx, i, *c);
                                    }
                                });
                            x = idx as f32;
                            r.response
                        } else {
                            let mut sl = egui::Slider::new(&mut x, p.min..=p.max).show_value(false);
                            if p.taper == soundcraft_dsp::Taper::Log && p.min > 0.0 {
                                sl = sl.logarithmic(true);
                            }
                            ui.add(sl)
                        };
                        if (x - v).abs() > f32::EPSILON {
                            let _ = app.engine.execute_merged(
                                "mix.insert_param",
                                &json!({"track": tid.0, "slot": slot, "param": p.id, "value": x}),
                                &format!("param:{}:{slot}:{}", tid.0, p.id),
                            );
                            if let Some(pl) = &app.player {
                                // Keep the plugin's own editor in step.
                                pl.editor_set_param(tid, slot, p.id, x);
                            }
                        }
                        let _ = resp;
                        ui.label(egui::RichText::new(p.format(x)).font(mono(11.0)));
                        ui.end_row();
                    }
                });
            });
        if open {
            keep.push((tid, slot));
        }
    }
    app.ui.plugin_windows = keep;
}

fn about(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_about;
    if !open {
        return;
    }
    let tab_id = egui::Id::new("about_tab");
    egui::Window::new("About SML Studio").open(&mut open).default_size(vec2(640.0, 420.0)).collapsible(false).show(ctx, |ui| {
        let mut tab = ui.data_mut(|d| d.get_temp::<u8>(tab_id)).unwrap_or(0);
        ui.horizontal(|ui| {
            for (i, l) in (0u8..).zip(["About", "Contributors", "Models"]) {
                if ui.selectable_label(tab == i, l).clicked() {
                    tab = i;
                }
            }
        });
        ui.data_mut(|d| d.insert_temp(tab_id, tab));
        ui.separator();
        match tab {
            1 => crate::credits::contributors_ui(ui),
            2 => crate::credits::models_ui(ui),
            _ => {
                ui.label(egui::RichText::new("SML Studio").font(bold(22.0)));
                ui.label(format!(
                    "Version {} — SoundCraft engine (session format, command ids, automation ids and plugin state keys unchanged)",
                    env!("CARGO_PKG_VERSION")
                ));
                ui.label("An open-source digital audio workstation from the ArtCraft team.");
                ui.label("Dual-licensed MIT OR Apache-2.0. Made with Rust and egui.");
                ui.hyperlink_to("getartcraft.com/apps/soundcraft", "https://getartcraft.com/apps/soundcraft");
                ui.hyperlink_to("Join us on Discord", "https://discord.gg/artcraft");
            }
        }
    });
    app.ui.show_about = open;
}

fn session_info(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_session_info;
    if !open {
        return;
    }
    egui::Window::new("Session Info").open(&mut open).default_size(vec2(480.0, 360.0)).show(ctx, |ui| {
        let text = soundcraft_engine::inspect::session_text(&app.engine);
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label(egui::RichText::new(text).font(mono(11.0)));
        });
    });
    app.ui.show_session_info = open;
}

/// Folder for a plugin's user presets.
fn preset_dir(app: &SoundApp, plugin: &str) -> Option<std::path::PathBuf> {
    app.preset_dir.as_ref().map(|d| d.join(crate::preset_folder_name(plugin)))
}

fn preset_menu(
    app: &mut SoundApp,
    ui: &mut Ui,
    tid: soundcraft_model::TrackId,
    slot: usize,
    info: &soundcraft_dsp::PluginInfo,
    ins: &soundcraft_model::Insert,
) {
    if ui.button("<factory default>").clicked() {
        let defaults: serde_json::Map<String, serde_json::Value> = info.params.iter().map(|p| (p.id.to_string(), json!(p.default))).collect();
        let _ = app.run("mix.insert_params", json!({"track": tid.0, "slot": slot, "params": defaults, "preset": "<factory default>"}));
        ui.close();
    }
    let Some(dir) = preset_dir(app, info.id) else {
        ui.label("User presets need the desktop app.");
        return;
    };
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .map(|rd| rd.flatten().filter_map(|e| e.path().file_stem().and_then(|x| x.to_str()).map(str::to_string)).collect())
        .unwrap_or_default();
    names.sort();
    for n in &names {
        if ui.button(n).clicked() {
            if let Ok(text) = std::fs::read_to_string(dir.join(format!("{n}.json")))
                && let Ok(v) = serde_json::from_str::<serde_json::Value>(&text)
            {
                let _ = app.run(
                    "mix.insert_params",
                    json!({"track": tid.0, "slot": slot, "params": v.get("params").cloned().unwrap_or_default(), "preset": n}),
                );
            }
            ui.close();
        }
    }
    ui.separator();
    let key = egui::Id::new(("preset_name", tid.0, slot));
    let mut name: String = ui.ctx().memory(|m| m.data.get_temp(key)).unwrap_or_else(|| format!("{} preset", info.name));
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut name).desired_width(140.0));
        if ui.button("Save").clicked() {
            let clean = soundcraft_engine::io::sanitize_name(&name);
            let body = json!({"format": "soundcraft-plugin-preset", "plugin": info.id, "params": ins.params});
            let ok = std::fs::create_dir_all(&dir).is_ok() && std::fs::write(dir.join(format!("{clean}.json")), body.to_string()).is_ok();
            app.ui.status = if ok { format!("Saved preset {clean}") } else { "Could not save the preset".into() };
            if ok {
                let _ = app.run("mix.insert_params", json!({"track": tid.0, "slot": slot, "params": {}, "preset": clean}));
            }
        }
    });
    ui.ctx().memory_mut(|m| m.data.insert_temp(key, name));
}
