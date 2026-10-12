//! The Edit window toolbar.

use crate::theme::{Tokens, bold, mono, regular};
use crate::widgets::icon_button;
use crate::{SoundApp, icons};
use egui::{Align2, Color32, CornerRadius, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::json;
use soundcraft_model::{EditMode, Tool};
use soundcraft_time::{TimeFormat, format_length, format_position};

pub const HEIGHT: f32 = 78.0;

/// RC09 keyboard fallback, stated where Nudge is used: the toolkit cannot tell keypad − from
/// the main-row − (only keypad + is distinguishable), so plain −/+ nudge whenever no text field,
/// plugin editor, dialog or shortcut capture has focus; every Nudge action can be remapped.
pub const NUDGE_EARLIER_HELP: &str = "Nudge earlier: Num− (keypad). Without a keypad, or when it is not recognised: − whenever no text field, plugin editor or dialog has focus. Remap in Setup › Keyboard Shortcuts (nudge.apply).";
pub const NUDGE_LATER_HELP: &str = "Nudge later: Num+ (keypad). Without a keypad, or when it is not recognised: + whenever no text field, plugin editor or dialog has focus. Remap in Setup › Keyboard Shortcuts (nudge.apply).";

fn group(ui: &mut Ui, w: f32, h: f32, add: impl FnOnce(&mut Ui, Rect)) {
    let t = Tokens::current();
    let (r, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    ui.painter().rect(r, CornerRadius::same(4), t.toolbar_group, Stroke::new(1.0, t.group_border), StrokeKind::Inside);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r.shrink(5.0)).layout(egui::Layout::left_to_right(egui::Align::Min)));
    add(&mut child, r);
}

/// Where the always-reachable toolbar controls were drawn last frame (RC09 evidence and tests).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolbarLayout {
    /// The window was narrower than the full row: Nudge and transport are pinned on the right.
    pub compact: bool,
    /// Natural width of the full row (measured on a full-row frame; 0 until then).
    pub full_width: f32,
    /// Natural width of the Nudge panel (measured with the full row).
    pub nudge_width: f32,
    pub transport: Rect,
    pub nudge: Rect,
    /// The area the toolbar had.
    pub visible: Rect,
}

impl Default for ToolbarLayout {
    fn default() -> Self {
        ToolbarLayout { compact: false, full_width: 0.0, nudge_width: 0.0, transport: Rect::NOTHING, nudge: Rect::NOTHING, visible: Rect::NOTHING }
    }
}

pub fn show(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, t.toolbar_bg);
    ui.add_space(4.0);
    let mut layout = app.toolbar_layout;
    layout.visible = full;
    // RC09: when the row does not fit (e.g. 1366×768 at 150 %), transport/record and the active
    // Nudge are pinned on the right where they are always visible; only the other groups
    // scroll. The row's natural width is learned from a frame where it fitted (or the first one).
    layout.compact = layout.full_width > 0.0 && full.width() < layout.full_width;
    if !layout.compact {
        let out = egui::ScrollArea::horizontal().id_salt("edit_toolbar").auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.spacing_mut().item_spacing.x = 8.0;
                edit_modes(app, ui);
                zoom_group(app, ui);
                tools(app, ui);
                audio_tools(app, ui);
                counters(app, ui);
                grid_nudge(app, ui);
                // Exactly its natural width, so the scrolling groups keep the rest of the row.
                let nw = if layout.nudge_width > 0.0 { layout.nudge_width } else { 400.0 };
                layout.nudge = ui
                    .allocate_ui_with_layout(vec2(nw, ui.available_height()), egui::Layout::left_to_right(egui::Align::Min), |ui| {
                        ui.set_max_width(nw);
                        nudge_panel(app, ui);
                    })
                    .response
                    .rect;
                layout.transport = ui.scope(|ui| transport(app, ui)).response.rect;
                tempo_meter(app, ui);
                output_meter(app, ui);
            });
        });
        layout.full_width = out.content_size.x + 16.0;
        layout.nudge_width = layout.nudge.width();
    } else {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.add_space(8.0);
            layout.transport = ui.scope(|ui| transport(app, ui)).response.rect;
            // Exactly its natural width, so the scrolling groups keep the rest of the row.
            let nw = if layout.nudge_width > 0.0 { layout.nudge_width } else { 400.0 };
            layout.nudge = ui
                .allocate_ui_with_layout(vec2(nw, ui.available_height()), egui::Layout::left_to_right(egui::Align::Min), |ui| {
                    ui.set_max_width(nw);
                    nudge_panel(app, ui);
                })
                .response
                .rect;
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                egui::ScrollArea::horizontal().id_salt("edit_toolbar_compact").auto_shrink([false, true]).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.spacing_mut().item_spacing.x = 8.0;
                        edit_modes(app, ui);
                        zoom_group(app, ui);
                        tools(app, ui);
                        audio_tools(app, ui);
                        counters(app, ui);
                        grid_nudge(app, ui);
                        tempo_meter(app, ui);
                        output_meter(app, ui);
                    });
                });
            });
        });
    }
    app.toolbar_layout = layout;
}

