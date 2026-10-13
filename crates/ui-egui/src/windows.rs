//! Secondary floating windows (Window menu).

use crate::SoundApp;
use crate::theme::{Tokens, mono, regular, rgb};
use egui::{Color32, Sense, vec2};
use serde_json::json;
use soundcraft_model::{AutomationMode, ClipContent};

pub fn show(app: &mut SoundApp, ctx: &egui::Context) {
    audio_health(app, ctx);
    automation(app, ctx);
    color_palette(app, ctx);
    disk_usage(app, ctx);
    system_usage(app, ctx);
    task_manager(app, ctx);
    metadata(app, ctx);
    event_list(app, ctx);
    midi_keyboard(app, ctx);
    workspace(app, ctx);
    configurations(app, ctx);
    playback_engine(app, ctx);
    recover(app, ctx);
    io_setup(app, ctx);
    shortcuts_window(app, ctx);
    clip_effects(app, ctx);
    #[cfg(not(target_arch = "wasm32"))]
    remote_window(app, ctx);
    help_window(app, ctx);
    assistant_window(app, ctx);
}

/// Window › Command Help (R22): build-versioned, searchable knowledge over the real registry
/// (engine `help.search`) plus the UI-layer commands, each with the active profile's binding,
/// status, risk and the build's stated limits.
fn help_window(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_help;
    let t = Tokens::current();
    let mac = ctx.os().is_mac();
    win(ctx, &mut open, "Command Help", vec2(720.0, 520.0), |ui| {
        ui.horizontal(|ui| {
            ui.label("Search");
            ui.add(egui::TextEdit::singleline(&mut app.ui.help_query).desired_width(300.0).hint_text("what do you want to do?"));
            ui.label(
                egui::RichText::new(format!("build {} · profile {}", env!("CARGO_PKG_VERSION"), app.ui.shortcut_profile.label()))
                    .small()
                    .color(t.text_dim),
            );
        });
        let q = app.ui.help_query.trim().to_string();
        let r =
            app.engine.execute("help.search", &json!({"query": q, "limit": 60})).unwrap_or_else(|e| json!({"results": [], "error": e.to_string()}));
        let mut rows: Vec<serde_json::Value> = r["results"].as_array().cloned().unwrap_or_default();
        // UI-layer commands (windows, themes, shortcuts, remote, monitor) are part of the answer too.
        let ql = q.to_lowercase();
        for (id, label, path, _) in crate::menus::UI_COMMANDS {
            if ql.is_empty() || id.contains(&ql) || label.to_lowercase().contains(&ql) || path.to_lowercase().contains(&ql) {
                rows.push(json!({"id": id, "label": label, "menu": path.split(" > ").collect::<Vec<_>>(), "status": "implemented", "risk": "ui", "undoable": false, "params": "", "limits": []}));
            }
        }
        ui.label(
            egui::RichText::new(format!("{} matches — commands this build has; a tool that is not listed does not exist here.", rows.len()))
                .small()
                .color(t.text_dim),
        );
        egui::ScrollArea::vertical().show(ui, |ui| {
            for row in rows.iter().take(80) {
                let id = row["id"].as_str().unwrap_or("");
                let chord = app.shortcut_for(id, None).map(|c| crate::shortcuts::shortcut_label(&c, mac)).unwrap_or_default();
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(row["label"].as_str().unwrap_or("")).strong());
                        ui.label(egui::RichText::new(id).font(mono(10.5)).color(t.text_dim));
                        if !chord.is_empty() {
                            ui.label(egui::RichText::new(chord).font(mono(10.5)));
                        }
                        let status = row["status"].as_str().unwrap_or("");
                        if status.starts_with("DISABLED") {
                            ui.label(egui::RichText::new(status).color(t.rec));
                        }
                        if let Some(reason) = row["disabled_reason"].as_str() {
                            ui.label(egui::RichText::new(format!("now: {reason}")).small().color(t.text_dim));
                        }
                    });
                    let menu =
                        row["menu"].as_array().map(|m| m.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(" › ")).unwrap_or_default();
                    if !menu.is_empty() {
                        ui.label(egui::RichText::new(menu).small());
                    }
                    let params = row["params"].as_str().unwrap_or("");
                    if !params.is_empty() {
                        ui.label(egui::RichText::new(params).font(mono(10.0)).color(t.text_dim));
                    }
                    ui.label(
                        egui::RichText::new(format!(
                            "risk {} · {}",
                            row["risk"].as_str().unwrap_or("?"),
                            if row["undoable"] == true { "undoable" } else { "no undo entry" }
                        ))
                        .small(),
                    );
                    for l in row["limits"].as_array().into_iter().flatten().filter_map(|l| l.as_str()) {
                        ui.label(egui::RichText::new(format!("limit: {l}")).small().color(t.solo));
                    }
                });
            }
        });
    });
    app.ui.show_help = open;
}

/// Window › SML Remote (R21): opt-in start/stop on an explicit interface, pairing code with its
/// countdown, paired clients with revocation, the redacted event log and the stated limits.
#[cfg(not(target_arch = "wasm32"))]
fn remote_window(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_remote;
    let t = Tokens::current();
    win(ctx, &mut open, "SML Remote", vec2(520.0, 460.0), |ui| {
        let running = app.remote.is_some();
        ui.horizontal(|ui| {
            ui.label("Bind");
            ui.add_enabled(!running, egui::TextEdit::singleline(&mut app.ui.remote_bind).desired_width(160.0))
                .on_hover_text("host:port — 127.0.0.1:0 = this computer only; 0.0.0.0:8787 = every interface (your LAN)");
            if !running {
                if ui.button("Start").clicked() {
                    let _ = app.run("remote.start", json!({}));
                }
            } else if ui.button("Stop").clicked() {
                let _ = app.run("remote.stop", json!({}));
            }
            ui.checkbox(&mut app.ui.remote_autostart, "Start with the app");
        });
        if let Some(s) = app.remote.clone() {
            ui.label(egui::RichText::new(format!("Running at {} (plain HTTP)", s.url())).color(t.accent));
            if s.local_addr().ip().is_unspecified() {
                ui.label(egui::RichText::new("Bound to every interface: on the phone, use this computer's LAN address with the same port.").small());
            }
            ui.horizontal(|ui| {
                if ui.button("Open pairing (2 min)").clicked() {
                    let _ = app.run("remote.pair", json!({}));
                }
                match (&app.remote_code, s.pairing_remaining()) {
                    (Some(code), Some(left)) => {
                        ui.label(egui::RichText::new(code).font(mono(26.0)).color(t.counter_text));
                        ui.label(format!("{left} s left"));
                        ctx.request_repaint_after(std::time::Duration::from_millis(500));
                    }
                    _ => {
                        ui.label(egui::RichText::new("no pairing open").small().color(t.text_dim));
                    }
                }
            });
            ui.separator();
            ui.label(egui::RichText::new("Paired phones").strong());
            let clients = s.tokens();
            if clients.is_empty() {
                ui.label(egui::RichText::new("none").small().color(t.text_dim));
            }
            for c in clients {
                ui.horizontal(|ui| {
                    ui.label(format!("{} ({})", c.name, c.id));
                    if ui.small_button("Revoke").clicked() {
                        let _ = app.run("remote.revoke", json!({"id": c.id}));
                    }
                });
            }
            ui.separator();
            egui::CollapsingHeader::new("Events").show(ui, |ui| {
                for e in s.events().iter().rev().take(20) {
                    ui.label(egui::RichText::new(e).font(mono(10.5)));
                }
            });
        } else {
            ui.label(
                egui::RichText::new("Stopped. Starting binds the chosen interface; phones then pair with a one-time code.").small().color(t.text_dim),
            );
        }
        ui.separator();
        ui.label(
            egui::RichText::new("Limits of this build: plain HTTP (no HTTPS, so no installable PWA, no microphone, no encryption — a phone's LAN-IP origin is not localhost); no QR image (type the address and code); receipts confirm acceptance, not sample-accurate execution; a reconnect refreshes the state and can never replay an old Record; disconnecting a phone leaves recording running.")
                .small()
                .color(t.text_dim),
        );
    });
    app.ui.show_remote = open;
}

