//! The control bar under the menu bar, in every layout (Hotfix Pass 01):
//! Audio Setup and the device summary (HF06), tempo and time signature (HF03), Click and
//! Count Off, the Loop toggle with its range and settings (HF05), and — in the Mix and Master
//! layouts, which have no Edit toolbar — Stop/Play/Record with the position. It wraps onto a
//! second row on narrow windows instead of clipping. Every control runs an existing command
//! against the one session; the bar keeps no tempo, loop or device state of its own (only
//! uncommitted text while you type).

use crate::SoundApp;
use crate::theme::{Tokens, group_frame, metric, mono, regular, size};
use egui::{Color32, CornerRadius, RichText, Sense, Stroke, StrokeKind, Ui, vec2};
use serde_json::json;
use soundcraft_time::{TimeFormat, format_position};

pub fn show(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let bar = egui::Panel::top("control_bar")
        .resizable(false)
        .frame(egui::Frame::NONE.fill(t.toolbar_bg).inner_margin(egui::Margin::symmetric(8, 4)))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                // Segments (docs/STYLE_GUIDE.md): each is one raised group; on narrow windows
                // whole segments wrap to the next row, so no control is ever cut in half.
                ui.spacing_mut().item_spacing = vec2(metric::GAP, 4.0);
                // A frame's width is only known after it is drawn, so each segment remembers its
                // width from the previous frame and starts a new row when it would not fit.
                let (left, right) = (ui.max_rect().min.x, ui.max_rect().max.x);
                let seg = |ui: &mut Ui, name: &str, f: &mut dyn FnMut(&mut Ui)| {
                    let key = egui::Id::new(("control_bar_segment", name));
                    let need = ui.ctx().data(|d| d.get_temp::<f32>(key)).unwrap_or(0.0);
                    // (In a wrapping layout `available_width` is a whole new row, so compare with
                    // the row's right edge captured before any segment was drawn.)
                    if ui.cursor().min.x + need > right && ui.cursor().min.x > left + 1.0 {
                        ui.end_row();
                    }
                    let r = group_frame().show(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = metric::GAP;
                        ui.horizontal(|ui| f(ui));
                    });
                    ui.ctx().data_mut(|d| d.insert_temp(key, r.response.rect.width()));
                };
                if crate::layouts::switcher_overflows(ui.ctx()) {
                    seg(ui, "layouts", &mut |ui| crate::layouts::switcher(app, ui));
                }
                // Mix and Master have no Edit bottom bar: their transport is here.
                if matches!(app.ui.window, crate::MainWindow::Mix | crate::MainWindow::Master) {
                    seg(ui, "transport", &mut |ui| transport(app, ui));
                }
                seg(ui, "tempo", &mut |ui| {
                    tempo(app, ui);
                    meter(app, ui);
                });
                seg(ui, "click", &mut |ui| click_countoff(app, ui));
                seg(ui, "loop", &mut |ui| loop_controls(app, ui));
                seg(ui, "recording", &mut |ui| recording_status(app, ui));
                seg(ui, "audio", &mut |ui| audio_setup(app, ui));
            });
        });
    // Windows opened from the bar (Audio Setup) stay below it, so the bar is never covered.
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(BOTTOM), bar.response.rect.max.y));
}

const BOTTOM: &str = "control_bar_bottom";

/// The screen area below the menu and control bars (for windows that must not cover them).
pub fn area_below(ctx: &egui::Context) -> egui::Rect {
    let screen = ctx.content_rect();
    let top = ctx.data(|d| d.get_temp::<f32>(egui::Id::new(BOTTOM))).unwrap_or(screen.min.y + 56.0);
    egui::Rect::from_min_max(egui::pos2(screen.min.x, top.min(screen.max.y - 120.0)), screen.max)
}

fn fmt(app: &SoundApp, at: i64, f: TimeFormat) -> String {
    let s = app.engine.session();
    format_position(at, f, s.sample_rate, &s.tempo, s.frame_rate, s.timecode_start)
}

