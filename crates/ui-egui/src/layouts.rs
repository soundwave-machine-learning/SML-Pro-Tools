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
    egui::Panel::top("master_banner").exact_size(40.0).frame(egui::Frame::NONE.fill(t.toolbar_bg).inner_margin(egui::Margin::same(8))).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("MASTER").font(bold(13.0)).color(t.header_text));
            if ui.button("Back to Mix").clicked() {
                let _ = app.run("window.mix", json!({}));
            }
            ui.label(egui::RichText::new("chain = Master track inserts (applied once) · reference and monitor controls are excluded from exports · meters: BS.1770-4 / EBU R128 / Tech 3342 (E07 checked, not certified)").font(regular(10.5)).color(t.text_dim));
        });
    });
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.window_bg).inner_margin(egui::Margin::same(12))).show(ui, |ui| {
        let main = app.engine.session().tracks.iter().find(|x| x.kind == TrackKind::Master).map(|x| x.id);
        let snap = app.player.as_ref().filter(|_| app.engine.transport.playing).map(|p| p.meters());
        ui.horizontal_top(|ui| {
            match main {
                Some(id) => crate::mix_window::strip(app, ui, id, snap.as_ref()),
                None => { ui.label("This session has no Master track."); }
            }
            ui.add_space(24.0);
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Program and reference").font(bold(12.0)));
                let st = app.engine.execute("mastering.status", &json!({})).unwrap_or_default();
                ui.label(format!("program: {}", st["program"].as_u64().map_or("session mix".to_string(), |s| format!("source {s}"))));
                let reference = st["reference"].clone();
                if reference.is_null() {
                    ui.label("reference: none (Clip List › right-click an imported file › Use as reference)");
                } else {
                    let listening = reference["listening"].as_bool().unwrap_or(false);
                    ui.horizontal(|ui| {
                        ui.label(format!("reference: source {} at {:+.1} dB", reference["source"].as_u64().unwrap_or(0), reference["gain_db"].as_f64().unwrap_or(0.0)));
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
                }
                ui.separator();
                ui.label(egui::RichText::new("Meters (offline over the program)").font(bold(12.0)));
                if ui.button("Measure").clicked() {
                    app.ui.master_meters = app.engine.execute("mastering.meters", &json!({})).ok();
                }
                if let Some(m) = &app.ui.master_meters {
                    for (k, unit) in [("sample_peak_db", "dBFS"), ("true_peak_db", "dBTP"), ("max_momentary_lufs", "LUFS (M max)"), ("max_short_term_lufs", "LUFS (S max)"), ("integrated_lufs", "LUFS (I)"), ("loudness_range_lu", "LU (LRA)"), ("correlation", "(−1..1)"), ("mono_peak_db", "dBFS mono")] {
                        ui.label(format!("{}: {:.2} {unit}", k.replace('_', " "), m[k].as_f64().unwrap_or(f64::NAN)));
                    }
                }
                ui.separator();
                ui.label(egui::RichText::new("Export").font(bold(12.0)));
                ui.label(egui::RichText::new("File › Bounce Mix…: format, bit depth (dither only on integer depths when chosen), rate (SRC stated), tail (auto/seconds), report JSON; refuses missing or failed plugins unless acknowledged; never adds a limiter, normaliser or dither on its own.").font(regular(10.5)).color(t.text_dim));
            });
        });
    });
}