fn audio_health(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_audio_health;
    win(ctx, &mut open, "Session Audio Health", vec2(620.0, 360.0), |ui| {
        let refresh = ui.button("Refresh Report").clicked();
        if app.audio_health_report.is_none() || refresh {
            app.audio_health_report = Some(app.engine.execute("session.audio_health", &json!({})).map_err(|error| error.to_string()));
        }
        let Some(result) = &app.audio_health_report else { return };
        match result {
            Ok(report) => {
                ui.label(format!("{} audio clips checked · {} issues", report["audio_clips"], report["issue_count"]));
                ui.label(match report["active_audio_end_seconds"].as_f64() {
                    Some(seconds) => format!("Active audio ends at {seconds:.2} seconds"),
                    None => "Active audio end time is unknown (invalid session sample rate)".into(),
                });
                ui.label("Includes alternate takes and muted clips. Refresh after editing.");
                ui.separator();
                if report["healthy"] == true {
                    ui.label("All audio clips have loaded media within source bounds.");
                }
                egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    if let Some(issues) = report["issues"].as_array() {
                        for issue in issues {
                            ui.group(|ui| {
                                let text = |key: &str| issue[key].as_str().unwrap_or("");
                                ui.label(format!(
                                    "{} / {} / {}{}",
                                    text("track"),
                                    text("playlist"),
                                    text("clip"),
                                    if issue["active"] == true { "" } else { " (alternate playlist)" }
                                ));
                                ui.label(text("message"));
                                if let Some(path) = issue["path"].as_str() {
                                    ui.label(path);
                                }
                            });
                        }
                    }
                });
                ui.separator();
                ui.label("Checks loaded media only, not files on disk, signal levels, plugins or routing.");
            }
            Err(error) => {
                ui.label(error.to_string());
            }
        }
    });
    app.ui.show_audio_health = open;
    if !open {
        app.audio_health_report = None;
    }
}

fn win(ctx: &egui::Context, open: &mut bool, title: &str, size: egui::Vec2, body: impl FnOnce(&mut egui::Ui)) {
    if !*open {
        return;
    }
    egui::Window::new(title).open(open).default_size(size).show(ctx, body);
}

fn automation(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_automation;
    win(ctx, &mut open, "Automation", vec2(260.0, 260.0), |ui| {
        ui.label("Write automation for the selected tracks:");
        let tracks = app.engine.session().edit.selected_tracks.clone();
        ui.horizontal_wrapped(|ui| {
            for m in [
                AutomationMode::Off,
                AutomationMode::Read,
                AutomationMode::Touch,
                AutomationMode::Latch,
                AutomationMode::TouchLatch,
                AutomationMode::Write,
                AutomationMode::Trim,
            ] {
                if ui.button(m.label()).clicked() {
                    let ids: Vec<u64> = tracks.iter().map(|t| t.0).collect();
                    let _ = app.run("mix.automation_mode", json!({"tracks": ids, "mode": m.label()}));
                }
            }
        });
        ui.separator();
        ui.label("Enabled parameters");
        for f in ["volume", "pan", "mute", "send level", "send pan", "send mute", "plugin"] {
            let id = format!("automation.enable.{}", f.replace(' ', "_"));
            let mut on = !app.engine.session().edit.flag(&format!("{id}.off"));
            if ui.checkbox(&mut on, f).changed() {
                app.engine.session_mut().edit.set_flag(&format!("{id}.off"), !on);
            }
        }
        ui.separator();
        if ui.button("Write to Current").clicked() {
            let _ = app.run("automation.write_to_current", json!({}));
        }
        if ui.button("Thin All").clicked() {
            let _ = app.run("automation.thin_all", json!({}));
        }
    });
    app.ui.show_automation = open;
}

fn color_palette(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_color_palette;
    win(ctx, &mut open, "Color Palette", vec2(300.0, 140.0), |ui| {
        ui.label("Apply to selected tracks (Shift: selected clips)");
        ui.horizontal_wrapped(|ui| {
            for (i, c) in soundcraft_model::TRACK_COLORS.iter().enumerate() {
                let (r, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
                ui.painter().rect_filled(r, 3.0, rgb(*c));
                if resp.clicked() {
                    if ui.input(|x| x.modifiers.shift) {
                        let _ = app.run("clip.color", json!({"color": c}));
                    } else {
                        let _ = app.run("track.color", json!({"color": i}));
                    }
                }
            }
        });
    });
    app.ui.show_color_palette = open;
}

fn disk_usage(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_disk_usage;
    win(ctx, &mut open, "Disk Usage", vec2(360.0, 160.0), |ui| {
        let s = app.engine.session();
        let bytes: u64 = s.sources.iter().map(|x| x.frames * u64::from(x.channels) * 4).sum();
        ui.label(format!("Session media: {} files, {:.1} MB in memory (32-bit float)", s.sources.len(), bytes as f64 / 1_048_576.0));
        let per_min = f64::from(s.sample_rate.hz()) * 60.0 * 3.0 / 1_048_576.0;
        ui.label(format!("Recording at 24-bit uses {per_min:.1} MB per mono track-minute"));
        if let Some(p) = &app.engine.path {
            ui.label(format!("Session file: {p}"));
        }
    });
    app.ui.show_disk_usage = open;
}

fn system_usage(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_system_usage;
    win(ctx, &mut open, "System Usage", vec2(320.0, 140.0), |ui| {
        ui.label(format!("UI frame time: {:.1} ms", app.frame_ms));
        ui.label(format!("Audio: {}", app.player.as_ref().map_or("no engine".to_string(), |p| format!("{} @ {} Hz", p.device_name, p.device_rate))));
        let plugins: usize = app.engine.session().tracks.iter().map(|t| t.mixer.inserts.iter().flatten().count()).sum();
        ui.label(format!("Active plugin instances: {plugins}"));
        ui.label(format!("Tracks: {}", app.engine.session().tracks.len()));
        ui.label(format!("Undo steps: {}", app.engine.undo_history().len()));
    });
    app.ui.show_system_usage = open;
}

fn task_manager(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_task_manager;
    win(ctx, &mut open, "Task Manager", vec2(320.0, 120.0), |ui| {
        ui.label("No background tasks are running.");
        ui.label(
            egui::RichText::new("Renders, bounces and AudioSuite processes run to completion before returning.")
                .small()
                .color(Tokens::current().text_dim),
        );
    });
    app.ui.show_task_manager = open;
}

fn metadata(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_metadata;
    win(ctx, &mut open, "Metadata Inspector", vec2(380.0, 260.0), |ui| {
        let s = app.engine.session();
        ui.label(egui::RichText::new(&s.name).strong());
        egui::Grid::new("meta").num_columns(2).striped(true).show(ui, |ui| {
            for (k, v) in [
                ("Sample rate", format!("{} Hz", s.sample_rate.hz())),
                ("Bit depth", format!("{:?}", s.bit_depth)),
                ("Timecode rate", s.frame_rate.label().to_string()),
                ("Tracks", s.tracks.len().to_string()),
                ("Clips", s.tracks.iter().map(|t| t.clips().len()).sum::<usize>().to_string()),
                ("Audio files", s.sources.len().to_string()),
                ("Markers", s.markers.len().to_string()),
                (
                    "Length",
                    soundcraft_time::format_position(s.content_end(), soundcraft_time::TimeFormat::MinSecs, s.sample_rate, &s.tempo, s.frame_rate, 0),
                ),
            ] {
                ui.label(k);
                ui.label(v);
                ui.end_row();
            }
        });
        ui.separator();
        let mut c = s.comments.clone();
        ui.label("Session comments");
        if ui.text_edit_multiline(&mut c).changed() {
            app.engine.session_mut().comments = c;
        }
    });
    app.ui.show_metadata = open;
}

fn event_list(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_event_list;
    win(ctx, &mut open, "MIDI Event List", vec2(420.0, 360.0), |ui| {
        let Some(cid) = crate::midi_editor::target_clip(app) else {
            ui.label("Select a MIDI clip.");
            return;
        };
        let notes = match app.engine.session().find_clip(cid).map(|(_, c)| c.content.clone()) {
            Some(ClipContent::Midi { sequence }) => sequence.notes,
            _ => Vec::new(),
        };
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("evl").num_columns(5).striped(true).show(ui, |ui| {
                for h in ["Start (ticks)", "Note", "Vel", "Length", ""] {
                    ui.label(egui::RichText::new(h).strong());
                }
                ui.end_row();
                for (i, n) in notes.iter().enumerate() {
                    ui.label(egui::RichText::new(n.start.to_string()).font(mono(11.0)));
                    ui.label(soundcraft_midi::note_name(n.pitch));
                    let mut v = i64::from(n.velocity);
                    if ui.add(egui::DragValue::new(&mut v).range(1..=127)).changed() {
                        let _ = app.engine.execute("midi.note_edit", &json!({"clip": cid.0, "index": i, "velocity": v}));
                    }
                    ui.label(n.length.to_string());
                    if ui.small_button("✕").clicked() {
                        let _ = app.run("midi.note_delete", json!({"clip": cid.0, "indices": [i]}));
                    }
                    ui.end_row();
                }
            });
        });
    });
    app.ui.show_event_list = open;
}

