//! HF11: the Mixer in its own native window (Window › Detach Mixer, `window.mix_detach`), for
//! a second monitor. It is an egui *immediate* viewport: drawn in the same frame, on the same
//! thread, from the same `SoundApp` — one engine, one session, one player — so every fader,
//! mute, send or insert change goes through the one command path and is applied once. On a
//! backend without multiple native windows (the headless test harness) it is drawn as a window
//! inside the main one instead.
//!
//! Its geometry (position, size, maximized, the size and scale of the display it was on) is a
//! UI preference (`ui.json`), never session state. A saved position that is not on any display
//! the app can see reopens on the main display. Closing the window docks the Mixer back into
//! the main window; playback is not affected.

use crate::{MainWindow, SoundApp};
use serde_json::{Value, json};

/// Stable id of the detached Mixer viewport.
pub const VIEWPORT: &str = "sml_detached_mixer";

/// The detached Mixer's state, saved with the UI preferences.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct MixerWindowPrefs {
    pub detached: bool,
    /// Outer position and inner size in points, as last reported by the window system.
    pub pos: Option<[f32; 2]>,
    pub size: Option<[f32; 2]>,
    pub maximized: bool,
    /// Size (points) and scale of the display the window was last on: a best-effort identity
    /// (egui reports no display names).
    pub monitor_size: Option<[f32; 2]>,
    pub scale: Option<f32>,
}

fn viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of(VIEWPORT)
}

/// The position to reopen at, or `None` (let the system place it, normally on the main
/// display) when the saved one cannot be on a display: further from the main display than the
/// widest arrangement of displays this size allows (three side by side, either side).
pub fn usable_position(saved: Option<[f32; 2]>, main_monitor: Option<egui::Vec2>, size: Option<[f32; 2]>) -> Option<[f32; 2]> {
    let [x, y] = saved?;
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    let m = main_monitor.unwrap_or(egui::vec2(1920.0, 1080.0));
    let [w, h] = size.unwrap_or([800.0, 600.0]);
    // Some part of the title bar must be within reach of a display.
    let ok_x = x + w > -3.0 * m.x + 40.0 && x < 4.0 * m.x - 40.0;
    let ok_y = y > -2.0 * m.y && y < 3.0 * m.y - 40.0 && y + h > -2.0 * m.y;
    (ok_x && ok_y).then_some([x, y])
}

/// Detach (`true`) or dock (`false`). Detaching from the Mix layout shows the Edit layout in the
/// main window, so Edit and Mix are side by side on two displays.
pub fn set_detached(app: &mut SoundApp, on: bool) -> Value {
    if on != app.ui.mixer_window.detached {
        app.ui.mixer_window.detached = on;
        if on && app.ui.window == MainWindow::Mix {
            app.ui.window = MainWindow::Edit;
        }
        app.ui.status = if on {
            "Mixer detached: it is in its own window (move it to another display). Close it or use Window › Dock Mixer to bring it back.".into()
        } else {
            "Mixer docked: it is back in the main window (Window › Mix).".into()
        };
    }
    status(app)
}

/// `window.mix_window_status`: whether the Mixer is detached and its saved geometry.
pub fn status(app: &SoundApp) -> Value {
    let w = &app.ui.mixer_window;
    json!({
        "detached": w.detached,
        "position": w.pos,
        "size": w.size,
        "maximized": w.maximized,
        "monitor_size": w.monitor_size,
        "scale": w.scale,
    })
}

/// Draws the detached Mixer (each frame while detached).
pub fn show(app: &mut SoundApp, ctx: &egui::Context) {
    if !app.ui.mixer_window.detached {
        return;
    }
    let main_monitor = ctx.input(|i| i.viewport().monitor_size);
    let saved = app.ui.mixer_window.clone();
    let mut b = egui::ViewportBuilder::default()
        .with_title("SML Studio — Mixer")
        .with_app_id("ai.storyteller.soundcraft")
        .with_min_inner_size([480.0, 360.0])
        .with_inner_size(saved.size.unwrap_or([1200.0, 760.0]));
    match usable_position(saved.pos, main_monitor, saved.size) {
        Some(p) => b = b.with_position(p),
        None if saved.pos.is_some() => {
            app.ui.status = "The Mixer window's saved position is not on a connected display; it opened on the main display.".into();
            app.ui.mixer_window.pos = None;
        }
        None => {}
    }
    if saved.maximized {
        b = b.with_maximized(true);
    }
    ctx.show_viewport_immediate(viewport_id(), b, |ui, class| {
        let (close, outer, inner, maximized, monitor, scale) = ui.ctx().input(|i| {
            let v = i.viewport();
            (v.close_requested(), v.outer_rect, v.inner_rect, v.maximized, v.monitor_size, v.native_pixels_per_point)
        });
        if class != egui::ViewportClass::EmbeddedWindow {
            // Remember the native window's geometry as the window system reports it.
            let w = &mut app.ui.mixer_window;
            if let Some(o) = outer {
                w.pos = Some([o.min.x, o.min.y]);
            }
            if let Some(r) = inner {
                w.size = Some([r.width(), r.height()]);
            }
            w.maximized = maximized.unwrap_or(false);
            if let Some(m) = monitor {
                w.monitor_size = Some([m.x, m.y]);
            }
            w.scale = scale.or(w.scale);
            // Its own keyboard input: the same shortcuts as the main window (Space plays).
            crate::shortcuts::handle(app, &ui.ctx().clone());
        }
        if close {
            // Closing docks the Mixer; the transport keeps running.
            set_detached(app, false);
            return;
        }
        egui::Panel::top("detached_mixer_transport").exact_size(30.0).show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                crate::control_bar::transport(app, ui);
                if ui.button("Dock Mixer").on_hover_text("Bring the Mixer back into the main window (Window › Dock Mixer)").clicked() {
                    set_detached(app, false);
                }
            });
        });
        egui::CentralPanel::default().show(ui, |ui| crate::mix_window::content(app, ui));
    });
}

/// The main window's Mix layout while the Mixer is detached.
pub fn placeholder(app: &mut SoundApp, ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.3);
        ui.heading("The Mixer is in its own window");
        ui.label("Move it to another display; it shows the same session, live.");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Show Mixer window").clicked() {
                ui.ctx().send_viewport_cmd_to(viewport_id(), egui::ViewportCommand::Focus);
            }
            if ui.button("Dock Mixer").clicked() {
                set_detached(app, false);
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_positions_on_a_missing_display_fall_back_to_the_main_one() {
        let m = Some(egui::vec2(1920.0, 1080.0));
        // Second and third display to the right, and one to the left: kept.
        for p in [[1940.0, 20.0], [3900.0, 0.0], [-1900.0, 100.0]] {
            assert_eq!(usable_position(Some(p), m, Some([1200.0, 800.0])), Some(p), "{p:?}");
        }
        // Far off every plausible display, or not a number: the system places it.
        for p in [[20000.0, 0.0], [0.0, 9000.0], [-9000.0, 0.0], [f32::NAN, 0.0]] {
            assert_eq!(usable_position(Some(p), m, Some([1200.0, 800.0])), None, "{p:?}");
        }
        assert_eq!(usable_position(None, m, None), None);
    }
}