/// One-line device summary: host, device, effective rate and callback size (or what is unknown).
pub fn device_summary(app: &SoundApp) -> String {
    match &app.player {
        None => "No audio output — open Audio Setup".to_string(),
        Some(p) => {
            let host = p.negotiated.as_ref().map_or("default host".to_string(), |n| n.host.clone());
            let mut name = p.device_name.clone();
            if name.chars().count() > 28 {
                name = format!("{}…", name.chars().take(27).collect::<String>());
            }
            let (_, _, last) = p.stream_clock();
            let block = if last == 0 {
                match p.negotiated.as_ref().and_then(|n| n.requested_buffer) {
                    Some(b) => format!("{b} fr requested (not running yet)"),
                    None => "default buffer (not running yet)".to_string(),
                }
            } else {
                format!("{last} fr = {:.1} ms", f64::from(last) * 1000.0 / f64::from(p.device_rate.max(1)))
            };
            format!("{host} · {name} · {:.1} kHz · {block}", f64::from(p.device_rate) / 1000.0)
        }
    }
}

/// A control-bar button: optional icon, a label, the standard height and state edges. `name`
/// is the accessible name (kept stable for screen readers, tests and the manual). Lit buttons use
/// their state colour (`on_fill`) with dark text, so on/off never depends on hue alone: the fill
/// changes from neutral to solid.
pub fn chip(ui: &mut Ui, icon: Option<&str>, text: &str, name: &str, on: bool, on_fill: Color32, enabled: bool) -> egui::Response {
    chip_impl(ui, icon, text, name, on, on_fill, enabled, false)
}

/// A chip that opens a popover: the same button with a drop-down arrow at its right.
pub fn chip_menu(ui: &mut Ui, text: &str, name: &str, enabled: bool) -> egui::Response {
    chip_impl(ui, None, text, name, false, Tokens::current().accent, enabled, true)
}

#[allow(clippy::too_many_arguments)]
fn chip_impl(ui: &mut Ui, icon: Option<&str>, text: &str, name: &str, on: bool, on_fill: Color32, enabled: bool, dropdown: bool) -> egui::Response {
    let t = Tokens::current();
    let fg = if !enabled {
        t.text_disabled
    } else if on {
        t.text_dark
    } else {
        t.text
    };
    let galley = ui.painter().layout_no_wrap(text.to_string(), regular(size::BODY), fg);
    let icon_w = if icon.is_some() { 16.0 } else { 0.0 };
    let w = galley.size().x + icon_w + 14.0 + if dropdown { 14.0 } else { 0.0 };
    let (r, resp) = ui.allocate_exact_size(vec2(w, metric::CONTROL_H), if enabled { Sense::click() } else { Sense::hover() });
    let fill = if on {
        on_fill
    } else if resp.hovered() && enabled {
        t.button_hi
    } else {
        t.button
    };
    let edge = if resp.has_focus() {
        Stroke::new(1.5, t.focus)
    } else if resp.hovered() && enabled && !on {
        Stroke::new(1.0, t.border_light)
    } else {
        Stroke::new(1.0, t.button_border)
    };
    ui.painter().rect(r, CornerRadius::same(metric::RADIUS), fill, edge, StrokeKind::Inside);
    let mut x = r.min.x + 7.0;
    if let Some(icon) = icon {
        // The record dot is always the record colour (armed), unless the chip is lit red.
        let ic = if icon == "record" && !on && enabled { t.rec } else { fg };
        crate::icons::draw(ui.painter(), egui::Rect::from_center_size(egui::pos2(x + 6.0, r.center().y), vec2(12.0, 12.0)), icon, ic);
        x += icon_w;
    }
    ui.painter().galley(egui::pos2(x, r.center().y - galley.size().y / 2.0), galley, fg);
    if dropdown {
        let a = egui::Rect::from_center_size(egui::pos2(r.max.x - 10.0, r.center().y), vec2(9.0, 9.0));
        crate::icons::draw(ui.painter(), a, "triangle_down", if enabled { t.text_dim } else { t.text_disabled });
    }
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, on, name));
    resp
}