fn midi_keyboard(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_midi_keyboard;
    win(ctx, &mut open, "MIDI Keyboard", vec2(640.0, 110.0), |ui| {
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width().max(400.0), 80.0), Sense::click());
        let whites = 28;
        let w = r.width() / whites as f32;
        let mut wi = 0;
        let base = 48u8;
        let mut clicked: Option<u8> = None;
        let pos = resp.interact_pointer_pos();
        for p in base..base + 48 {
            if !matches!(p % 12, 1 | 3 | 6 | 8 | 10) {
                let kr = egui::Rect::from_min_size(egui::pos2(r.min.x + wi as f32 * w, r.min.y), vec2(w - 1.0, r.height()));
                ui.painter().rect_filled(kr, 2.0, Color32::from_rgb(235, 235, 235));
                if resp.clicked() && pos.is_some_and(|q| kr.contains(q)) {
                    clicked = Some(p);
                }
                wi += 1;
            }
        }
        wi = 0;
        for p in base..base + 48 {
            if matches!(p % 12, 1 | 3 | 6 | 8 | 10) {
                let kr = egui::Rect::from_min_size(egui::pos2(r.min.x + wi as f32 * w - w * 0.3, r.min.y), vec2(w * 0.6, r.height() * 0.6));
                ui.painter().rect_filled(kr, 2.0, Color32::from_rgb(20, 20, 20));
                if resp.clicked() && pos.is_some_and(|q| kr.contains(q)) {
                    clicked = Some(p);
                }
            } else {
                wi += 1;
            }
        }
        if let Some(p) = clicked
            && let Some(cid) = crate::midi_editor::target_clip(app)
        {
            // Step input: add the note at the insertion point (relative to the clip) and advance.
            let s = app.engine.session();
            let (clip_start, at) = (s.find_clip(cid).map_or(0, |(_, c)| c.start), s.edit.selection.start);
            let rel = s.tempo.samples_to_ticks(at, s.sample_rate) - s.tempo.samples_to_ticks(clip_start, s.sample_rate);
            let len = soundcraft_time::TICKS_PER_QUARTER / 2;
            let _ = app.run("midi.note_add", json!({"clip": cid.0, "pitch": p, "start_ticks": rel.max(0), "length_ticks": len}));
            let s = app.engine.session();
            let next = s.tempo.tick_to_samples(s.tempo.samples_to_ticks(at, s.sample_rate) + len, s.sample_rate);
            let _ = app.run("transport.locate", json!({"at": next}));
        }
        ui.label(egui::RichText::new("Click keys to step-enter notes into the selected MIDI clip at the insertion point.").small());
    });
    app.ui.show_midi_keyboard = open;
}

fn workspace(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_workspace;
    if !open {
        return;
    }
    egui::Window::new("Workspace").open(&mut open).default_size(vec2(460.0, 420.0)).show(ctx, |ui| {
        let dir = if app.ui.workspace_dir.is_empty() { std::env::var("HOME").unwrap_or_else(|_| ".".into()) } else { app.ui.workspace_dir.clone() };
        ui.horizontal(|ui| {
            if ui.button("⬆").on_hover_text("Parent folder").clicked()
                && let Some(p) = std::path::Path::new(&dir).parent()
            {
                app.ui.workspace_dir = p.to_string_lossy().into_owned();
            }
            ui.label(egui::RichText::new(&dir).small());
        });
        ui.separator();
        let mut entries: Vec<(String, bool)> = std::fs::read_dir(&dir)
            .map(|rd| {
                rd.flatten()
                    .filter_map(|e| {
                        let name = e.file_name().to_string_lossy().into_owned();
                        if name.starts_with('.') {
                            return None;
                        }
                        let is_dir = e.file_type().is_ok_and(|t| t.is_dir());
                        let ext = std::path::Path::new(&name).extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
                        (is_dir || ["wav", "aif", "aiff", "flac", "mp3", "ogg", "m4a", "caf", "mid", "midi", "scraft"].contains(&ext.as_str()))
                            .then_some((name, is_dir))
                    })
                    .collect()
            })
            .unwrap_or_default();
        entries.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.to_lowercase().cmp(&b.0.to_lowercase())));
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (name, is_dir) in entries.into_iter().take(2000) {
                let path = std::path::Path::new(&dir).join(&name).to_string_lossy().into_owned();
                let label = if is_dir { format!("📁 {name}") } else { format!("🔊 {name}") };
                let r = ui.selectable_label(false, label);
                if r.double_clicked() {
                    if is_dir {
                        app.ui.workspace_dir = path;
                    } else if name.ends_with(".scraft") {
                        let _ = app.run("session.open", json!({"path": path}));
                    } else if name.ends_with(".mid") || name.ends_with(".midi") {
                        let _ = app.run("file.import_midi", json!({"path": path}));
                    } else {
                        let at = app.engine.session().edit.selection.start;
                        let _ = app.run("file.import_audio", json!({"path": path, "at": at}));
                    }
                }
            }
        });
        ui.label(egui::RichText::new("Double-click to import audio/MIDI at the insertion point or open a session.").small());
    });
    app.ui.show_workspace = open;
}

fn configurations(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_configurations;
    win(ctx, &mut open, "Window Configurations", vec2(320.0, 240.0), |ui| {
        if ui.button("New Configuration…").clicked() {
            let n = app.ui.configurations.len() + 1;
            let mut snap = app.ui.clone();
            snap.configurations.clear();
            if let Ok(v) = serde_json::to_value(&snap) {
                app.ui.configurations.push((format!("Configuration {n}"), v));
            }
        }
        ui.separator();
        let list = app.ui.configurations.clone();
        for (i, (name, v)) in list.iter().enumerate() {
            ui.horizontal(|ui| {
                if ui.button(format!("{}  {name}", i + 1)).clicked()
                    && let Ok(mut st) = serde_json::from_value::<crate::UiState>(v.clone())
                {
                    if let Err(error) = st.docking.validate().and_then(|()| st.mix_docking.validate_mix()) {
                        app.ui.status = format!("Window configuration could not be restored: {error}");
                    } else {
                        st.configurations = app.ui.configurations.clone();
                        st.theme = app.ui.theme;
                        st.show_configurations = true;
                        crate::docking::cancel_replaced_state(app, &st);
                        app.ui = st;
                    }
                }
                if ui.small_button("✕").clicked() {
                    app.ui.configurations.remove(i);
                }
            });
        }
    });
    app.ui.show_configurations = open;
}

