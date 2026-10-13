//! The control bar under the menu bar, in every layout (Hotfix Pass 01):
//! Audio Setup and the device summary (HF06), tempo and time signature (HF03), Click and
//! Count Off, the Loop toggle with its range and settings (HF05), and — in the Mix and Master
//! layouts, which have no Edit toolbar — Stop/Play/Record with the position. It wraps onto a
//! second row on narrow windows instead of clipping. Every control runs an existing command
//! against the one session; the bar keeps no tempo, loop or device state of its own (only
//! uncommitted text while you type).

use crate::SoundApp;
use crate::theme::{Tokens, mono};
use egui::{RichText, Ui, vec2};
use serde_json::json;
use soundcraft_time::{TimeFormat, format_position};

pub fn show(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let bar = egui::Panel::top("control_bar")
        .resizable(false)
        .frame(egui::Frame::NONE.fill(t.toolbar_bg).inner_margin(egui::Margin::symmetric(8, 3)))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
                audio_setup(app, ui);
                ui.separator();
                if matches!(app.ui.window, crate::MainWindow::Mix | crate::MainWindow::Master) {
                    transport(app, ui);
                    ui.separator();
                }
                tempo(app, ui);
                meter(app, ui);
                click_countoff(app, ui);
                ui.separator();
                loop_controls(app, ui);
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

fn audio_setup(app: &mut SoundApp, ui: &mut Ui) {
    let open = app.ui.show_playback_engine;
    let b = ui
        .selectable_label(open, RichText::new("⚙ Audio Setup").strong())
        .on_hover_text("Microphone/interface input, output device, sample rate, buffer, monitoring and latency (Setup › Playback Engine…)");
    if b.clicked() {
        app.ui.audio_focus_track = None;
        app.ui.show_playback_engine = !open;
    }
    let summary = device_summary(app);
    let input =
        app.recorder.as_ref().map_or("input: opened on first record".to_string(), |r| format!("input: {} ({} ch)", r.device_name, r.channels));
    let s = ui
        .add(egui::Label::new(RichText::new(&summary).font(mono(11.0)).color(Tokens::current().text_dim)).sense(egui::Sense::click()))
        .on_hover_text(format!("{summary}\n{input}\nDriver latency: Unknown (not reported by this backend). Click for Audio Setup."));
    if s.clicked() {
        app.ui.show_playback_engine = true;
    }
}

fn transport(app: &mut SoundApp, ui: &mut Ui) {
    let playing = app.is_playing();
    let recording = app.engine.transport.recording;
    if ui.button("■ Stop").clicked() {
        let _ = app.run("transport.stop", json!({}));
    }
    if ui.selectable_label(playing && !recording, "▶ Play").clicked() {
        let _ = app.run("transport.play", json!({}));
    }
    let rec = RichText::new("● Record").color(if recording { egui::Color32::from_rgb(240, 80, 70) } else { Tokens::current().text });
    if ui.selectable_label(recording, rec).clicked() {
        let _ = app.run("transport.record", json!({}));
    }
    let f = app.engine.session().edit.main_counter;
    let pos = fmt(app, app.position(), f);
    ui.label(RichText::new(pos).font(mono(13.0)));
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
    let field = egui::TextEdit::singleline(&mut text).id(id).desired_width(62.0).font(mono(15.0)).horizontal_align(egui::Align::RIGHT);
    let resp = ui.add_enabled(!recording, field);
    let label = ui.label(RichText::new("BPM").strong());
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
        ui.label(RichText::new("MAP").small().strong()).on_hover_text(where_);
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
        .add_enabled(!recording, egui::Button::new(RichText::new(format!("{num}/{den}")).font(mono(13.0))))
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
    if ui.selectable_label(click, "Click").on_hover_text("Metronome (Options › Click)").clicked() {
        let _ = app.run("options.click", json!({}));
    }
    if ui
        .selectable_label(countoff, format!("Count Off {bars}"))
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
    let toggle = ui
        .selectable_label(enabled, RichText::new("⟲ Loop").strong())
        .on_hover_text("Loop Playback on/off (Options › Loop Playback). Recording is never started by this button.");
    if toggle.clicked() {
        let _ = app.run("options.loop_playback", json!({}));
    }
    let range_text = match (stored, sel.is_empty()) {
        (Some(r), _) => format!("{} – {}", fmt(app, r.start, f), fmt(app, r.end, f)),
        (None, false) => "selection".to_string(),
        (None, true) => "not set".to_string(),
    };
    let resp = ui.button(format!("Loop: {range_text} ▾")).on_hover_text("Loop range and settings");
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