/// A small status dot with a shape that differs by state (filled = OK, ring = waiting,
/// cross = missing), so the state reads without colour.
fn status_dot(ui: &mut Ui, state: Status) {
    let t = Tokens::current();
    let (r, _) = ui.allocate_exact_size(vec2(10.0, metric::CONTROL_H), Sense::hover());
    let c = r.center();
    match state {
        Status::Ok => {
            ui.painter().circle_filled(c, 4.0, t.playback);
        }
        Status::Waiting => {
            ui.painter().circle_stroke(c, 3.5, Stroke::new(1.5, t.warning));
        }
        Status::Missing => {
            let s = Stroke::new(1.8, t.error);
            ui.painter().line_segment([c + vec2(-3.5, -3.5), c + vec2(3.5, 3.5)], s);
            ui.painter().line_segment([c + vec2(-3.5, 3.5), c + vec2(3.5, -3.5)], s);
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Status {
    Ok,
    Waiting,
    Missing,
}

fn audio_setup(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let open = app.ui.show_playback_engine;
    let b = chip(ui, Some("gear"), "Audio Setup", "⚙ Audio Setup", open, t.accent, true)
        .on_hover_text("Microphone/interface input, output device, sample rate, buffer, monitoring and latency (Setup › Playback Engine…)");
    if b.clicked() {
        app.ui.audio_focus_track = None;
        app.ui.show_playback_engine = !open;
    }
    let state = match &app.player {
        None => Status::Missing,
        Some(p) if p.stream_clock().2 == 0 => Status::Waiting,
        Some(_) => Status::Ok,
    };
    status_dot(ui, state);
    let summary = device_summary(app);
    let input =
        app.recorder.as_ref().map_or("input: opened on first record".to_string(), |r| format!("input: {} ({} ch)", r.device_name, r.channels));
    let colour = if state == Status::Missing { t.warning } else { t.text_dim };
    let s = ui
        .add(egui::Label::new(RichText::new(&summary).font(mono(size::SMALL)).color(colour)).sense(Sense::click()))
        .on_hover_text(format!("{summary}\n{input}\nDriver latency: Unknown (not reported by this backend). Click for Audio Setup."));
    if s.clicked() {
        app.ui.show_playback_engine = true;
    }
}

/// What will record and how it is heard: armed tracks with their inputs, input monitoring, the
/// monitoring mode and the input device — only what the session and the open devices say.
fn recording_status(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let (armed, monitored, hardware) = {
        let s = app.engine.session();
        let input_of = |tr: &soundcraft_model::Track| match &tr.mixer.input {
            soundcraft_model::Route::Hardware(h) => h.clone(),
            _ => "In 1".to_string(),
        };
        let armed: Vec<(soundcraft_model::TrackId, String, String)> = s
            .tracks
            .iter()
            .filter(|tr| tr.mixer.record_arm && tr.kind == soundcraft_model::TrackKind::Audio && !tr.inactive)
            .map(|tr| (tr.id, tr.name.clone(), input_of(tr)))
            .collect();
        let monitored = s.tracks.iter().filter(|tr| tr.mixer.input_monitor).count();
        (armed, monitored, s.edit.value("monitor.mode", 1.0) as i64 == 2)
    };
    let recording = app.engine.transport.recording;
    let text = match armed.as_slice() {
        [] => "No track armed".to_string(),
        [(_, name, input)] => format!("Armed: {name} · {input}"),
        many => format!("Armed: {} tracks", many.len()),
    };
    let lit = !armed.is_empty();
    let resp = chip(ui, lit.then_some("record"), &text, "Recording status", recording, t.rec, true);
    let mic = app.recorder.as_ref().map_or("Input device: opens on the first record or arm".to_string(), |r| {
        format!("Input device: {} ({} channel(s) @ {} Hz)", r.device_name, r.channels, r.sample_rate)
    });
    let list = armed.iter().map(|(_, n, i)| format!("● {n} records {i}")).collect::<Vec<_>>().join("\n");
    let monitoring = if hardware {
        "Monitoring: hardware/direct (SML Studio's input monitoring is off)".to_string()
    } else {
        format!("Monitoring: software · {monitored} track(s) input-monitored")
    };
    let resp = resp.on_hover_text(format!(
        "{}{monitoring}\n{mic}\nClick for Audio Setup (input device and this track's input).",
        if list.is_empty() { "No track is record-enabled.\n".to_string() } else { format!("{list}\n") }
    ));
    if resp.clicked() {
        app.ui.audio_focus_track = armed.first().map(|a| a.0);
        app.ui.show_playback_engine = true;
    }
    if monitored > 0 && !hardware {
        ui.label(RichText::new(format!("I {monitored}")).font(mono(size::SMALL)).color(t.input))
            .on_hover_text(format!("{monitored} track(s) monitor their live input through the mixer"));
    }
}

pub(crate) fn transport(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let playing = app.is_playing();
    let recording = app.engine.transport.recording;
    if chip(ui, Some("stop"), "Stop", "■ Stop", false, t.button, true).on_hover_text("Stop").clicked() {
        let _ = app.run("transport.stop", json!({}));
    }
    if chip(ui, Some("play"), "Play", "▶ Play", playing && !recording, t.playback, true).on_hover_text("Play").clicked() {
        let _ = app.run("transport.play", json!({}));
    }
    if chip(ui, Some("record"), "Record", "● Record", recording, t.rec, true).on_hover_text("Record (armed tracks)").clicked() {
        let _ = app.run("transport.record", json!({}));
    }
    let f = app.engine.session().edit.main_counter;
    let pos = fmt(app, app.position(), f);
    ui.label(RichText::new(pos).font(mono(size::NUMERIC)).color(t.counter_text));
}

/// The tempo change in effect at `at` (samples): its position and BPM, and how many changes the
/// map has. Editing the BPM field edits exactly this change; other changes stay as they are.
pub fn tempo_at(app: &SoundApp, at: i64) -> (i64, f64, usize) {
    let s = app.engine.session();
    let tick = s.tempo.samples_to_ticks(at, s.sample_rate);
    let evs = s.tempo.tempos();
    let ev_tick = evs.iter().filter(|e| e.tick <= tick).map(|e| e.tick).next_back().unwrap_or(0);
    (s.tempo.tick_to_samples(ev_tick, s.sample_rate), s.tempo.tempo_at_tick(tick), evs.len())
}

fn tempo(app: &mut SoundApp, ui: &mut Ui) {
    let pos = app.position();
    let (ev_at, bpm, changes) = tempo_at(app, pos);
    let recording = app.engine.transport.recording;
    let id = egui::Id::new("control_bar_bpm");
    let mut text = app.ui.bpm_draft.clone().unwrap_or_else(|| format!("{bpm:.2}"));
    let field = egui::TextEdit::singleline(&mut text).id(id).desired_width(64.0).font(mono(size::NUMERIC_LARGE)).horizontal_align(egui::Align::RIGHT);
    let resp = ui.add_enabled(!recording, field);
    let label = ui.label(RichText::new("BPM").font(regular(size::SMALL)).color(Tokens::current().text_dim));
    let resp = resp.labelled_by(label.id);
    let where_ = if changes > 1 {
        format!(
            "Tempo map with {changes} changes: this field edits the change at {} (in effect at the playhead); the others are kept.",
            fmt(app, ev_at, TimeFormat::BarsBeats)
        )
    } else {
        "Constant tempo: this field sets the session tempo.".to_string()
    };
    let resp = resp
        .on_hover_text(format!("{where_}\n5–1000 BPM. Enter applies (one undo step); Escape or clicking away cancels. Audio clips keep their sample positions (nothing is stretched)."))
        .on_disabled_hover_text("Tempo cannot change during a take");
    if resp.gained_focus() || resp.changed() || (resp.has_focus() && app.ui.bpm_draft.is_none()) {
        app.ui.bpm_draft = Some(text.clone());
    }
    if resp.lost_focus() {
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        if enter {
            match text.trim().parse::<f64>() {
                Ok(v) if v.is_finite() && (5.0..=1000.0).contains(&v) => {
                    if (v - bpm).abs() > 1e-9
                        && let Err(e) = app.run("event.tempo", json!({"bpm": v, "at": ev_at}))
                    {
                        app.ui.status = e;
                    }
                }
                _ => app.ui.status = format!("Tempo not changed: `{}` is not a tempo between 5 and 1000 BPM", text.trim()),
            }
        }
        app.ui.bpm_draft = None;
    }
    if changes > 1 {
        ui.label(RichText::new("MAP").font(crate::theme::bold(size::CAPTION)).color(Tokens::current().focus)).on_hover_text(where_);
    }
}

fn meter(app: &mut SoundApp, ui: &mut Ui) {
    let (num, den, at) = {
        let s = app.engine.session();
        let tick = s.tempo.samples_to_ticks(app.position(), s.sample_rate);
        let m = s.tempo.meter_at_tick(tick);
        let ev = s.tempo.meters().iter().filter(|e| e.tick <= tick).map(|e| e.tick).next_back().unwrap_or(0);
        (m.numerator, m.denominator, s.tempo.tick_to_samples(ev, s.sample_rate))
    };
    let recording = app.engine.transport.recording;
    let resp = ui
        .add_enabled(
            !recording,
            egui::Button::new(RichText::new(format!("{num}/{den}")).font(mono(size::NUMERIC))).min_size(vec2(40.0, metric::CONTROL_H)),
        )
        .on_hover_text("Time signature in effect at the playhead — click to change it")
        .on_disabled_hover_text("The time signature cannot change during a take");
    let shown = egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        let (mut n, mut d) = app.ui.meter_draft.unwrap_or((num, den));
        ui.label(RichText::new("Time signature").strong());
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut n).range(1..=64));
            ui.label("/");
            egui::ComboBox::from_id_salt("meter_den").selected_text(d.to_string()).width(48.0).show_ui(ui, |ui| {
                for v in [1u32, 2, 4, 8, 16, 32, 64] {
                    ui.selectable_value(&mut d, v, v.to_string());
                }
            });
        });
        app.ui.meter_draft = Some((n, d));
        ui.label(RichText::new(format!("Changes the signature that starts at {}", fmt(app, at, TimeFormat::BarsBeats))).small());
        ui.horizontal(|ui| {
            if ui.button("Cancel").clicked() {
                app.ui.meter_draft = None;
                ui.close();
            }
            if ui.button(RichText::new("Apply").strong()).clicked() {
                if let Err(e) = app.run("event.meter", json!({"numerator": n, "denominator": d, "at": at})) {
                    app.ui.status = e;
                }
                app.ui.meter_draft = None;
                ui.close();
            }
        });
    });
    // Closing the popup any other way (click outside, Escape) discards the draft.
    if shown.is_none() {
        app.ui.meter_draft = None;
    }
}