/// Setup › Playback Engine / Hardware.
pub fn playback_engine(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_playback_engine;
    if open {
        let area = crate::control_bar::area_below(ctx);
        egui::Window::new("Audio Setup")
            .open(&mut open)
            .default_size(vec2(560.0, 520.0))
            .default_pos(area.min + vec2(24.0, 8.0))
            .constrain_to(area)
            .show(ctx, |ui| audio_setup(app, ui));
    }
    app.ui.show_playback_engine = open;
    if !open {
        // Closing the window discards anything not applied.
        app.cancel_audio_draft();
    }
}

/// HF06: Audio Setup (Setup › Playback Engine… / Hardware…, Search "Audio Setup", the control
/// bar's ⚙ Audio Setup, a track's input menu). Device choices are drafts until Apply; Cancel
/// returns to what is open. Every value shown comes from the open streams, the session or the
/// mixer; anything the backend does not report says Unknown.
fn audio_setup(app: &mut SoundApp, ui: &mut egui::Ui) {
    let running = app.is_playing() || app.engine.transport.recording;
    let rate = app.engine.session().sample_rate.hz();
    egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
        ui.label(egui::RichText::new("Basic").strong());
        ui.label(
            egui::RichText::new(
                "Audio host: the platform default (WASAPI on Windows). ASIO is not in this build (it needs Steinberg's SDK at build time).",
            )
            .small(),
        );
        egui::Grid::new("as_basic").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
            // Output device (draft).
            ui.label("Output device");
            ui.horizontal(|ui| {
                if ui.button("Scan").on_hover_text("Scan output devices").clicked() {
                    app.ui.output_devices = soundcraft_playback::device::enumerate(soundcraft_playback::device::Direction::Output);
                    if app.ui.output_devices.is_empty() {
                        app.ui.status = "No output devices found by this build's audio hosts".into();
                    }
                }
                let label = app.ui.device_choice.as_ref().map_or("default device".to_string(), |(h, d)| format!("{h}: {d}"));
                egui::ComboBox::from_id_salt("pe_device").selected_text(label).width(300.0).show_ui(ui, |ui| {
                    if ui.selectable_label(app.ui.device_choice.is_none(), "default device").clicked() {
                        app.ui.device_choice = None;
                    }
                    for d in app.ui.output_devices.clone() {
                        let sel = app.ui.device_choice.as_ref() == Some(&(d.host.clone(), d.name.clone()));
                        if ui.selectable_label(sel, format!("{}: {}", d.host, d.name)).clicked() {
                            app.ui.device_choice = Some((d.host, d.name));
                        }
                    }
                });
            });
            ui.end_row();
            ui.label("");
            ui.label(egui::RichText::new(format!("open now: {}", crate::control_bar::device_summary(app))).small());
            ui.end_row();
            // Input device (draft).
            ui.label("Input device (microphone / interface)");
            ui.horizontal(|ui| {
                if ui.button("Scan").on_hover_text("Scan input devices").clicked() {
                    app.ui.input_devices = soundcraft_playback::device::enumerate(soundcraft_playback::device::Direction::Input);
                    if app.ui.input_devices.is_empty() {
                        app.ui.status = "No input devices found by this build's audio hosts".into();
                    }
                }
                let shown = match &app.ui.input_draft {
                    Some(Some(r)) => format!("{}: {}", r.host.as_deref().unwrap_or("?"), r.device.as_deref().unwrap_or("?")),
                    Some(None) => "default input".to_string(),
                    None => app.ui.input_request.as_ref().and_then(|r| Some(format!("{}: {}", r.host.as_deref()?, r.device.as_deref()?))).unwrap_or_else(|| "default input".into()),
                };
                egui::ComboBox::from_id_salt("pe_input").selected_text(shown).width(300.0).show_ui(ui, |ui| {
                    if ui.selectable_label(false, "default input").clicked() {
                        app.ui.input_draft = Some(None);
                    }
                    for d in app.ui.input_devices.clone() {
                        if ui.selectable_label(false, format!("{}: {}", d.host, d.name)).clicked() {
                            let buffer = (app.ui.buffer_choice > 0).then_some(app.ui.buffer_choice);
                            match soundcraft_playback::device::input_request(&d, rate, buffer) {
                                Some(r) => app.ui.input_draft = Some(Some(r)),
                                // Never substitute another input: say why this one cannot be used.
                                None => app.ui.status = format!("`{}` has no 32-bit float input at {rate} Hz", d.name),
                            }
                        }
                    }
                });
            });
            ui.end_row();
            ui.label("");
            ui.label(
                egui::RichText::new(app.recorder.as_ref().map_or("open now: none (opened on the first record or arm)".to_string(), |r| {
                    format!("open now: {} — {} input channel(s) @ {} Hz", r.device_name, r.channels, r.sample_rate)
                }))
                .small(),
            );
            ui.end_row();
            // Sample rate.
            ui.label("Sample rate");
            let dev = app.player.as_ref().map(|p| p.device_rate);
            ui.label(match dev {
                Some(d) if d != rate => format!("session {rate} Hz · output device runs at {d} Hz (converted)"),
                Some(d) => format!("{d} Hz (session and output device)"),
                None => format!("session {rate} Hz · no output device"),
            });
            ui.end_row();
            // Buffer (draft).
            ui.label("Buffer size (requested)");
            egui::ComboBox::from_id_salt("pe_buffer")
                .selected_text(if app.ui.buffer_choice == 0 { "default buffer".to_string() } else { format!("{} frames", app.ui.buffer_choice) })
                .show_ui(ui, |ui| {
                    for b in [0u32, 32, 64, 128, 256, 512, 1024] {
                        let l = if b == 0 { "default buffer".to_string() } else { format!("{b} frames ({:.1} ms)", f64::from(b) * 1000.0 / f64::from(rate.max(1))) };
                        if ui.selectable_label(app.ui.buffer_choice == b, l).clicked() {
                            app.ui.buffer_choice = b;
                        }
                    }
                });
            ui.end_row();
            ui.label("Buffer (delivered)");
            ui.label(app.player.as_ref().map_or("no output stream".to_string(), |p| {
                let (_, _, last) = p.stream_clock();
                if last == 0 {
                    "no callback yet".to_string()
                } else {
                    format!("{last} frames per callback ({:.2} ms) — what the driver actually delivers", f64::from(last) * 1000.0 / f64::from(p.device_rate.max(1)))
                }
            }));
            ui.end_row();
            // Monitoring.
            ui.label("Monitoring");
            ui.horizontal(|ui| {
                let hw = app.engine.session().edit.value("monitor.mode", 1.0) as i64 == 2;
                if ui.add_enabled(!running, egui::Button::selectable(!hw, "Software (through SML Studio)")).clicked() {
                    let _ = app.run("mix.monitor_mode", json!({"mode": "tracking"}));
                }
                if ui.add_enabled(!running, egui::Button::selectable(hw, "Hardware / direct (interface)")).on_disabled_hover_text("Change while stopped").clicked() {
                    let _ = app.run("mix.monitor_mode", json!({"mode": "hardware"}));
                }
            });
            ui.end_row();
        });
        ui.label(
            egui::RichText::new(
                "Software monitoring plays armed/input-monitored tracks through the mixer (with its latency and plugins). Hardware/direct turns SML Studio's input monitoring off so you use your interface's own direct monitoring; SML Studio cannot control the interface's mixer, gain, phantom power or headphone mix. Use one or the other, not both, or you will hear the voice twice.",
            )
            .small(),
        );
        ui.horizontal(|ui| {
            let changed = app.audio_draft_pending();
            if ui
                .add_enabled(!running && changed, egui::Button::new(egui::RichText::new("Apply").strong()))
                .on_disabled_hover_text(if running { "Stop the transport (and any recording) first" } else { "Nothing to apply" })
                .clicked()
            {
                app.ui.status = app.apply_audio_draft();
            }
            if ui.add_enabled(changed, egui::Button::new("Cancel")).clicked() {
                app.cancel_audio_draft();
            }
            if app.ui.audio_output_missing {
                ui.label(egui::RichText::new("the saved output is not open — Apply retries it").small().italics());
            } else if changed {
                ui.label(egui::RichText::new("unapplied changes").small().italics());
            }
        });
        ui.separator();
        selected_track_input(app, ui);
        ui.separator();
        egui::CollapsingHeader::new("Advanced / Latency").id_salt("as_adv").default_open(false).show(ui, |ui| latency_ledger(app, ui));
    });
}

