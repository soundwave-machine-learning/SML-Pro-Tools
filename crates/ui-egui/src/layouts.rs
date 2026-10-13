//! Record / Edit / Mix / Master are layouts of one application (R18). Edit and Mix are the
//! existing windows; Record is the Edit layout with the Recording essentials docked on the
//! left (transport, track identity, monitoring state, active tool and Nudge stay in the Edit
//! toolbar); Master is a navigation shell for Sprint 4's mastering workflow: the main strip
//! and meters, visibly labelled incomplete. Switching layouts changes no identity: session
//! format, command ids, automation ids and plugin state keys are untouched.

use crate::theme::{Tokens, bold, regular};
use crate::{MainWindow, SoundApp};
use egui::{Ui, vec2};
use serde_json::json;
use soundcraft_model::TrackKind;

/// Width the switcher needs (four 56-point buttons and their gaps).
pub const SWITCHER_WIDTH: f32 = 4.0 * 56.0 + 3.0 * 6.0;
/// egui temp-data key: the menu bar had no room for the switcher this frame.
pub const OVERFLOW_KEY: &str = "layout_switcher_overflow";

/// Whether the switcher moved from the menu bar into the control bar (compact windows).
pub fn switcher_overflows(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(egui::Id::new(OVERFLOW_KEY))).unwrap_or(false)
}

/// The layout switcher shown at the right of the menu bar (or, on compact windows, at the start
/// of the control bar).
pub fn switcher(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let mut order = [
        (MainWindow::Record, "Record", "window.record"),
        (MainWindow::Edit, "Edit", "window.edit"),
        (MainWindow::Mix, "Mix", "window.mix"),
        (MainWindow::Master, "Master", "window.master"),
    ];
    // Same left-to-right order wherever it is drawn (the menu bar lays out right to left).
    if !ui.layout().prefer_right_to_left() {
        order.reverse();
    }
    for (w, label, cmd) in order {
        let on = app.ui.window == w;
        // The button text is the layout name; its accessible name says "layout" so it never
        // collides with the Edit menu (screen readers and the UI harness address by name).
        if crate::widgets::text_toggle_named(ui, vec2(56.0, 18.0), label, &format!("{label} layout"), on, t.accent, &format!("{label} layout"))
            .clicked()
            && !on
        {
            let _ = app.run(cmd, json!({}));
        }
    }
}

/// Record layout: Recording essentials on the left, the Edit timeline on the right.
pub fn record(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    egui::Panel::left("record_essentials").exact_size(300.0).frame(egui::Frame::NONE.fill(t.panel_bg).inner_margin(egui::Margin::same(8))).show(
        ui,
        |ui| {
            ui.label(egui::RichText::new("RECORD").font(bold(12.0)).color(t.header_text));
            crate::panels::recording_essentials(app, ui);
            ui.separator();
            ui.label(
                egui::RichText::new(
                    "Takes land on the selected track's active playlist; alternate takes are its other playlists (Track › Playlists).",
                )
                .font(regular(10.5))
                .color(t.text_dim),
            );
        },
    );
    crate::edit_window::show(app, ui);
}