fn click_countoff(app: &mut SoundApp, ui: &mut Ui) {
    let (click, countoff, bars) = {
        let e = &app.engine.session().edit;
        (e.click, e.countoff, e.countoff_bars)
    };
    let t = Tokens::current();
    if chip(ui, Some("click"), "Click", "Click", click, t.accent, true).on_hover_text("Metronome (Options › Click)").clicked() {
        let _ = app.run("options.click", json!({}));
    }
    let label = format!("Count Off {bars}");
    if chip(ui, None, &label, &label, countoff, t.accent, true)
        .on_hover_text("Count Off: recording starts this many bars early with the click (bars: Recording window)")
        .clicked()
    {
        let _ = app.run("options.countoff", json!({}));
    }
}

fn loop_controls(app: &mut SoundApp, ui: &mut Ui) {
    let (enabled, stored, sel, loop_record, f) = {
        let e = &app.engine.session().edit;
        (e.loop_playback, e.loop_range, e.selection, e.loop_record, e.main_counter)
    };
    let t = Tokens::current();
    let toggle = chip(ui, Some("loop"), "Loop", "⟲ Loop", enabled, t.accent, true)
        .on_hover_text("Loop Playback on/off (Options › Loop Playback). Recording is never started by this button.");
    if toggle.clicked() {
        let _ = app.run("options.loop_playback", json!({}));
    }
    let range_text = match (stored, sel.is_empty()) {
        (Some(r), _) => format!("{} – {}", fmt(app, r.start, f), fmt(app, r.end, f)),
        (None, false) => "selection".to_string(),
        (None, true) => "not set".to_string(),
    };
    let resp = chip_menu(ui, &range_text, &format!("Loop: {range_text} ▾"), true)
        .on_hover_text("Loop range and settings (start – end in the main counter format)");
    let shown =
        egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| loop_settings(app, ui, f, loop_record));
    if shown.is_none() {
        // A closed popup keeps no typed text: reopening shows the range as it is now.
        app.ui.loop_draft = None;
    }
}