/// The selected track's recording input: which channel(s) of the input device it records.
/// This is per track (`track.input`), separate from choosing the device itself.
fn selected_track_input(app: &mut SoundApp, ui: &mut egui::Ui) {
    ui.label(egui::RichText::new("Selected track input").strong());
    let track = {
        let s = app.engine.session();
        app.ui
            .audio_focus_track
            .and_then(|id| s.track(id))
            .or_else(|| s.edit.selected_tracks.iter().filter_map(|id| s.track(*id)).find(|t| t.kind.has_playlist()))
            .map(|t| {
                let input = match &t.mixer.input {
                    soundcraft_model::Route::Hardware(h) => h.clone(),
                    // An audio track with no input set records In 1 (as its strip shows).
                    soundcraft_model::Route::None => "In 1".to_string(),
                    soundcraft_model::Route::Bus(b) => s.bus(*b).map_or_else(|| "bus".to_string(), |b| format!("bus {}", b.name)),
                    soundcraft_model::Route::Main => "Main".to_string(),
                };
                (t.id, t.name.clone(), t.format.channels(), input, t.mixer.record_arm, t.mixer.input_monitor)
            })
    };
    let Some((id, name, width, input, armed, monitored)) = track else {
        ui.label("Select an audio track (or open this from its input selector) to choose which input it records.");
        return;
    };
    let device_channels = app.recorder.as_ref().map(|r| r.channels);
    let n = device_channels.unwrap_or(8).max(1);
    let current = input;
    ui.horizontal(|ui| {
        ui.strong(&name);
        egui::ComboBox::from_id_salt("as_track_input").selected_text(current.as_str()).show_ui(ui, |ui| {
            for k in 1..=n {
                // The same values as the channel strip's input menu: "In k" (a stereo track
                // records k and k+1).
                let value = format!("In {k}");
                let shown = if width > 1 { format!("In {k} (inputs {k}–{})", k + width - 1) } else { value.clone() };
                let ok = width == 1 || k + width - 1 <= n || n == 1;
                let r = ui
                    .add_enabled(ok, egui::Button::selectable(current == value, &shown))
                    .on_disabled_hover_text("The input device does not have these channels");
                if r.clicked() {
                    let _ = app.run("track.input", json!({"tracks": [id.0], "input": value}));
                }
            }
        });
    });
    match device_channels {
        Some(ch) => {
            let first = current.trim_start_matches("In ").split(['-', ' ']).next().and_then(|x| x.trim().parse::<usize>().ok()).unwrap_or(1);
            // A one-input device may feed a stereo track (the same signal on both sides).
            if first > ch || (ch > 1 && first + width - 1 > ch) {
                ui.colored_label(
                    egui::Color32::from_rgb(230, 110, 90),
                    format!(
                        "{current} is not available on the open input device ({ch} channel(s)): recording will refuse rather than use another input."
                    ),
                );
            } else {
                ui.label(egui::RichText::new(format!("Records {current} of the open input device ({ch} channel(s)).")).small());
            }
        }
        None => {
            ui.label(egui::RichText::new("The input device opens on the first record or arm; its channel count is not known yet.").small());
        }
    }
    if armed || monitored {
        let lvl = app.meters.get(&id).map_or(0.0, |m| m.level[0].max(m.level[1]));
        ui.add(egui::ProgressBar::new(lvl.clamp(0.0, 1.0)).text(if lvl > 0.0 {
            format!("input {:.1} dBFS", 20.0 * lvl.max(1e-6).log10())
        } else {
            "input: no signal".into()
        }));
    } else {
        ui.label(egui::RichText::new("Arm or input-monitor the track to meter its input (no test signal is ever generated).").small());
    }
}

/// HF06 latency ledger: each term with where it comes from; nothing measured is invented.
fn latency_ledger(app: &mut SoundApp, ui: &mut egui::Ui) {
    let rate = app.engine.session().sample_rate.as_f64();
    let ms = |frames: f64| frames * 1000.0 / rate.max(1.0);
    egui::Grid::new("as_latency").num_columns(2).spacing(vec2(12.0, 4.0)).show(ui, |ui| {
        if let Some(p) = &app.player {
            let (generation, frames, last) = p.stream_clock();
            ui.label("Requested buffer");
            ui.label(p.negotiated.as_ref().and_then(|n| n.requested_buffer).map_or("backend default".to_string(), |b| format!("{b} frames")));
            ui.end_row();
            ui.label("Driver-reported buffer range");
            ui.label(p.negotiated.as_ref().map_or("not queried".to_string(), |n| match n.reported_buffer {
                soundcraft_playback::device::BufferRange::Range { min, max } => format!("{min}–{max} frames"),
                soundcraft_playback::device::BufferRange::Unknown => "Unknown (not reported)".to_string(),
            }));
            ui.end_row();
            ui.label("Delivered callback (observed)");
            ui.label(if last == 0 {
                "no callback yet".to_string()
            } else {
                format!("{last} frames = {:.2} ms", f64::from(last) * 1000.0 / f64::from(p.device_rate.max(1)))
            });
            ui.end_row();
            ui.label("Output buffering (calculated)");
            ui.label(if last == 0 {
                "—".to_string()
            } else {
                format!(
                    "≥ {last} frames ≈ {:.2} ms (one callback; an estimate, not a measurement)",
                    f64::from(last) * 1000.0 / f64::from(p.device_rate.max(1))
                )
            });
            ui.end_row();
            ui.label("Stream clock");
            ui.label(format!("generation {generation}, {frames} frames serviced"));
            ui.end_row();
        } else {
            ui.label("Output");
            ui.label("no output stream");
            ui.end_row();
        }
        ui.label("Input buffer (requested)");
        ui.label(app.recorder.as_ref().map_or("input not open".to_string(), |r| {
            r.config.as_ref().and_then(|n| n.requested_buffer).map_or("backend default".to_string(), |b| format!("{b} frames"))
        }));
        ui.end_row();
        ui.label("Driver input/output latency");
        ui.label("Unknown (not reported by this backend; not assumed zero)");
        ui.end_row();
        let s = app.engine.session();
        let worst = soundcraft_mix::strip_latencies(s).into_iter().map(|(_, l)| l).max().unwrap_or(0);
        let sandboxed = s.tracks.iter().flat_map(|t| t.mixer.inserts.iter().flatten()).filter(|i| i.plugin.starts_with("sandbox:")).count();
        ui.label("Plugin / host path (delay compensation)");
        ui.label(format!(
            "{worst} samples = {:.2} ms (largest channel){}",
            ms(worst as f64),
            if sandboxed > 0 { format!("; includes the one-block delay of {sandboxed} sandboxed plugin(s)") } else { String::new() }
        ));
        ui.end_row();
        ui.label("Measured round trip");
        ui.label("Not measured: this build has no loopback calibration");
        ui.end_row();
    });
    let mut dc = app.engine.session().edit.delay_compensation;
    if ui.checkbox(&mut dc, "Delay compensation").changed() {
        let _ = app.run("options.delay_compensation", json!({"value": dc}));
    }
    if ui
        .button("Reopen the system default output")
        .on_hover_text("Recovers after a device was unplugged; opens the default output, not a chosen one")
        .clicked()
    {
        app.player = Some(soundcraft_playback::Player::new(app.engine.session_arc()));
    }
}