fn edit_modes(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let cur = app.engine.session().edit.edit_mode;
    group(ui, 112.0, 52.0, |ui, r| {
        let inner = r.shrink(6.0);
        let cw = inner.width() / 2.0;
        let ch = inner.height() / 2.0;
        let modes = [
            (EditMode::Shuffle, 0, 0, "SHUFFLE", "F1"),
            (EditMode::Spot, 1, 0, "SPOT", "F3"),
            (EditMode::Slip, 0, 1, "SLIP", "F2"),
            (EditMode::Grid, 1, 1, "GRID", "F4"),
        ];
        for (m, cx, cy, label, key) in modes {
            let cell = Rect::from_min_size(pos2(inner.min.x + cw * cx as f32, inner.min.y + ch * cy as f32), vec2(cw, ch)).shrink(1.0);
            let resp = ui.interact(cell, ui.id().with(("mode", label)), Sense::click());
            let on = cur == m || (m == EditMode::Grid && cur == EditMode::GridRelative);
            ui.painter().rect_filled(cell, 1.0, if on { t.mode_on } else { t.mode_off });
            let txt = if m == EditMode::Grid && cur == EditMode::GridRelative { "REL GRID" } else { label };
            ui.painter().text(cell.center(), Align2::CENTER_CENTER, txt, bold(10.5), if on { t.mode_on_text } else { t.mode_off_text });
            if resp.clicked() {
                let id = match m {
                    EditMode::Grid if cur == EditMode::Grid => "relative_grid",
                    EditMode::Shuffle => "shuffle",
                    EditMode::Spot => "spot",
                    EditMode::Slip => "slip",
                    _ => "grid",
                };
                let _ = app.run("edit.mode", json!({"mode": id}));
            }
            resp.on_hover_text(format!("{} mode ({key})", label.to_lowercase()));
        }
    });
}

fn zoom_group(app: &mut SoundApp, ui: &mut Ui) {
    group(ui, 128.0, 52.0, |ui, _| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 3.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                if icon_button(ui, vec2(28.0, 22.0), "minus", false, "Zoom out (Cmd+[)").clicked() {
                    let _ = app.run("view.zoom_out", json!({}));
                }
                if icon_button(ui, vec2(30.0, 22.0), "wave", false, "Waveform zoom in (Cmd+Alt+])").clicked() {
                    let _ = app.run("view.waveform_zoom", json!({"factor": 2.0}));
                }
                if icon_button(ui, vec2(28.0, 22.0), "plus", false, "Zoom in (Cmd+])").clicked() {
                    let _ = app.run("view.zoom_in", json!({}));
                }
                if icon_button(ui, vec2(28.0, 22.0), "zoom", false, "Fill window with session (Alt+A)").clicked() {
                    let w = ui.ctx().content_rect().width() - 420.0;
                    let _ = app.run("view.zoom_fit", json!({"width_px": w}));
                }
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for i in 1..=5 {
                    let r = crate::widgets::text_toggle(
                        ui,
                        vec2(22.0, 18.0),
                        &i.to_string(),
                        false,
                        Color32::GRAY,
                        "Zoom preset (click to recall, Cmd-click to store)",
                    );
                    if r.clicked() {
                        let store = ui.input(|x| x.modifiers.command);
                        let _ = app.run("view.zoom_preset", json!({"preset": i, "store": store}));
                    }
                }
            });
        });
    });
}

