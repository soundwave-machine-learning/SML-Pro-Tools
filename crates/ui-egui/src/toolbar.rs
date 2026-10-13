//! The Edit window toolbar.

use crate::SoundApp;
use crate::theme::{Tokens, bold, mono, regular};
use crate::widgets::icon_button;
use egui::{Align2, Color32, CornerRadius, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::json;
use soundcraft_model::{EditMode, Tool};
use soundcraft_time::{TimeFormat, format_length, format_position};

/// HF08: the Edit window's controls are in two fixed rows that never scroll. Above the tracks,
/// the editing tools (edit modes, tools, audio tools, zoom); below them, the bottom bar
/// (transport, counters, Nudge, grid, output meter). When a row is too narrow for every group,
/// the groups that do not fit go into its labelled overflow menu ("More tools…", "More
/// controls ▾"), lowest priority first; the transport and Nudge are the last to go.
pub const HEIGHT: f32 = 60.0;
/// Height of the bottom bar.
pub const BOTTOM_HEIGHT: f32 = 34.0;

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

/// Where the always-reachable controls were drawn last frame (RC09/HF08 evidence and tests).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolbarLayout {
    /// The top (editing tools) row.
    pub visible: Rect,
    /// The bottom bar.
    pub bottom: Rect,
    /// The edit tools and audio tools groups (`Rect::NOTHING` when in the overflow menu).
    pub tools: Rect,
    pub transport: Rect,
    pub nudge: Rect,
    /// Groups in each row's overflow menu this frame (0: everything is in view).
    pub top_overflow: u8,
    pub bottom_overflow: u8,
}

impl Default for ToolbarLayout {
    fn default() -> Self {
        ToolbarLayout {
            visible: Rect::NOTHING,
            bottom: Rect::NOTHING,
            tools: Rect::NOTHING,
            transport: Rect::NOTHING,
            nudge: Rect::NOTHING,
            top_overflow: 0,
            bottom_overflow: 0,
        }
    }
}

/// Height of the top (editing tools) row.
pub fn height(_app: &SoundApp) -> f32 {
    HEIGHT
}

type Draw = fn(&mut SoundApp, &mut Ui);

/// One group of a row: its key (for the remembered width), a first-frame width estimate, its
/// caption in the overflow menu, and how to draw it.
struct Item {
    key: &'static str,
    est: f32,
    caption: &'static str,
    draw: Draw,
}