/// File › Recover Autosave…: recovery points (rolling autosave generations per session) and
/// unfinished takes (capture journals still "recording"). Nothing is parsed or changed until a
/// button is pressed; recovery opens an unsaved session and never overwrites originals.
pub fn recover(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_recover;
    win(ctx, &mut open, "Recover Autosave", vec2(640.0, 380.0), |ui| {
        ui.label(egui::RichText::new(app.autosave_summary()).small());
        ui.separator();
        ui.label(egui::RichText::new("Recovery points").strong());
        let points = app.recovery_points();
        if points.is_empty() {
            ui.label("No autosave generations found.");
        }
        let mut open_point = None;
        egui::Grid::new("recover_points").num_columns(5).striped(true).show(ui, |ui| {
            for (i, p) in points.iter().enumerate() {
                ui.label(&p.session_name);
                ui.label(format!("generation {}", p.generation));
                ui.label(crate::unix_hms(p.unix_time));
                ui.label(p.plugin_state_age_s.map_or("plugin state: not captured".to_string(), |a| format!("plugin state {a} s older")));
                if ui.button("Open as unsaved session").clicked() {
                    open_point = Some(i);
                }
                ui.end_row();
            }
        });
        if let Some(i) = open_point
            && let Some(p) = points.get(i)
            && let Err(e) = app.open_recovery_point(p)
        {
            app.ui.status = format!("Recovery failed: {e}");
        }
        ui.separator();
        ui.label(egui::RichText::new("Unfinished takes").strong());
        let takes = app.unfinished_takes();
        if takes.is_empty() {
            ui.label("No unfinished takes beside this session or in the temp capture folder.");
        }
        let mut recover_take = None;
        egui::Grid::new("recover_takes").num_columns(4).striped(true).show(ui, |ui| {
            for (i, t) in takes.iter().enumerate() {
                ui.label(t.media.file_name().and_then(|n| n.to_str()).unwrap_or("?"));
                ui.label(format!("{} durable frames @ {} Hz, {} ch", t.journal.durable_frames, t.journal.sample_rate, t.journal.channels));
                ui.label(format!("{:?}{}", t.journal.state, t.journal.error.as_deref().map(|e| format!(" — {e}")).unwrap_or_default()));
                if ui.button("Recover to a new track").clicked() {
                    recover_take = Some(i);
                }
                ui.end_row();
            }
        });
        if let Some(i) = recover_take
            && let Some(t) = takes.get(i)
            && let Err(e) = app.recover_take(t)
        {
            app.ui.status = format!("Take recovery failed: {e}");
        }
        ui.separator();
        ui.label(egui::RichText::new("Recovery keeps every validated generation and every journaled frame; nothing here deletes or overwrites a file. Media written after the last durable checkpoint is discarded and reported.").small());
    });
    app.ui.show_recover = open;
}

/// Setup › I/O: busses and output paths.
pub fn io_setup(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_io_setup;
    win(ctx, &mut open, "I/O Setup", vec2(420.0, 300.0), |ui| {
        ui.label(egui::RichText::new("Busses").strong());
        let busses: Vec<(String, String)> = app.engine.session().busses.iter().map(|b| (b.name.clone(), b.format.label().to_string())).collect();
        egui::Grid::new("busses").num_columns(3).striped(true).show(ui, |ui| {
            for (name, fmt) in busses {
                let key = egui::Id::new(("bus_name", name.clone()));
                let mut edit: String = ui.ctx().memory(|m| m.data.get_temp(key)).unwrap_or_else(|| name.clone());
                let r = ui.text_edit_singleline(&mut edit);
                ui.ctx().memory_mut(|m| m.data.insert_temp(key, edit.clone()));
                if r.lost_focus() && edit != name {
                    let _ = app.run("setup.io", json!({"action": "rename_bus", "name": name, "new_name": edit}));
                }
                ui.label(fmt);
                if ui.small_button("Delete").clicked() {
                    let _ = app.run("setup.io", json!({"action": "delete_bus", "name": name}));
                }
                ui.end_row();
            }
        });
        if ui.button("New Bus").clicked() {
            let n = app.engine.session().busses.len() + 1;
            let _ = app.run("mix.new_bus", json!({"name": format!("Bus {n}")}));
        }
        ui.separator();
        ui.label(egui::RichText::new("Outputs").strong());
        // Main output format: stereo, or a surround format (the mix and master faders follow).
        let main = app.engine.session().main_format();
        ui.horizontal(|ui| {
            ui.label("Main output");
            egui::ComboBox::from_id_salt("main_output_format").selected_text(main.label()).show_ui(ui, |ui| {
                for f in soundcraft_model::ChannelFormat::ALL.into_iter().filter(|f| (2..=16).contains(&f.channels())) {
                    if ui.selectable_label(f == main, f.label()).clicked() && f != main {
                        let _ = app.run("setup.main_format", json!({"format": f.label()}));
                    }
                }
            });
            ui.label(egui::RichText::new(format!("{} ch", main.channels())).small());
        });
        for o in app.engine.session().outputs.clone() {
            ui.label(format!("{} — {} from channel {}", o.name, o.format.label(), o.first_channel + 1));
        }
    });
    app.ui.show_io_setup = open;
}