fn tools(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let cur = app.engine.session().edit.tool;
    group(ui, 268.0 + 38.0, 52.0, |ui, r| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 3.0);
            let row = ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (tool, icon) in [
                    (Tool::Zoom, "zoom"),
                    (Tool::Trim, "trim"),
                    (Tool::Selector, "selector"),
                    (Tool::Grabber, "grabber"),
                    (Tool::Scrubber, "scrubber"),
                    (Tool::Pencil, "pencil"),
                ] {
                    let smart_member = cur == Tool::Smart && matches!(tool, Tool::Trim | Tool::Selector | Tool::Grabber);
                    let tip = format!("{}{}", tool.label(), tool.fkey().map(|k| format!(" ({k})")).unwrap_or_default());
                    if icon_button(ui, vec2(36.0, 26.0), icon, cur == tool || smart_member, &tip).clicked() {
                        let _ = app.run("edit.tool", json!({"tool": tool.id()}));
                    }
                }
            });
            // Smart tool bracket above Trim/Selector/Grabber.
            let rr = row.response.rect;
            let bx0 = rr.min.x + 38.0;
            let bx1 = bx0 + 38.0 * 3.0 - 2.0;
            let bracket = Rect::from_min_max(pos2(bx0 - 2.0, r.min.y + 1.0), pos2(bx1 + 2.0, r.min.y + 5.0));
            let resp = ui.interact(bracket.expand2(vec2(0.0, 2.0)), ui.id().with("smart"), Sense::click());
            ui.painter().rect_filled(bracket, 1.0, if cur == Tool::Smart { t.accent } else { t.tool_bracket_off });
            if resp.on_hover_text("Smart Tool (F6+F7 / F7+F8)").clicked() {
                let _ = app.run("edit.tool", json!({"tool": "smart"}));
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let e = app.engine.session().edit.clone();
                let toggles: [(&str, &str, bool, &str); 5] = [
                    ("options.tab_to_transient", "selector", e.tab_to_transient, "Tab to Transient"),
                    ("options.link_timeline_edit", "link", e.link_timeline_edit, "Link Timeline and Edit Selection"),
                    ("options.link_track_edit", "link", e.link_track_edit, "Link Track and Edit Selection"),
                    ("options.mirrored_midi", "note", e.mirrored_midi, "Mirrored MIDI Editing"),
                    ("options.automation_follows_edit", "wave", e.automation_follows_edit, "Automation Follows Edit"),
                ];
                for (id, icon, on, tip) in toggles {
                    if icon_button(ui, vec2(36.0, 18.0), icon, on, tip).clicked() {
                        let _ = app.run(id, json!({}));
                    }
                }
                let focus = e.keyboard_focus == "commands";
                if crate::widgets::text_toggle(ui, vec2(36.0, 18.0), "a-z", focus, t.accent, "Commands Keyboard Focus (single-key editing)").clicked()
                {
                    let _ = app.run("options.keyboard_focus", json!({}));
                }
            });
        });
    });
}