/// Lays out `items` (drawn in this order) in the row: groups are kept in `priority` order while
/// they fit; the others go into a menu named `more`. Returns each drawn group's rect (by index;
/// `Rect::NOTHING` when in the menu or not drawn) and how many went into the menu.
fn row(app: &mut SoundApp, ui: &mut Ui, id: &str, items: &[Item], priority: &[usize], more: &str, force_first: bool) -> (Vec<Rect>, u8) {
    const GAP: f32 = 8.0;
    let width = |ui: &Ui, it: &Item| ui.ctx().data(|d| d.get_temp::<f32>(egui::Id::new((id, it.key)))).unwrap_or(it.est);
    let avail = ui.available_width() - 16.0;
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new((id, "avail")), avail));
    // Narrow rows get a small "…" menu button (same accessible name).
    let narrow = avail < 600.0;
    let more_w = if narrow { 30.0 } else { 120.0 };
    let fit = |reserve: f32| {
        let mut shown = vec![false; items.len()];
        let mut used = reserve;
        for (k, &i) in priority.iter().enumerate() {
            if let Some(it) = items.get(i) {
                let w = width(ui, it) + GAP;
                if used + w <= avail || (force_first && k == 0) {
                    used += w;
                    if let Some(s) = shown.get_mut(i) {
                        *s = true;
                    }
                }
            }
        }
        shown
    };
    let mut shown = fit(0.0);
    if shown.iter().any(|s| !s) {
        shown = fit(more_w);
    }
    let mut rects = vec![Rect::NOTHING; items.len()];
    let hidden: Vec<usize> = (0..items.len()).filter(|i| !shown.get(*i).copied().unwrap_or(false)).collect();
    ui.horizontal_centered(|ui| {
        ui.add_space(8.0);
        ui.spacing_mut().item_spacing.x = GAP;
        // On a narrow row the menu comes first, so it can never be pushed off the edge.
        let more_menu = |app: &mut SoundApp, ui: &mut Ui| {
            if hidden.is_empty() {
                return;
            }
            let r = ui.menu_button(if narrow { "…" } else { more }, |ui| {
                for &i in &hidden {
                    let Some(it) = items.get(i) else { continue };
                    ui.label(
                        egui::RichText::new(it.caption).font(crate::theme::bold(crate::theme::size::CAPTION)).color(Tokens::current().section_label),
                    );
                    let r = ui.scope(|ui| ui.horizontal(|ui| (it.draw)(app, ui))).response.rect;
                    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new((id, it.key)), r.width()));
                    ui.separator();
                }
            });
            let name = more.trim_end_matches('…').to_string();
            r.response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
            r.response.on_hover_text("Controls that do not fit this window width; nothing here is hidden for good");
        };
        if narrow {
            more_menu(app, ui);
        }
        for (i, it) in items.iter().enumerate() {
            if !shown.get(i).copied().unwrap_or(false) {
                continue;
            }
            let r = ui.scope(|ui| (it.draw)(app, ui)).response.rect;
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new((id, it.key)), r.width()));
            if let Some(slot) = rects.get_mut(i) {
                *slot = r;
            }
        }
        if !narrow {
            more_menu(app, ui);
        }
    });
    (rects, hidden.len() as u8)
}

/// The top row: the editing tools, next to the tracks.
pub fn show(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, t.toolbar_bg);
    let items = [
        Item { key: "modes", est: 112.0, caption: "EDIT MODE", draw: edit_modes },
        Item { key: "tools", est: 306.0, caption: "TOOLS", draw: tools },
        Item { key: "audio_tools", est: 364.0, caption: "AUDIO TOOLS", draw: audio_tools },
        Item { key: "zoom", est: 128.0, caption: "ZOOM", draw: zoom_group },
    ];
    // Tools first; audio tools, edit modes (F1–F4) and zoom (shortcuts too) give way first.
    let (rects, hidden) = row(app, ui, "edit_top", &items, &[1, 2, 0, 3], "More tools…", false);
    let mut layout = app.toolbar_layout;
    layout.visible = full;
    layout.tools = match (rects.get(1).copied(), rects.get(2).copied()) {
        (Some(a), Some(b)) if a != Rect::NOTHING && b != Rect::NOTHING => a.union(b),
        (Some(a), _) => a,
        _ => Rect::NOTHING,
    };
    layout.top_overflow = hidden;
    app.toolbar_layout = layout;
}

/// The bottom bar: locate buttons, transport, counters, Nudge, grid and the output meter.
pub fn show_bottom(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, t.toolbar_bg);
    ui.painter().line_segment([full.left_top(), full.right_top()], Stroke::new(1.0, t.border));
    let items = [
        Item { key: "locate", est: 120.0, caption: "LOCATE", draw: locate },
        Item { key: "transport", est: 240.0, caption: "TRANSPORT", draw: transport },
        Item { key: "counters", est: 380.0, caption: "COUNTERS", draw: counters },
        Item { key: "nudge", est: 300.0, caption: "NUDGE", draw: nudge_panel },
        Item { key: "grid", est: 150.0, caption: "GRID", draw: grid_nudge },
        Item { key: "meter", est: 48.0, caption: "OUTPUT", draw: output_meter },
    ];
    // The transport is always in the bar (Record is never hidden).
    let (rects, hidden) = row(app, ui, "edit_bottom", &items, &[1, 3, 2, 0, 4, 5], "More controls…", true);
    let mut layout = app.toolbar_layout;
    layout.bottom = full;
    layout.transport = rects.get(1).copied().unwrap_or(Rect::NOTHING);
    layout.nudge = rects.get(3).copied().unwrap_or(Rect::NOTHING);
    layout.bottom_overflow = hidden;
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