/// Setup › Keyboard Shortcuts.
/// Keyboard Shortcuts (R13): profile selection, search, reassignment by capture, unbind,
/// reset, import/export and the conflict inspector. Every row shows the active binding.
pub fn shortcuts_window(app: &mut SoundApp, ctx: &egui::Context) {
    use crate::shortcut_profiles::{ConflictKind, Profile};
    let mut open = app.ui.show_shortcuts;
    let mac = ctx.os().is_mac();
    let t = Tokens::current();
    win(ctx, &mut open, "Keyboard Shortcuts", vec2(640.0, 560.0), |ui| {
        ui.horizontal(|ui| {
            ui.label("Profile");
            let cur = app.ui.shortcut_profile;
            egui::ComboBox::from_id_salt("shortcut_profile").selected_text(cur.label()).show_ui(ui, |ui| {
                for p in Profile::ALL {
                    if ui.selectable_label(cur == p, p.label()).clicked() && p != cur {
                        let _ = app.run("ui.shortcut_profile", json!({"profile": p.id()}));
                    }
                }
            });
            if app.ui.shortcut_profile == Profile::Custom {
                ui.label("based on");
                let base = app.ui.shortcut_base;
                egui::ComboBox::from_id_salt("shortcut_base").selected_text(base.label()).show_ui(ui, |ui| {
                    for p in [Profile::ProTools, Profile::StudioOne] {
                        if ui.selectable_label(base == p, p.label()).clicked() && p != base {
                            let _ = app.run("ui.shortcut_profile", json!({"base": p.id()}));
                        }
                    }
                });
            }
            if ui.button("Reset").on_hover_text("Drop every Custom override").clicked() {
                let _ = app.run("ui.shortcuts_reset", json!({}));
            }
            if ui.button("Import…").clicked() {
                let _ = app.run("ui.shortcuts_import", json!({}));
            }
            if ui.button("Export…").clicked() {
                let _ = app.run("ui.shortcuts_export", json!({}));
            }
        });
        ui.label(
            egui::RichText::new("Pro Tools-style chords come from the command registry; Studio One-style chords are best-known defaults pending verification against the manual (see the shortcut inventory). A reassignment switches to the Custom profile; the built-in tables are never edited.")
                .small()
                .color(t.text_dim),
        );
        ui.horizontal(|ui| {
            ui.label("Search");
            ui.add(egui::TextEdit::singleline(&mut app.shortcut_query).desired_width(220.0));
            if let Some(k) = &app.shortcut_capture {
                ui.label(egui::RichText::new(format!("Press the new chord for {k} (Escape cancels)")).color(t.accent));
            }
        });
        let table = app.shortcut_table();
        let conflicts = crate::shortcut_profiles::conflicts(&table);
        if !conflicts.is_empty() {
            egui::CollapsingHeader::new(format!("Conflicts ({})", conflicts.len())).default_open(true).show(ui, |ui| {
                for c in &conflicts {
                    let kind = match c.kind {
                        ConflictKind::Duplicate => "duplicate",
                        ConflictKind::OsReserved => "OS-reserved",
                        ConflictKind::Undeliverable => "not deliverable",
                    };
                    ui.label(format!("{} — {kind}: {} — {}", crate::shortcuts::shortcut_label(&c.chord, mac), c.commands.join(", "), c.note));
                }
            });
        }
        let q = app.shortcut_query.to_lowercase();
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("sc").num_columns(4).striped(true).show(ui, |ui| {
                for e in table
                    .iter()
                    .filter(|e| q.is_empty() || e.label.to_lowercase().contains(&q) || e.command.contains(&q) || e.chord.to_lowercase().contains(&q))
                {
                    let shown = crate::shortcuts::shortcut_label(&e.chord, mac);
                    let capture = app.shortcut_capture.as_deref() == Some(e.key().as_str());
                    let label = if capture {
                        "…".to_string()
                    } else if shown.is_empty() {
                        "(macOS only)".to_string()
                    } else {
                        shown
                    };
                    if ui
                        .add(egui::Button::new(egui::RichText::new(label).font(mono(11.0))).min_size(vec2(120.0, 18.0)))
                        .on_hover_text("Click, then press the new chord")
                        .clicked()
                    {
                        app.shortcut_capture = Some(e.key());
                    }
                    ui.label(&e.label);
                    ui.label(
                        egui::RichText::new(format!("{}{}", e.command, if e.verified { "" } else { " · unverified" })).small().color(t.text_dim),
                    );
                    if ui.small_button("Unbind").clicked() {
                        let _ = app.run("ui.shortcut_set", json!({"key": e.key(), "chord": ""}));
                    }
                    ui.end_row();
                }
            });
        });
    });
    app.ui.show_shortcuts = open;
}

/// Clip Effects: per-clip EQ, dynamics and gain (stored as `clip_fx.<clip>.*` values).
pub fn clip_effects(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_clip_effects;
    win(ctx, &mut open, "Clip Effects", vec2(560.0, 420.0), |ui| {
        let clip = app.engine.session().edit.selected_clips.first().copied().or_else(|| {
            let s = app.engine.session();
            s.tracks
                .iter()
                .filter(|t| s.edit.selected_tracks.contains(&t.id))
                .flat_map(|t| t.clips().iter())
                .find(|c| c.range().contains(s.edit.selection.start) && c.is_audio())
                .map(|c| c.id)
        });
        let Some(cid) = clip else {
            ui.label("Select an audio clip.");
            return;
        };
        let name = app.engine.session().find_clip(cid).map(|(_, c)| c.name.clone()).unwrap_or_default();
        let bypass = app.engine.session().edit.flag(&format!("clip_fx.bypass.{}", cid.0));
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&name).strong());
            if ui.selectable_label(bypass, "Bypass").clicked() {
                let _ = app.run("clip.effects_bypass", json!({"clips": [cid.0]}));
            }
            if ui.button("Render").clicked() {
                let _ = app.run("clip.effects_render", json!({"clips": [cid.0]}));
            }
            if ui.button("Clear").clicked() {
                let _ = app.run("edit.clear_clip_effects", json!({"clips": [cid.0]}));
            }
        });
        let get = |app: &SoundApp, k: &str, d: f32| app.engine.session().edit.values.get(&format!("clip_fx.{}.{k}", cid.0)).map_or(d, |v| *v as f32);
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.columns(2, |cols| {
                for (col, (title, plugin, prefix)) in cols.iter_mut().zip([("EQ", "eq_7band", "eq."), ("Dynamics", "compressor", "comp.")]) {
                    col.label(egui::RichText::new(title).strong());
                    let Some(info) = soundcraft_dsp::plugin_info(plugin) else { continue };
                    for p in info.params {
                        let key = format!("{prefix}{}", p.id);
                        let mut v = get(app, &key, p.default);
                        let before = v;
                        col.add(egui::Slider::new(&mut v, p.min..=p.max).text(p.name));
                        if (v - before).abs() > f32::EPSILON {
                            let mut m = serde_json::Map::new();
                            m.insert(key.clone(), json!(v));
                            let _ = app.engine.execute_merged(
                                "clip.effects_set",
                                &json!({"clips": [cid.0], "params": m}),
                                &format!("cfx:{}:{key}", cid.0),
                            );
                        }
                    }
                }
            });
            let mut g = get(app, "gain", 0.0);
            let before = g;
            ui.add(egui::Slider::new(&mut g, -36.0..=24.0).text("Clip effects gain (dB)"));
            if (g - before).abs() > f32::EPSILON {
                let _ =
                    app.engine.execute_merged("clip.effects_set", &json!({"clips": [cid.0], "params": {"gain": g}}), &format!("cfx:{}:gain", cid.0));
            }
        });
    });
    app.ui.show_clip_effects = open;
}