/// The eight audio-editing tools (EDIT_TOOLBAR_SPEC T01) in reference order, with the Link
/// Arrow/Range toggle, the alternate-tool assignment and the Paint shape (T06, T10). Labelled
/// text buttons: no icon-only dependency.
fn audio_tools(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let (cur, link, alt, shape, cycles) = {
        let e = &app.engine.session().edit;
        (e.tool, e.tool_link, e.alt_tool, e.paint_shape.clone(), e.paint_cycles)
    };
    group(ui, 8.0 * 44.0 + 12.0, 52.0, |ui, _| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 3.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for tool in Tool::AUDIO {
                    let tip = format!("{} ({}) — {}", tool.label(), tool.number_key().map(|k| k.to_string()).unwrap_or_default(), tool.description());
                    let on = cur == tool;
                    if crate::widgets::text_toggle(ui, vec2(42.0, 22.0), tool.label(), on, t.accent, &tip).clicked() {
                        let _ = app.run("edit.tool", json!({"tool": tool.id()}));
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if icon_button(ui, vec2(42.0, 18.0), "link", link, "Link Arrow/Range: the upper half of a tall track selects a range").clicked() {
                    let _ = app.run("edit.tool_link", json!({}));
                }
                ui.label(egui::RichText::new("Alt:").font(regular(10.0)).color(t.text_dim));
                let alt_label = alt.map_or("None", Tool::label);
                egui::ComboBox::from_id_salt("alt_tool")
                    .selected_text(alt_label)
                    .width(64.0)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(alt.is_none(), "None").clicked() {
                            let _ = app.run("edit.alt_tool", json!({"tool": null}));
                        }
                        for tool in Tool::AUDIO.into_iter().filter(|x| *x != Tool::Arrow) {
                            if ui.selectable_label(alt == Some(tool), tool.label()).clicked() {
                                let _ = app.run("edit.alt_tool", json!({"tool": tool.id()}));
                            }
                        }
                    })
                    .response
                    .on_hover_text("Alternate tool while Ctrl (Cmd on macOS) is held with Arrow active");
                ui.label(egui::RichText::new("Paint:").font(regular(10.0)).color(t.text_dim));
                egui::ComboBox::from_id_salt("paint_shape").selected_text(&shape).width(78.0).show_ui(ui, |ui| {
                    for sh in ["freehand", "line", "parabola", "square", "triangle", "saw", "sine", "transform"] {
                        if ui.selectable_label(shape == sh, sh).clicked() {
                            let _ = app.run("edit.paint_shape", json!({"shape": sh}));
                        }
                    }
                });
                if matches!(shape.as_str(), "square" | "triangle" | "saw" | "sine") {
                    let mut c = cycles;
                    let r = ui.add(egui::DragValue::new(&mut c).speed(0.1).range(0.1..=1000.0).suffix(" cyc"));
                    if r.changed() && (c - cycles).abs() > 1e-9 {
                        let _ = app.run("edit.paint_shape", json!({"shape": shape, "cycles": c}));
                    }
                }
            });
        });
    });
}

fn fmt_pos(app: &SoundApp, at: i64, f: TimeFormat) -> String {
    let s = app.engine.session();
    format_position(at, f, s.sample_rate, &s.tempo, s.frame_rate, s.timecode_start)
}

fn counters(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let (main_fmt, sub_fmt, sel) = {
        let s = app.engine.session();
        (s.edit.main_counter, s.edit.sub_counter.unwrap_or(TimeFormat::Samples), s.edit.selection)
    };
    let pos = app.position();
    let (r, _) = ui.allocate_exact_size(vec2(318.0, 64.0), Sense::hover());
    ui.painter().rect(r, CornerRadius::same(4), t.counter_bg, Stroke::new(1.0, t.counter_border), StrokeKind::Inside);
    let main_r = Rect::from_min_max(r.min, pos2(r.min.x + 180.0, r.min.y + 44.0));
    let txt = fmt_pos(app, pos, main_fmt);
    ui.painter().text(pos2(main_r.max.x - 18.0, main_r.center().y), Align2::RIGHT_CENTER, txt, mono(26.0), t.counter_text);
    let resp = ui.interact(main_r, ui.id().with("main_counter"), Sense::click());
    icons::draw(ui.painter(), Rect::from_center_size(pos2(main_r.max.x - 9.0, main_r.center().y), vec2(10.0, 10.0)), "triangle_down", t.text_dim);
    egui::Popup::menu(&resp).show(|ui| {
        for f in TimeFormat::ALL {
            if ui.selectable_label(main_fmt == f, f.label()).clicked() {
                let _ = app.run("view.main_counter", json!({"format": f.id()}));
            }
        }
    });
    // Start / End / Length.
    let s = app.engine.session();
    let rows = [
        ("Start", fmt_pos(app, sel.start, main_fmt)),
        ("End", fmt_pos(app, sel.end, main_fmt)),
        ("Length", format_length(sel.start, sel.len(), main_fmt, s.sample_rate, &s.tempo, s.frame_rate)),
    ];
    for (i, (label, val)) in rows.iter().enumerate() {
        let y = r.min.y + 10.0 + i as f32 * 13.0;
        ui.painter().text(pos2(r.min.x + 214.0, y), Align2::RIGHT_CENTER, *label, regular(11.0), t.counter_label);
        ui.painter().text(pos2(r.max.x - 8.0, y), Align2::RIGHT_CENTER, val, mono(11.0), t.counter_text);
    }
    // Sub counter row.
    let y = r.max.y - 11.0;
    ui.painter().line_segment([pos2(r.min.x + 6.0, r.max.y - 21.0), pos2(r.max.x - 6.0, r.max.y - 21.0)], Stroke::new(1.0, t.counter_rule));
    ui.painter().text(pos2(r.min.x + 10.0, y), Align2::LEFT_CENTER, "Sub", regular(11.0), t.counter_label);
    ui.painter().text(pos2(r.min.x + 150.0, y), Align2::RIGHT_CENTER, fmt_pos(app, pos, sub_fmt), mono(11.0), t.counter_text);
    let st = if app.engine.is_dirty() { "modified" } else { "saved" };
    let dev = app.player.as_ref().map_or("no audio engine", |p| if p.silent { "silent clock" } else { "audio on" });
    ui.painter().text(pos2(r.max.x - 8.0, y), Align2::RIGHT_CENTER, format!("{st} · {dev}"), regular(10.0), t.text_dim);
}