/// Main counter (click: format; the sub counter shows on hover) and the selection's Start / End
/// / Length, in one row.
fn counters(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let (main_fmt, sub_fmt, sel) = {
        let s = app.engine.session();
        (s.edit.main_counter, s.edit.sub_counter.unwrap_or(TimeFormat::Samples), s.edit.selection)
    };
    let pos = app.position();
    crate::theme::group_frame().fill(t.counter_bg).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let main = egui::Button::new(egui::RichText::new(fmt_pos(app, pos, main_fmt)).font(mono(16.0)).color(t.counter_text)).frame(false);
            let sub = fmt_pos(app, pos, sub_fmt);
            let resp =
                ui.add(main).on_hover_text(format!("Main counter (click to choose the time format)\nSub counter: {sub} ({})", sub_fmt.label()));
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Main counter"));
            egui::Popup::menu(&resp).show(|ui| {
                for f in TimeFormat::ALL {
                    if ui.selectable_label(main_fmt == f, f.label()).clicked() {
                        let _ = app.run("view.main_counter", json!({"format": f.id()}));
                    }
                }
            });
            let s = app.engine.session();
            for (label, val) in [
                ("Start", fmt_pos(app, sel.start, main_fmt)),
                ("End", fmt_pos(app, sel.end, main_fmt)),
                ("Length", format_length(sel.start, sel.len(), main_fmt, s.sample_rate, &s.tempo, s.frame_rate)),
            ] {
                ui.spacing_mut().item_spacing.x = 3.0;
                ui.label(egui::RichText::new(label).font(regular(10.0)).color(t.counter_label));
                ui.label(egui::RichText::new(val).font(mono(11.0)).color(t.counter_text));
                ui.add_space(3.0);
            }
        });
    });
}

/// Grid lines toggle and the grid value.
fn grid_nudge(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let (grid, lines) = {
        let e = &app.engine.session().edit;
        (e.grid, e.grid_lines)
    };
    crate::theme::group_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if crate::control_bar::chip(ui, None, "Grid", "Grid lines", lines, t.mode_on, true).on_hover_text("Show grid lines").clicked() {
                let _ = app.run("view.grid_lines", json!({}));
            }
            let resp = crate::control_bar::chip_menu(ui, &grid.label(), "Grid value", true).on_hover_text("Grid value");
            value_menu(app, &resp, "view.grid");
        });
    });
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