/// Master layout (R34): the real Master strip (its user inserts ARE the mastering chain,
/// applied once), the program/reference setup, standard meters over the program (offline
/// analysis on request) and the export settings. Reference, monitor controls and speech never
/// reach an export; the status line says so.
pub fn master(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    egui::Panel::top("master_banner").exact_size(40.0).frame(egui::Frame::NONE.fill(t.toolbar_bg).inner_margin(egui::Margin::same(8))).show(
        ui,
        |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("MASTER").font(bold(13.0)).color(t.header_text));
                if ui.button("Back to Mix").clicked() {
                    let _ = app.run("window.mix", json!({}));
                }
                ui.label(
                    egui::RichText::new(
                        "The chain is the Master track's inserts (applied once). Reference and monitor controls never reach an export.",
                    )
                    .font(regular(11.0))
                    .color(t.text_dim),
                );
            });
        },
    );
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.window_bg).inner_margin(egui::Margin::same(12))).show(ui, |ui| {
        let main = app.engine.session().tracks.iter().find(|x| x.kind == TrackKind::Master).map(|x| x.id);
        let snap = app.player.as_ref().filter(|_| app.engine.transport.playing).map(|p| p.meters());
        let card = |ui: &mut Ui, title: &str, body: &mut dyn FnMut(&mut Ui)| {
            egui::Frame::NONE
                .fill(t.panel_bg)
                .stroke(egui::Stroke::new(1.0, t.border))
                .corner_radius(egui::CornerRadius::same(crate::theme::metric::RADIUS))
                .inner_margin(egui::Margin::same(10))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width().min(560.0));
                    ui.label(egui::RichText::new(title).font(bold(crate::theme::size::CAPTION)).color(t.section_label));
                    ui.add_space(4.0);
                    body(ui);
                });
            ui.add_space(10.0);
        };
        ui.horizontal_top(|ui| {
            match main {
                Some(id) => crate::mix_window::strip(app, ui, id, snap.as_ref()),
                None => {
                    ui.label("This session has no Master track.");
                }
            }
            ui.add_space(16.0);
            egui::ScrollArea::vertical().id_salt("master_cards").show(ui, |ui| {
                ui.vertical(|ui| {
                    card(ui, "PROGRAM AND REFERENCE", &mut |ui| {
                        let st = app.engine.execute("mastering.status", &json!({})).unwrap_or_default();
                        ui.label(format!("Program: {}", st["program"].as_u64().map_or("session mix".to_string(), |s| format!("source {s}"))));
                        let reference = st["reference"].clone();
                        if reference.is_null() {
                            ui.label(
                                egui::RichText::new("Reference: none. Set one with the command mastering.reference {source: <id>} (there is no menu entry for it yet).")
                                    .color(t.text_dim),
                            );
                        } else {
                            let listening = reference["listening"].as_bool().unwrap_or(false);
                            ui.label(format!(
                                "Reference: source {} at {:+.1} dB",
                                reference["source"].as_u64().unwrap_or(0),
                                reference["gain_db"].as_f64().unwrap_or(0.0)
                            ));
                            ui.horizontal(|ui| {
                                if ui.selectable_label(listening, "A/B: listen to reference").clicked() {
                                    let _ = app.run("mastering.reference", json!({"listening": !listening}));
                                }
                                if ui.button("Level match").on_hover_text("audition gain only; never an export setting").clicked()
                                    && let Ok(m) = app.engine.execute("mastering.level_match", &json!({}))
                                {
                                    let _ = app.run("mastering.reference", json!({"gain_db": m["gain_db"]}));
                                }
                                if ui.button("Clear").clicked() {
                                    let _ = app.run("mastering.reference", json!({"source": null}));
                                }
                            });
                            // Known issue K1: be explicit rather than imply A/B plays audio.
                            ui.label(
                                egui::RichText::new("OPEN DEFECT: A/B stores the setting only — no reference audio reaches the speakers (known issue K1). Compare in another player at the level-matched gain.")
                                    .font(regular(crate::theme::size::SMALL))
                                    .color(t.warning),
                            );
                        }
                    });
                    card(ui, "METERS (OFFLINE, OVER THE PROGRAM)", &mut |ui| {
                        ui.horizontal(|ui| {
                            if ui.button("Measure").on_hover_text("Offline analysis of the program; nothing is processed or changed").clicked() {
                                app.ui.master_meters = app.engine.execute("mastering.meters", &json!({})).ok();
                            }
                            ui.label(egui::RichText::new("BS.1770-4 / EBU R128 / Tech 3342 — checked, not certified").font(regular(crate::theme::size::SMALL)).color(t.text_dim));
                        });
                        match &app.ui.master_meters {
                            None => {
                                ui.label(egui::RichText::new("Not measured yet.").color(t.text_dim));
                            }
                            Some(m) => {
                                egui::Grid::new("master_meter_grid").num_columns(3).spacing(vec2(12.0, 3.0)).show(ui, |ui| {
                                    for (k, label, unit) in [
                                        ("sample_peak_db", "Sample peak", "dBFS"),
                                        ("true_peak_db", "True peak", "dBTP"),
                                        ("integrated_lufs", "Integrated", "LUFS"),
                                        ("max_short_term_lufs", "Short-term max", "LUFS"),
                                        ("max_momentary_lufs", "Momentary max", "LUFS"),
                                        ("loudness_range_lu", "Loudness range", "LU"),
                                        ("correlation", "Correlation", "−1…1"),
                                        ("mono_peak_db", "Mono peak", "dBFS"),
                                    ] {
                                        ui.label(label);
                                        let v = m[k].as_f64();
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            ui.label(
                                                egui::RichText::new(v.map_or("—".to_string(), |v| format!("{v:.2}")))
                                                    .font(crate::theme::mono(crate::theme::size::NUMERIC))
                                                    .color(t.counter_text),
                                            );
                                        });
                                        ui.label(egui::RichText::new(unit).color(t.text_dim));
                                        ui.end_row();
                                    }
                                });
                            }
                        }
                    });
                    card(ui, "EXPORT", &mut |ui| {
                        if ui.button("Bounce Mix…").on_hover_text("File › Bounce Mix…").clicked() {
                            crate::menus::invoke_menu(app, "ui.bounce_dialog", "File > Bounce Mix...");
                        }
                        ui.label(egui::RichText::new("Format, bit depth (dither only on integer depths when chosen), rate (SRC stated), tail (auto/seconds), report JSON. Refuses missing or failed plugins unless acknowledged; never adds a limiter, normaliser or dither on its own.").font(regular(crate::theme::size::SMALL)).color(t.text_dim));
                    });
                });
            });
        });
    });
}