fn grid_nudge(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let (grid, nudge, lines) = {
        let e = &app.engine.session().edit;
        (e.grid, e.nudge, e.grid_lines)
    };
    let (r, _) = ui.allocate_exact_size(vec2(170.0, 64.0), Sense::hover());
    ui.painter().rect(r, CornerRadius::same(4), t.counter_bg, Stroke::new(1.0, t.counter_border), StrokeKind::Inside);
    let gl = Rect::from_min_size(pos2(r.min.x + 6.0, r.min.y + 8.0), vec2(48.0, 16.0));
    ui.painter().rect_filled(gl, 1.0, if lines { t.mode_on } else { t.counter_off });
    ui.painter().text(gl.center(), Align2::CENTER_CENTER, "Grid", bold(11.0), if lines { t.mode_on_text } else { t.mode_off_text });
    if ui.interact(gl, ui.id().with("grid_lines"), Sense::click()).on_hover_text("Show grid lines").clicked() {
        let _ = app.run("view.grid_lines", json!({}));
    }
    let gr = Rect::from_min_max(pos2(r.min.x + 60.0, r.min.y + 6.0), pos2(r.max.x - 4.0, r.min.y + 26.0));
    ui.painter().text(pos2(gr.max.x - 14.0, gr.center().y), Align2::RIGHT_CENTER, grid.label(), mono(11.0), t.counter_text);
    let gresp = ui.interact(gr, ui.id().with("grid_value"), Sense::click());
    value_menu(app, &gresp, "view.grid");
    // The nudge value lives in the dedicated NUDGE group (separate from Grid); the legacy
    // grid-style nudge value (`view.nudge` / `edit.nudge`) is no longer shown here.
    let _ = nudge;
    icons::draw(ui.painter(), Rect::from_center_size(pos2(gr.max.x - 6.0, gr.center().y), vec2(10.0, 10.0)), "triangle_down", t.text_dim);
}

/// Steps the dedicated Nudge value through the SML presets (Ctrl+Alt+Num±).
pub fn cycle_nudge_value(app: &mut SoundApp, dir: i64) {
    let p = app.engine.session().edit.nudge_panel.clone();
    let presets = soundcraft_model::NUDGE_PRESETS;
    let cur = presets.iter().position(|(v, u)| *u == p.unit && (*v - p.value).abs() < 1e-9);
    let n = presets.len() as i64;
    let next = match cur {
        Some(i) => (i as i64 + dir).rem_euclid(n),
        None => 0,
    };
    if let Some((v, u)) = presets.get(next as usize) {
        let _ = app.run("nudge.set", json!({"value": v, "unit": u.label()}));
    }
}