fn loop_settings(app: &mut SoundApp, ui: &mut Ui, f: TimeFormat, loop_record: bool) {
    ui.set_min_width(300.0);
    ui.label(RichText::new("Loop settings").strong());
    let (enabled, stored, sel, recording) = {
        let s = app.engine.session();
        (s.edit.loop_playback, s.edit.loop_range, s.edit.selection, app.engine.transport.recording)
    };
    let mut on = enabled;
    if ui.checkbox(&mut on, "Loop Playback").changed() {
        let _ = app.run("options.loop_playback", json!({"value": on}));
    }
    match stored {
        Some(_) => ui.label("Playback loops the stored loop range below."),
        None => ui.label("No loop range is stored: Loop Playback loops the edit selection."),
    };
    let cur = stored.unwrap_or(if sel.is_empty() { soundcraft_time::Range::new(0, 0) } else { sel });
    let mut draft = app.ui.loop_draft.clone().unwrap_or_else(|| [fmt(app, cur.start, f), fmt(app, cur.end, f)]);
    egui::Grid::new("loop_grid").num_columns(2).show(ui, |ui| {
        let l = ui.label("Start");
        ui.add_enabled(!recording, egui::TextEdit::singleline(&mut draft[0]).desired_width(150.0).font(mono(12.0))).labelled_by(l.id);
        ui.end_row();
        let l = ui.label("End");
        ui.add_enabled(!recording, egui::TextEdit::singleline(&mut draft[1]).desired_width(150.0).font(mono(12.0))).labelled_by(l.id);
        ui.end_row();
        ui.label("Length");
        ui.label(RichText::new(if cur.is_empty() { "—".to_string() } else { format!("{} samples", cur.len()) }).font(mono(12.0)));
        ui.end_row();
    });
    ui.label(RichText::new(format!("Times in the main counter format ({}).", f.label())).small());
    app.ui.loop_draft = Some(draft.clone());
    ui.horizontal_wrapped(|ui| {
        if ui.add_enabled(!recording, egui::Button::new("Apply")).on_disabled_hover_text("Stop recording first").clicked() {
            let p = json!({"start": format!("{}:{}", f.id(), draft[0].trim()), "end": format!("{}:{}", f.id(), draft[1].trim())});
            match app.run("loop.set", p) {
                Ok(_) => app.ui.loop_draft = None,
                Err(e) => app.ui.status = e,
            }
        }
        if ui.add_enabled(!sel.is_empty() && !recording, egui::Button::new("Set Loop to Selection")).clicked() {
            if let Err(e) = app.run("loop.to_selection", json!({})) {
                app.ui.status = e;
            }
            app.ui.loop_draft = None;
        }
        if ui
            .add_enabled(stored.is_some(), egui::Button::new("Select Loop Range"))
            .on_hover_text("Copies the loop range into the edit selection (for Loop Record)")
            .clicked()
        {
            let _ = app.run("loop.select", json!({}));
        }
        if ui.add_enabled(stored.is_some() && !recording, egui::Button::new("Clear")).clicked() {
            let _ = app.run("loop.clear", json!({}));
            app.ui.loop_draft = None;
        }
    });
    ui.separator();
    ui.label(
        RichText::new(format!(
            "Loop Record ({}; Options › Loop Record) records over the edit selection, one take per pass — use Select Loop Range first to record the loop span.",
            if loop_record { "on" } else { "off" }
        ))
        .small(),
    );
}
