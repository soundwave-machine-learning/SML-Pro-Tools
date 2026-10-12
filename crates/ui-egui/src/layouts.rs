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

/// The layout switcher shown at the right of the menu bar.
pub fn switcher(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    for (w, label, cmd) in [
        (MainWindow::Record, "Record", "window.record"),
        (MainWindow::Edit, "Edit", "window.edit"),
        (MainWindow::Mix, "Mix", "window.mix"),
        (MainWindow::Master, "Master", "window.master"),
    ] {
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

/// Master layout: a navigation shell (main strip + meters). Mastering DSP, metering standards
/// and export acceptance belong to Sprint 4 and are labelled incomplete until then (R34).
pub fn master(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    egui::Panel::top("master_banner").exact_size(54.0).frame(egui::Frame::NONE.fill(t.toolbar_bg).inner_margin(egui::Margin::same(8))).show(ui, |ui| {
        ui.label(egui::RichText::new("MASTER — navigation shell").font(bold(13.0)).color(t.header_text));
        ui.label(
            egui::RichText::new("Incomplete: mastering processing, standard meters, dither/rate/depth export and the reference/cue exclusions arrive in Sprint 4 (R34). What is here is the real main strip and output meters of this session; nothing in this layout processes audio differently.")
                .font(regular(10.5))
                .color(t.text_dim),
        );
    });
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.window_bg).inner_margin(egui::Margin::same(12))).show(ui, |ui| {
        let main = app.engine.session().tracks.iter().find(|x| x.kind == TrackKind::Master).map(|x| x.id);
        let snap = app.player.as_ref().filter(|_| app.engine.transport.playing).map(|p| p.meters());
        ui.horizontal_top(|ui| {
            match main {
                Some(id) => crate::mix_window::strip(app, ui, id, snap.as_ref()),
                None => {
                    ui.label("This session has no Master track.");
                }
            }
            ui.add_space(24.0);
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Output").font(bold(12.0)));
                let m = app.main_meter;
                ui.label(format!(
                    "Peak L {:.1} dBFS · R {:.1} dBFS{}",
                    20.0 * m.level[0].max(1e-6).log10(),
                    20.0 * m.level[1].max(1e-6).log10(),
                    if m.clip { " · CLIP" } else { "" }
                ));
                ui.label(
                    egui::RichText::new("Monitor volume/dim/mono act on the speaker path only (Window › Recording); they never reach a bounce.")
                        .font(regular(10.5))
                        .color(t.text_dim),
                );
                ui.separator();
                ui.label(egui::RichText::new("Navigation").font(bold(12.0)));
                if ui.button("Bounce Mix… (Sprint 4 acceptance pending)").clicked() {
                    let _ = app.run("ui.bounce_dialog", json!({}));
                }
                if ui.button("Session Audio Health").clicked() {
                    let _ = app.run("window.audio_health", json!({"value": true}));
                }
                if ui.button("Back to Mix").clicked() {
                    let _ = app.run("window.mix", json!({}));
                }
            });
        });
    });
}