/// The always-visible dedicated NUDGE group: value, unit, target, arrows and the exact
/// displacement readout (e.g. "10 ms = 480 smp"). Separate from the Grid value.
fn nudge_panel(app: &mut SoundApp, ui: &mut Ui) {
    use soundcraft_model::{NudgeTarget, NudgeUnit};
    let p = app.engine.session().edit.nudge_panel.clone();
    let sr = app.engine.session().sample_rate.as_f64();
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("NUDGE").strong().small());
            if ui.small_button("◀").on_hover_text(NUDGE_EARLIER_HELP).clicked()
                && let Err(e) = app.run("nudge.apply", json!({"direction": -1}))
            {
                app.ui.status = e;
            }
            if ui.small_button("▶").on_hover_text(NUDGE_LATER_HELP).clicked()
                && let Err(e) = app.run("nudge.apply", json!({"direction": 1}))
            {
                app.ui.status = e;
            }
            let mut v = p.value;
            if ui.add(egui::DragValue::new(&mut v).range(0.001..=100_000.0).speed(0.1).max_decimals(3)).changed() {
                let _ = app.run("nudge.set", json!({"value": v}));
            }
            egui::ComboBox::from_id_salt("nudge_unit").selected_text(p.unit.label()).width(64.0).show_ui(ui, |ui| {
                for u in [NudgeUnit::Samples, NudgeUnit::Milliseconds, NudgeUnit::Seconds, NudgeUnit::Ticks, NudgeUnit::Frames] {
                    if ui.selectable_label(p.unit == u, u.label()).clicked() {
                        let _ = app.run("nudge.set", json!({"unit": u.label()}));
                    }
                }
            });
        });
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("nudge_target").selected_text(p.target.label()).width(130.0).show_ui(ui, |ui| {
                for tg in NudgeTarget::ALL {
                    if ui.selectable_label(p.target == tg, tg.label()).clicked() {
                        let _ = app.run("nudge.set", json!({"target": tg.id()}));
                    }
                }
            });
            for (v, u, label) in [(1.0, "samples", "1 smp"), (1.0, "ms", "1 ms"), (10.0, "ms", "10 ms")] {
                if ui.small_button(label).clicked() {
                    let _ = app.run("nudge.set", json!({"value": v, "unit": u}));
                }
            }
            let readout = match p.unit {
                NudgeUnit::Ticks => format!("{:.0} ticks (tempo map)", p.value),
                _ => {
                    let smp = soundcraft_engine::nudge_step_samples(app.engine.session()).unwrap_or(0.0);
                    format!("= {smp:.2} smp @ {:.1} kHz", sr / 1000.0)
                }
            };
            ui.label(egui::RichText::new(readout).small());
        });
    });
}

fn value_menu(app: &mut SoundApp, resp: &egui::Response, cmd: &str) {
    egui::Popup::menu(resp).show(|ui| {
        for v in ["1 bar", "1/2", "1/4", "1/8", "1/16", "1/32", "1/64", "1/8t", "1/16t", "1/4."] {
            if ui.button(v).clicked() {
                let _ = app.run(cmd, json!({"value": v}));
            }
        }
        ui.separator();
        for (label, v) in [
            ("1 msec", json!({"seconds": 0.001})),
            ("10 msec", json!({"seconds": 0.01})),
            ("100 msec", json!({"seconds": 0.1})),
            ("1 second", json!({"seconds": 1.0})),
            ("1 frame", json!({"frames": 1})),
            ("100 samples", json!({"samples": 100})),
        ] {
            if ui.button(label).clicked() {
                let _ = app.run(cmd, json!({"value": v}));
            }
        }
    });
}