/// The dedicated NUDGE group in one row: arrows, value, unit (with the 1 smp / 1 ms / 10 ms
/// presets) and target; the exact displacement ("= 480.00 smp @ 48.0 kHz") shows on hover. Separate from the Grid value.
fn nudge_panel(app: &mut SoundApp, ui: &mut Ui) {
    use soundcraft_model::{NudgeTarget, NudgeUnit};
    let p = app.engine.session().edit.nudge_panel.clone();
    let sr = app.engine.session().sample_rate.as_f64();
    let readout = match p.unit {
        NudgeUnit::Ticks => format!("{:.0} ticks (tempo map)", p.value),
        _ => {
            let smp = soundcraft_engine::nudge_step_samples(app.engine.session()).unwrap_or(0.0);
            format!("= {smp:.2} smp @ {:.1} kHz", sr / 1000.0)
        }
    };
    crate::theme::group_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(egui::RichText::new("NUDGE").strong().small()).on_hover_text(&readout);
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
            if ui.add(egui::DragValue::new(&mut v).range(0.001..=100_000.0).speed(0.1).max_decimals(3)).on_hover_text(&readout).changed() {
                let _ = app.run("nudge.set", json!({"value": v}));
            }
            egui::ComboBox::from_id_salt("nudge_unit").selected_text(p.unit.label()).width(56.0).show_ui(ui, |ui| {
                for u in [NudgeUnit::Samples, NudgeUnit::Milliseconds, NudgeUnit::Seconds, NudgeUnit::Ticks, NudgeUnit::Frames] {
                    if ui.selectable_label(p.unit == u, u.label()).clicked() {
                        let _ = app.run("nudge.set", json!({"unit": u.label()}));
                    }
                }
                ui.separator();
                for (v, u, label) in [(1.0, "samples", "1 smp"), (1.0, "ms", "1 ms"), (10.0, "ms", "10 ms")] {
                    if ui.button(label).clicked() {
                        let _ = app.run("nudge.set", json!({"value": v, "unit": u}));
                    }
                }
            });
            egui::ComboBox::from_id_salt("nudge_target").selected_text(p.target.label()).width(110.0).show_ui(ui, |ui| {
                for tg in NudgeTarget::ALL {
                    if ui.selectable_label(p.target == tg, tg.label()).clicked() {
                        let _ = app.run("nudge.set", json!({"target": tg.id()}));
                    }
                }
            });
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

/// Return to Zero, Rewind, Fast Forward, Go to End.
fn locate(app: &mut SoundApp, ui: &mut Ui) {
    crate::theme::group_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            for (icon, cmd, tip) in [
                ("rtz", "transport.rtz", "Return to Zero (Home)"),
                ("rewind", "transport.rewind", "Rewind"),
                ("ffwd", "transport.fast_forward", "Fast Forward"),
                ("end", "transport.go_to_end", "Go to End (End)"),
            ] {
                if icon_button(ui, vec2(26.0, 22.0), icon, false, tip).clicked() {
                    let _ = app.run(cmd, json!({}));
                }
            }
        });
    });
}

/// Stop / Play / Record (the same named buttons as the control bar's) and Loop Playback. Never
/// in the overflow menu; on a narrow bar the three are drawn as icons (same names and tooltips).
fn transport(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let playing = app.is_playing();
    let recording = app.engine.transport.recording;
    let looped = app.engine.session().edit.loop_playback;
    let narrow = ui.ctx().data(|d| d.get_temp::<f32>(egui::Id::new(("edit_bottom", "avail")))).is_some_and(|w| w < 700.0);
    let label = |s: &'static str| if narrow { "" } else { s };
    crate::theme::group_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            if crate::control_bar::chip(ui, Some("stop"), label("Stop"), "■ Stop", false, t.button, true).on_hover_text("Stop (Space)").clicked() {
                let _ = app.run("transport.stop", json!({}));
            }
            if crate::control_bar::chip(ui, Some("play"), label("Play"), "▶ Play", playing && !recording, t.playback, true)
                .on_hover_text("Play (Space)")
                .clicked()
            {
                let _ = app.run("transport.play", json!({}));
            }
            if crate::control_bar::chip(ui, Some("record"), label("Record"), "● Record", recording, t.rec, true)
                .on_hover_text("Record (Cmd+Space)")
                .clicked()
            {
                let _ = app.run("transport.record", json!({}));
            }
            if icon_button(ui, vec2(26.0, 22.0), "loop", looped, "Loop Playback (Cmd+Shift+L)").clicked() {
                let _ = app.run("options.loop_playback", json!({}));
            }
        });
    });
}