/// Window › Assistant (R32/R33, RC04/RC05): typed requests or push-to-talk (enrolled phrases),
/// grounded answers or a local model's checked proposals with citations, plan preview with Run
/// (one undo step), cancel/progress, spoken replies on the monitor output and the transcript.
fn assistant_window(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.ui.show_assistant;
    let t = Tokens::current();
    win(ctx, &mut open, "Assistant", vec2(760.0, 560.0), |ui| {
        ui.horizontal(|ui| {
            let r = ui.add(
                egui::TextEdit::singleline(&mut app.ui.assistant_input)
                    .desired_width(380.0)
                    .hint_text("ask, or say what to do (e.g. \"mute Kick\", \"is the lead clipping?\")"),
            );
            let busy = app.assistant.is_busy();
            let go = ui.add_enabled(!busy, egui::Button::new("Ask")).clicked()
                || (!busy && r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
            if let Some(secs) = app.assistant_pending_secs() {
                ui.spinner();
                if ui.button(format!("Cancel ({secs:.0} s)")).clicked() {
                    app.assistant_cancel();
                }
            }
            // Push-to-talk: held = listening; released = recognised off the UI thread.
            let recording_take = app.engine.transport.recording;
            let ptt = ui
                .add_enabled(
                    !recording_take && !busy,
                    egui::Button::new(if app.assistant.is_recording() { "🎤 Listening…" } else { "🎤 Hold to talk" })
                        .sense(egui::Sense::click_and_drag()),
                )
                .on_hover_text("hold while you say an enrolled phrase; release to recognise (never while recording)")
                .on_disabled_hover_text("off while recording or while an answer is pending");
            let held = ptt.is_pointer_button_down_on();
            if held && !app.assistant.is_recording() {
                app.assistant_ptt_press(crate::assistant_ui::PttPurpose::Ask);
            } else if !held && app.assistant.is_recording() && app.assistant.enroll_phrase.is_empty() {
                app.assistant_ptt_release();
            }
            if go && !app.ui.assistant_input.trim().is_empty() {
                let text = app.ui.assistant_input.trim().to_string();
                let shortcuts: serde_json::Map<String, serde_json::Value> = app
                    .engine
                    .command_ids()
                    .into_iter()
                    .filter_map(|id| app.shortcut_for(&id, None).map(|c| (id, json!(crate::shortcuts::shortcut_label(&c, ctx.os().is_mac())))))
                    .collect();
                app.assistant_ask_text(text, serde_json::Value::Object(shortcuts));
                app.ui.assistant_input.clear();
            }
        });
        let model = match &app.engine.voice.model {
            Some(c) => format!("local model at {} (answers checked before any action)", c.url),
            None => "grounded rules (no language model on)".to_string(),
        };
        ui.label(
            egui::RichText::new(format!(
                "Answers: {model} · speech: {} enrolled phrase(s), built-in robotic voice {} · nothing leaves this machine",
                app.engine.voice.recognizer.phrases.len(),
                if app.engine.voice.speak_replies { "on" } else { "off" }
            ))
            .small()
            .color(t.text_dim),
        );
        if !app.assistant.status.is_empty() {
            ui.label(egui::RichText::new(app.assistant.status.clone()).small());
        }
        ui.collapsing("Local language model (optional)", |ui| {
            ui.label(
                egui::RichText::new("SoundCraft ships no model. Run a local OpenAI-compatible server (llama.cpp llama-server, Ollama, LM Studio) and point this at it; only 127.0.0.1 / ::1 / localhost are accepted. The model can only propose actions; each is checked and shown before it runs.")
                    .small()
                    .color(t.text_dim),
            );
            let f = &mut app.assistant.form;
            ui.checkbox(&mut f.enabled, "Use the local model");
            ui.horizontal(|ui| {
                ui.label("Endpoint");
                ui.add(egui::TextEdit::singleline(&mut f.url).desired_width(320.0));
            });
            ui.horizontal(|ui| {
                ui.label("Model");
                ui.add(egui::TextEdit::singleline(&mut f.model).desired_width(160.0));
                ui.label("Time limit (s)");
                ui.add(egui::DragValue::new(&mut f.timeout_s).range(1.0..=600.0));
            });
            if ui.button("Apply").clicked() {
                app.assistant_apply_model();
            }
        });
        ui.collapsing("Voice: spoken replies and push-to-talk phrases", |ui| {
            let mut speak = app.engine.voice.speak_replies;
            let mut pitch = app.engine.voice.voice.pitch_hz;
            let mut changed = ui.checkbox(&mut speak, "Speak replies (monitor output only; silent while recording)").changed();
            changed |= ui.add(egui::Slider::new(&mut pitch, 70.0..=250.0).text("voice pitch (Hz)")).changed();
            if changed {
                let _ = app.engine.execute("assistant.voice", &json!({"speak_replies": speak, "pitch_hz": pitch}));
                app.assistant_save();
            }
            if ui.button("Stop speaking").clicked() {
                app.assistant_cancel();
            }
            ui.separator();
            ui.label(egui::RichText::new("Push-to-talk recognises only the phrases enrolled here (not dictation). Record each phrase in the room and with the microphone you will use; it needs a quiet room.").small().color(t.text_dim));
            let mut remove: Option<String> = None;
            for p in &app.engine.voice.recognizer.phrases {
                ui.horizontal(|ui| {
                    ui.label(format!("\"{}\" ({} recording{})", p.text, p.templates.len(), if p.templates.len() == 1 { "" } else { "s" }));
                    if ui.small_button("Remove").clicked() {
                        remove = Some(p.text.clone());
                    }
                });
            }
            if let Some(r) = remove {
                app.engine.voice.recognizer.remove(&r);
                app.assistant_save();
            }
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut app.assistant.enroll_phrase).desired_width(200.0).hint_text("phrase, e.g. mute kick"));
                let ok = !app.assistant.enroll_phrase.trim().is_empty() && !app.engine.transport.recording;
                let b = ui.add_enabled(ok, egui::Button::new(if app.assistant.is_recording() { "Recording…" } else { "Hold to record phrase" }).sense(egui::Sense::click_and_drag()));
                let held = b.is_pointer_button_down_on();
                if held && !app.assistant.is_recording() {
                    let phrase = app.assistant.enroll_phrase.trim().to_string();
                    app.assistant_ptt_press(crate::assistant_ui::PttPurpose::Enroll(phrase));
                } else if !held && app.assistant.is_recording() && !app.assistant.enroll_phrase.is_empty() {
                    app.assistant_ptt_release();
                }
            });
        });
        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut run: Option<(usize, serde_json::Value)> = None;
            for (i, (q, r)) in app.ui.assistant_log.iter().enumerate().rev() {
                ui.group(|ui| {
                    ui.label(egui::RichText::new(format!("You: {q}")).strong());
                    ui.label(
                        egui::RichText::new(format!("[{}] {}", r["mode"].as_str().unwrap_or(""), r["answer"].as_str().unwrap_or("")))
                            .font(regular(12.0)),
                    );
                    if let Some(c) = r["citations"].as_array().filter(|c| !c.is_empty()) {
                        ui.label(
                            egui::RichText::new(format!("cites: {}", c.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(", ")))
                                .small()
                                .color(t.text_dim),
                        );
                    }
                    if let Some(l) = r["labels"].as_array().filter(|l| !l.is_empty()) {
                        ui.label(egui::RichText::new(l.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(" · ")).small().color(t.text_dim));
                    }
                    if let Some(plan) = r["plan"].as_array().filter(|p| !p.is_empty()) {
                        ui.label(egui::RichText::new("Plan (one undo step):").strong());
                        for s in plan {
                            ui.label(format!(
                                "• {} — {} ({})",
                                s["describe"].as_str().unwrap_or(""),
                                s["command"].as_str().unwrap_or(""),
                                s["risk"].as_str().unwrap_or("")
                            ));
                        }
                        let stale = r["revision"].as_u64() != Some(app.engine.revision);
                        let done = r.get("executed").is_some();
                        if ui
                            .add_enabled(!stale && !done, egui::Button::new("Run plan"))
                            .on_disabled_hover_text(if done { "already run" } else { "the document changed since this plan was built: ask again" })
                            .clicked()
                        {
                            run = Some((i, r.clone()));
                        }
                    }
                });
            }
            if let Some((i, r)) = run {
                let res = app
                    .engine
                    .execute("assistant.execute", &json!({"plan": r["plan"], "revision": r["revision"], "request_id": format!("ui:{}", i)}))
                    .unwrap_or_else(|e| json!({"accepted": false, "reason": e.to_string()}));
                if let Some((_, entry)) = app.ui.assistant_log.get_mut(i) {
                    entry["executed"] = res;
                }
            }
        });
    });
    app.ui.show_assistant = open;
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod audio_health_tests {
    use super::*;

    #[test]
    fn window_command_renders_report_and_close_clears_snapshot() {
        let mut app = SoundApp::new(soundcraft_engine::Engine::default(), None, crate::Services::default());
        app.run("window.audio_health", json!({"value": true})).unwrap();
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| audio_health(&mut app, ui.ctx()));
        output.textures_delta.clear();
        assert_eq!(app.audio_health_report.as_ref().unwrap().as_ref().unwrap()["healthy"], true);
        assert!(!app.engine.is_dirty());
        app.run("window.hide_floating", json!({})).unwrap();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| audio_health(&mut app, ui.ctx()));
        output.textures_delta.clear();
        assert!(!app.ui.show_audio_health);
        assert!(app.audio_health_report.is_none());
        app.run("window.audio_health", json!({"value": true})).unwrap();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| audio_health(&mut app, ui.ctx()));
        output.textures_delta.clear();
        assert!(app.audio_health_report.is_some());
    }
}