fn transport(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let playing = app.is_playing();
    let recording = app.engine.transport.recording;
    let looped = app.engine.session().edit.loop_playback;
    group(ui, 176.0, 64.0, |ui, _| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = vec2(3.0, 4.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                if icon_button(ui, vec2(38.0, 28.0), "loop", looped, "Loop Playback (Cmd+Shift+L)").clicked() {
                    let _ = app.run("options.loop_playback", json!({}));
                }
                let (r, resp) = ui.allocate_exact_size(vec2(38.0, 28.0), Sense::click());
                ui.painter().rect(
                    r,
                    CornerRadius::same(3),
                    if resp.hovered() { t.button_hi } else { t.button },
                    Stroke::new(1.0, t.button_border),
                    StrokeKind::Inside,
                );
                icons::draw(ui.painter(), r.shrink(5.0), "stop", if playing { t.text } else { t.accent });
                if resp.on_hover_text("Stop (Space)").clicked() {
                    let _ = app.run("transport.stop", json!({}));
                }
                let (r, resp) = ui.allocate_exact_size(vec2(38.0, 28.0), Sense::click());
                ui.painter().rect(
                    r,
                    CornerRadius::same(3),
                    if playing {
                        Color32::from_rgb(40, 90, 50)
                    } else if resp.hovered() {
                        t.button_hi
                    } else {
                        t.button
                    },
                    Stroke::new(1.0, t.button_border),
                    StrokeKind::Inside,
                );
                icons::draw(ui.painter(), r.shrink(5.0), "play", if playing { Color32::from_rgb(120, 255, 140) } else { t.counter_text });
                if resp.on_hover_text("Play (Space)").clicked() {
                    let _ = app.run("transport.play", json!({}));
                }
                let (r, resp) = ui.allocate_exact_size(vec2(38.0, 28.0), Sense::click());
                ui.painter().rect(
                    r,
                    CornerRadius::same(3),
                    if recording {
                        Color32::from_rgb(110, 30, 30)
                    } else if resp.hovered() {
                        t.button_hi
                    } else {
                        t.button
                    },
                    Stroke::new(1.0, t.button_border),
                    StrokeKind::Inside,
                );
                icons::draw(ui.painter(), r.shrink(5.0), "record", t.rec);
                if resp.on_hover_text("Record (Cmd+Space)").clicked() {
                    let _ = app.run("transport.record", json!({}));
                }
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                for (icon, cmd, tip) in [
                    ("rtz", "transport.rtz", "Return to Zero (Home)"),
                    ("rewind", "transport.rewind", "Rewind"),
                    ("ffwd", "transport.fast_forward", "Fast Forward"),
                    ("end", "transport.go_to_end", "Go to End (End)"),
                ] {
                    if icon_button(ui, vec2(38.0, 22.0), icon, false, tip).clicked() {
                        let _ = app.run(cmd, json!({}));
                    }
                }
            });
        });
    });
}

fn tempo_meter(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let (bpm, meter, countoff, click, bars) = {
        let s = app.engine.session();
        let tick = s.tempo.samples_to_ticks(app.position(), s.sample_rate);
        let m = s.tempo.meter_at_tick(tick);
        (s.tempo.tempo_at_tick(tick), format!("{}/{}", m.numerator, m.denominator), s.edit.countoff, s.edit.click, s.edit.countoff_bars)
    };
    let (r, _) = ui.allocate_exact_size(vec2(150.0, 64.0), Sense::hover());
    ui.painter().rect(r, CornerRadius::same(4), t.counter_bg, Stroke::new(1.0, t.counter_border), StrokeKind::Inside);
    let co = Rect::from_min_size(pos2(r.min.x + 6.0, r.min.y + 6.0), vec2(64.0, 15.0));
    ui.painter().rect_filled(co, 1.0, if countoff { t.mode_on } else { t.counter_off });
    ui.painter().text(co.center(), Align2::CENTER_CENTER, "Count Off", bold(10.0), if countoff { t.mode_on_text } else { t.mode_off_text });
    if ui.interact(co, ui.id().with("countoff"), Sense::click()).clicked() {
        let _ = app.run("options.countoff", json!({}));
    }
    ui.painter().text(pos2(r.max.x - 8.0, co.center().y), Align2::RIGHT_CENTER, format!("{bars} bars"), mono(11.0), t.counter_text);
    ui.painter().text(pos2(r.min.x + 70.0, r.min.y + 32.0), Align2::RIGHT_CENTER, "Meter", regular(11.0), t.counter_label);
    ui.painter().text(pos2(r.max.x - 8.0, r.min.y + 32.0), Align2::RIGHT_CENTER, meter, mono(11.0), t.counter_text);
    ui.painter().text(pos2(r.min.x + 70.0, r.min.y + 48.0), Align2::RIGHT_CENTER, "Tempo", regular(11.0), t.counter_label);
    icons::draw(ui.painter(), Rect::from_center_size(pos2(r.min.x + 82.0, r.min.y + 48.0), vec2(13.0, 13.0)), "note", t.counter_text);
    ui.painter().text(pos2(r.max.x - 8.0, r.min.y + 48.0), Align2::RIGHT_CENTER, format!("{bpm:.4}"), mono(11.0), t.counter_text);
    let clk = Rect::from_min_size(pos2(r.min.x + 6.0, r.min.y + 40.0), vec2(18.0, 18.0));
    icons::draw(ui.painter(), clk, "click", if click { t.counter_text } else { t.text_dim });
    if ui.interact(clk, ui.id().with("click"), Sense::click()).on_hover_text("Click").clicked() {
        let _ = app.run("options.click", json!({}));
    }
}