/// The main output level, two horizontal bars.
fn output_meter(app: &mut SoundApp, ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(48.0, 22.0), Sense::hover());
    let m = app.main_meter;
    let h = (r.height() - 2.0) / 2.0;
    for c in 0..2 {
        let rr = Rect::from_min_size(pos2(r.min.x, r.min.y + c as f32 * (h + 2.0)), vec2(r.width(), h));
        crate::widgets::meter(ui, rr, m.level[c], m.hold[c], m.clip);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Renders the Edit window's two rows alone in a `w`×`h` (logical points) screen, a few
    /// frames (the first learns the group widths), and returns where things landed.
    fn layout_at(w: f32, h: f32, light: bool) -> ToolbarLayout {
        let mut app = SoundApp::new(soundcraft_engine::Engine::default(), None, crate::Services::default());
        let ctx = egui::Context::default();
        crate::fonts::install(&ctx);
        ctx.set_visuals(if light { egui::Visuals::light() } else { egui::Visuals::dark() });
        let raw = || egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(w, h))), ..Default::default() };
        for _ in 0..4 {
            let mut out = ctx.run_ui(raw(), |ui| {
                egui::Panel::top("edit_toolbar").exact_size(HEIGHT).frame(egui::Frame::NONE).show(ui, |ui| show(&mut app, ui));
                egui::Panel::bottom("edit_bottom_bar").exact_size(BOTTOM_HEIGHT).frame(egui::Frame::NONE).show(ui, |ui| show_bottom(&mut app, ui));
            });
            out.textures_delta.clear();
        }
        app.toolbar_layout
    }

    /// HF08 (keeps RC09's intent): transport and Nudge are fully on screen, never overlapping,
    /// at 1366×768 at 150 % (911×512), 1280, 1366, 1920 and 2560 points, light and dark; the
    /// tools lead the top row; nothing is in an overflow menu from 1366 points up.
    #[test]
    fn transport_and_nudge_are_reachable_on_small_screens() {
        for (w, h) in [(1366.0 / 1.5, 768.0 / 1.5), (1280.0, 720.0), (1366.0, 768.0), (1920.0, 1080.0), (2560.0, 1440.0)] {
            for light in [false, true] {
                let l = layout_at(w, h, light);
                for (name, r) in [("transport", l.transport), ("nudge", l.nudge)] {
                    assert!(r.width() > 20.0 && r.height() > 10.0, "{w}x{h} {name} drawn: {r:?}");
                    assert!(
                        r.min.x >= 0.0 && r.max.x <= w + 0.5 && r.min.y >= l.bottom.min.y - 0.5 && r.max.y <= h + 0.5,
                        "{w}x{h} {name} in the bottom bar: {r:?} {l:?}"
                    );
                }
                assert!(!l.transport.intersects(l.nudge.shrink(1.0)), "{w}x{h}: transport and Nudge do not overlap");
                assert!(l.tools.min.x >= 0.0 && l.tools.max.x <= w + 0.5 && l.tools.max.y <= HEIGHT + 0.5, "{w}x{h}: tools in the top row {l:?}");
                if w >= 1366.0 {
                    assert_eq!((l.top_overflow, l.bottom_overflow), (0, 0), "{w}x{h}: everything in view {l:?}");
                }
            }
        }
    }

    /// The Record layout beside its 300-point panel (611 points) and the minimum window at 150 %
    /// (340 points beside the panel): one row each, no scrolling; groups that do not fit are in
    /// the overflow menus, and the transport stays in the bar.
    #[test]
    fn narrow_rows_overflow_instead_of_scrolling() {
        for w in [611.0, 340.0] {
            let l = layout_at(w, 512.0, false);
            assert!(l.top_overflow > 0 && l.bottom_overflow > 0, "{w}: overflow used {l:?}");
            assert!(l.transport.width() > 20.0 && l.transport.min.x >= 0.0 && l.transport.max.x <= w + 0.5, "{w}: transport in the bar {l:?}");
        }
        let l = layout_at(611.0, 512.0, false);
        assert!(l.tools.width() > 20.0 && l.tools.max.x <= 611.5, "611: tools in the top row {l:?}");
        assert!(l.nudge.width() > 20.0 && l.nudge.max.x <= 611.5, "611: Nudge stays in the bar {l:?}");
        // At the extreme minimum the tools group (306 points) is in the "More tools" menu.
        assert_eq!(layout_at(340.0, 400.0, false).tools, Rect::NOTHING);
    }
}