fn output_meter(app: &mut SoundApp, ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(26.0, 64.0), Sense::hover());
    let m = app.main_meter;
    let w = (r.width() - 6.0) / 2.0;
    for c in 0..2 {
        let rr = Rect::from_min_size(pos2(r.min.x + 2.0 + c as f32 * (w + 2.0), r.min.y), vec2(w, r.height()));
        crate::widgets::meter(ui, rr, m.level[c], m.hold[c], m.clip);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Renders the Edit toolbar alone in a `w`×`h` (logical points) screen, twice (the first
    /// frame learns the row's natural width), and returns where things landed.
    fn layout_at(w: f32, h: f32, light: bool) -> ToolbarLayout {
        let mut app = SoundApp::new(soundcraft_engine::Engine::default(), None, crate::Services::default());
        let ctx = egui::Context::default();
        crate::fonts::install(&ctx);
        ctx.set_visuals(if light { egui::Visuals::light() } else { egui::Visuals::dark() });
        let raw = || egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(w, h))), ..Default::default() };
        for _ in 0..3 {
            let mut out = ctx.run_ui(raw(), |ui| {
                egui::Panel::top("edit_toolbar").exact_size(HEIGHT).frame(egui::Frame::NONE).show(ui, |ui| show(&mut app, ui));
            });
            out.textures_delta.clear();
        }
        app.toolbar_layout
    }

    /// RC09: at 1366×768 at 150 % (911×512 logical points) the transport/record group and the
    /// active Nudge panel are fully on screen without scrolling; at 1920×1080 and 2560×1440 the
    /// normal full row is used and they are on screen too. Light and dark themes alike.
    #[test]
    fn transport_and_nudge_are_reachable_on_small_screens() {
        for (w, h, compact) in [(1366.0 / 1.5, 768.0 / 1.5, true), (1280.0, 720.0, true), (1920.0, 1080.0, true), (2560.0, 1440.0, false)] {
            for light in [false, true] {
                let l = layout_at(w, h, light);
                println!("{w}x{h} light {light}: {l:?}");
                assert_eq!(l.compact, compact, "{w}x{h}: compact layout {l:?}");
                for (name, r) in [("transport", l.transport), ("nudge", l.nudge)] {
                    assert!(r.width() > 20.0 && r.height() > 20.0, "{w}x{h} {name} drawn: {r:?}");
                    assert!(
                        r.min.x >= 0.0 && r.max.x <= w + 0.5 && r.max.y <= HEIGHT + 0.5,
                        "{w}x{h} {name} on screen: {r:?} (visible {:?})",
                        l.visible
                    );
                }
                assert!(!l.transport.intersects(l.nudge.shrink(1.0)), "{w}x{h}: transport and Nudge do not overlap");
                if compact {
                    assert!(l.nudge.min.x >= 250.0, "{w}x{h}: the other groups keep a scrollable strip: {l:?}");
                }
            }
        }
    }
}
