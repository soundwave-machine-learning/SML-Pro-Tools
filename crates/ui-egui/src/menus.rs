//! The menu bar, built from the incumbent's menu catalog so every item has its place. Items
//! that map to a command are live; the rest are shown disabled (and listed in the parity report).

use crate::{MainWindow, SoundApp};
use serde_json::{Value, json};
use soundcraft_engine::catalog;

/// UI-layer commands: (id, label, catalog path or "", shortcut).
pub const UI_COMMANDS: &[(&str, &str, &str, Option<&str>)] = &[
    ("ui.dock", "Arrange Panel", "", None),
    ("ui.dock.reset", "Reset Panel Layout", "Window > Reset Panel Layout", None),
    ("ui.mix_split", "Mixer Split Size", "", None),
    ("window.mix", "Mix", "Window > Mix", Some("Cmd+=")),
    ("window.edit", "Edit", "Window > Edit", Some("Cmd+=")),
    ("window.toggle_mix_edit", "Toggle Mix/Edit", "", Some("Cmd+=")),
    ("window.transport", "Transport", "Window > Transport", Some("Cmd+1")),
    ("window.big_counter", "Big Counter", "Window > Big Counter", Some("Cmd+3")),
    ("window.memory_locations", "Memory Locations", "Window > Memory Locations", Some("Cmd+5")),
    ("window.undo_history", "Undo History", "Window > Undo History", None),
    ("window.clip_list", "Clip List", "Window > Clip List", None),
    ("window.track_list", "Track List", "View > Other Displays > Track List", None),
    ("window.clip_list_view", "Clip List", "View > Other Displays > Clip List", None),
    ("window.narrow_mix", "Narrow Mix", "View > Narrow Mix", Some("Cmd+Alt+M")),
    ("window.session_info", "Session Info", "", None),
    ("window.recording", "Recording", "Window > Recording", None),
    ("window.record", "Record Layout", "Window > Record Layout", None),
    ("window.master", "Master Layout", "Window > Master Layout", None),
    ("window.remote", "SML Remote", "Window > SML Remote", None),
    ("window.help", "Command Help", "Help > Command Help", Some("F1")),
    ("window.assistant", "Assistant", "Help > Assistant", None),
    ("remote.start", "Start SML Remote", "", None),
    ("remote.stop", "Stop SML Remote", "", None),
    ("remote.pair", "Open Remote Pairing", "", None),
    ("remote.revoke", "Revoke Remote Client", "", None),
    ("remote.status", "SML Remote Status", "", None),
    ("monitor.volume", "Monitor Volume", "", None),
    ("monitor.dim", "Monitor Dim", "", None),
    ("monitor.mono", "Monitor Mono", "", None),
    ("monitor.cue", "Headphone (Cue) Control", "", None),
    ("window.audio_health", "Session Audio Health", "", None),
    ("window.about", "About SML Recording", "", None),
    ("window.close", "Close Window", "Window > Close Window", Some("Cmd+W")),
    ("window.hide_floating", "Hide All Floating Windows", "Window > Hide All Floating Windows", Some("Cmd+Ctrl+W")),
    ("window.midi_editor", "MIDI Editor", "Window > MIDI Editor", None),
    ("window.universe", "Universe", "View > Other Displays > Universe", None),
    ("window.video", "Video", "Window > Video", Some("Cmd+9")),
    ("window.video_universe", "Video Universe", "Window > Video Universe", None),
    ("window.automation", "Automation", "Window > Automation", Some("Cmd+4")),
    ("window.color_palette", "Color Palette", "Window > Color Palette", None),
    ("window.disk_usage", "Disk Usage", "Window > Disk Usage", None),
    ("window.system_usage", "System Usage", "Window > System Usage", None),
    ("window.task_manager", "Task Manager", "Window > Task Manager", None),
    ("window.metadata", "Metadata Inspector", "Window > Metadata Inspector", None),
    ("window.event_list", "MIDI Event List", "Window > MIDI Event List", None),
    ("window.midi_keyboard", "MIDI Keyboard", "Window > MIDI Keyboard", None),
    ("window.workspace", "Workspace", "Window > New Workspace > Default", None),
    ("window.workspace_presets", "Workspace", "Window > New Workspace > Track Presets", None),
    ("window.workspace_sounds", "Workspace", "Window > New Workspace > Soundbase", None),
    ("window.workspace_front", "Workspaces to Front", "Window > Workspaces > Bring to Front", None),
    ("window.workspace_close", "Close All Workspaces", "Window > Workspaces > Close All Workspaces", None),
    ("window.configurations", "Window Configuration List", "Window > Configurations > Window Configuration List", None),
    ("window.config_new", "New Configuration", "Window > Configurations > New Configuration...", None),
    ("window.config_update", "Update Active Configuration", "Window > Configurations > Update Active Configuration", None),
    ("window.config_auto_update", "Auto-Update Active Configuration", "Window > Configurations > Auto-Update Active Configuration", None),
    ("window.renderer", "Renderer", "Window > Renderer", None),
    ("window.ui_customization", "UI Customization", "Window > UI Customization", None),
    ("window.arrange_tile", "Tile", "Window > Arrange > Tile", None),
    ("window.arrange_tile_h", "Tile Horizontal", "Window > Arrange > Tile Horizontal", None),
    ("window.arrange_tile_v", "Tile Vertical", "Window > Arrange > Tile Vertical", None),
    ("window.arrange_cascade", "Cascade", "Window > Arrange > Cascade", None),
    ("window.midi_front", "MIDI Editors to Front", "Window > MIDI Editors > Bring to Front", None),
    ("window.midi_back", "MIDI Editors to Back", "Window > MIDI Editors > Send to Back", None),
    ("window.clip_effects", "Clip Effects", "Window > Clip Effects", None),
    ("window.clip_effects_dock", "Clip Effects", "View > Other Displays > Lower Dock > Clip Effects", None),
    ("window.search", "Search", "Window > Pro Tools Search", Some("Cmd+Ctrl+S")),
    ("window.score", "Score Editor", "Window > Score Editor", Some("Cmd+Ctrl+=")),
    ("window.beat_detective", "Beat Detective", "", None),
    ("window.tempo_ops", "Tempo Operations", "", None),
    ("window.time_ops", "Time Operations", "", None),
    ("window.midi_ops", "MIDI Operations", "", None),
    ("window.rtp", "MIDI Real-Time Properties", "", None),
    ("window.playback_engine", "Playback Engine", "", None),
    ("window.audio_setup", "Audio Setup", "", None),
    ("window.recover", "Recover Autosave…", "File > Recover Autosave...", None),
    ("window.io_setup", "I/O Setup", "", None),
    ("window.shortcuts", "Keyboard Shortcuts", "", None),
    ("view.mix_section", "Mix Window View", "", None),
    ("ui.new_tracks_dialog", "New Tracks…", "", None),
    ("ui.bounce_dialog", "Bounce Mix…", "", None),
    ("ui.set", "Set UI State", "", None),
    ("ui.theme", "Appearance", "", None),
    ("ui.theme_import", "Import Theme…", "", None),
    ("ui.theme_export", "Export Theme…", "", None),
    ("ui.theme_reset", "Reset Theme", "", None),
    ("ui.theme_token", "Set Theme Token", "", None),
    ("window.theme_editor", "Theme Editor", "Window > Theme Editor", None),
    ("ui.shortcut_profile", "Shortcut Profile", "", None),
    ("ui.shortcut_set", "Assign Shortcut", "", None),
    ("ui.shortcuts_reset", "Reset Custom Shortcuts", "", None),
    ("ui.shortcuts_export", "Export Shortcuts…", "", None),
    ("ui.shortcuts_import", "Import Shortcuts…", "", None),
    ("ui.shortcuts_conflicts", "Shortcut Conflicts", "", None),
];

/// Extra catalog mappings handled by the UI layer.
pub fn ui_aliases() -> Vec<(&'static str, &'static str)> {
    let mut v: Vec<(&str, &str)> = UI_COMMANDS.iter().filter(|c| !c.2.is_empty()).map(|c| (c.2, c.0)).collect();
    v.extend(mix_view_aliases().iter().copied());
    v.extend(AUDIOSUITE.iter().map(|(p, _)| (*p, "audiosuite.process")));
    v.extend(MENU_WINDOWS.iter().copied());
    v
}

/// Mix-view menu paths, allocated once. Rebuilding them on every menu draw leaked a copy per frame.
fn mix_view_aliases() -> &'static [(&'static str, &'static str)] {
    static ALIASES: std::sync::OnceLock<Vec<(&'static str, &'static str)>> = std::sync::OnceLock::new();
    ALIASES.get_or_init(|| {
        [
            "Inserts A-E",
            "Inserts F-J",
            "Sends A-E",
            "Sends F-J",
            "I/O",
            "Track Color",
            "Comments",
            "EQ Curve",
            "Meters and Faders",
            "All",
            "Minimal",
            "Mic Preamps",
            "Instruments",
            "Object",
        ]
        .into_iter()
        .map(|sec| {
            let path: &'static str = Box::leak(format!("View > Mix Window Views > {sec}").into_boxed_str());
            (path, "view.mix_section")
        })
        .collect()
    })
}

/// AudioSuite catalog entries → our processes (functional equivalents, our own plugins).
pub const AUDIOSUITE: &[(&str, &str)] = &[
    ("AudioSuite > EQ > Channel Strip", "channel_strip"),
    ("AudioSuite > EQ > EQ3 1-Band", "eq_1band"),
    ("AudioSuite > EQ > EQ3 7-Band", "eq_7band"),
    ("AudioSuite > Dynamics > BF-76", "compressor"),
    ("AudioSuite > Dynamics > Channel Strip", "channel_strip"),
    ("AudioSuite > Dynamics > Dyn3 Compressor/Limiter", "compressor"),
    ("AudioSuite > Dynamics > Dyn3 De-Esser", "de_esser"),
    ("AudioSuite > Dynamics > Dyn3 Expander/Gate", "expander_gate"),
    ("AudioSuite > Dynamics > Maxim", "maximizer"),
    ("AudioSuite > Pitch Shift > Pitch II", "pitch_shift"),
    ("AudioSuite > Pitch Shift > Pitch Shift Legacy", "pitch_shift"),
    ("AudioSuite > Pitch Shift > Time Shift", "time_stretch"),
    ("AudioSuite > Pitch Shift > Vari-Fi", "varispeed"),
    ("AudioSuite > Reverb > D-Verb", "room_reverb"),
    ("AudioSuite > Delay > Mod Delay III", "mod_delay"),
    ("AudioSuite > Modulation > Sci-Fi", "flanger"),
    ("AudioSuite > Harmonic > Eleven Lite", "saturator"),
    ("AudioSuite > Harmonic > Lo-Fi", "lofi"),
    ("AudioSuite > Harmonic > Recti-Fi", "rectifier"),
    ("AudioSuite > Harmonic > SansAmp PSA-1", "saturator"),
    ("AudioSuite > Other > DC Offset Removal", "dc_offset_removal"),
    ("AudioSuite > Other > Duplicate", "duplicate"),
    ("AudioSuite > Other > Gain", "gain"),
    ("AudioSuite > Other > Invert", "invert"),
    ("AudioSuite > Other > Normalize", "normalize"),
    ("AudioSuite > Other > Reverse", "reverse"),
    ("AudioSuite > Other > Signal Generator", "signal_generator"),
    ("AudioSuite > Other > Time Compression Expansion", "time_stretch"),
];

/// Commands that open a dialog when invoked from a menu (programmatic calls never do).
const DIALOG_COMMANDS: &[&str] = &[
    "track.new",
    "file.bounce_mix",
    "session.new",
    "file.import_audio",
    "file.import_midi",
    "session.open",
    "session.save_as",
    "session.save_copy",
    "track.group",
    "edit.repeat",
    "edit.shift",
    "edit.fades_create",
    "setup.session",
    "event.quantize",
    "event.transpose",
    "edit.strip_silence",
];

/// Shared menu hierarchy for egui and native desktop hosts.
pub struct MenuNode {
    pub label: String,
    pub path: String,
    pub children: Vec<MenuNode>,
}

fn insert_path(roots: &mut Vec<MenuNode>, line: &str) {
    let parts: Vec<&str> = line.split(" > ").collect();
    let mut level = roots;
    let mut path = String::new();
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            path.push_str(" > ");
        }
        path.push_str(p);
        let idx = match level.iter().position(|n| n.label == *p) {
            Some(i) => i,
            None => {
                level.push(MenuNode { label: p.to_string(), path: path.clone(), children: Vec::new() });
                level.len() - 1
            }
        };
        let Some(node) = level.get_mut(idx) else { break };
        level = &mut node.children;
    }
}

/// The menu model, built once per process (E04/R19: opening a menu never rebuilds the catalog
/// tree; the per-item enabled/checked state is the only per-frame work).
pub fn tree() -> &'static [MenuNode] {
    static TREE: std::sync::OnceLock<Vec<MenuNode>> = std::sync::OnceLock::new();
    TREE.get_or_init(|| {
        let mut roots: Vec<MenuNode> = Vec::new();
        for line in catalog::catalog() {
            insert_path(&mut roots, line);
        }
        insert_path(&mut roots, "Window > Reset Panel Layout");
        for (path, _) in MENU_WINDOWS {
            insert_path(&mut roots, path);
        }
        roots
    })
}

pub fn menu_bar(app: &mut SoundApp, ui: &mut egui::Ui) {
    let t = crate::theme::Tokens::current();
    egui::Panel::top("menu_bar").exact_size(24.0).frame(egui::Frame::NONE.fill(t.menu_bar).inner_margin(egui::Margin::symmetric(8, 2))).show(
        ui,
        |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("SML Studio", |ui| {
                    if ui.button("About SML Studio").clicked() {
                        app.ui.show_about = true;
                    }
                    if ui.button("Session Info").clicked() {
                        app.ui.show_session_info = true;
                    }
                    if ui.button("Session Audio Health").clicked() {
                        let _ = app.run("window.audio_health", json!({"value": true}));
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        let _ = app.run("app.quit", json!({}));
                    }
                });
                let extra = ui_aliases();
                for root in tree() {
                    ui.menu_button(&root.label, |ui| {
                        for c in &root.children {
                            menu_node(app, ui, c, &extra);
                        }
                        if root.label == "AudioSuite" {
                            ui.separator();
                            ui.label(egui::RichText::new("Processes selected clips offline").small().color(t.text_dim));
                        }
                    });
                }
                let title = format!("{}{}", app.engine.session().name, if app.engine.is_dirty() { " *" } else { "" });
                // Compact windows: the menus keep their room. The session title shortens first;
                // if the layout switcher still does not fit, it moves into the control bar
                // (crate::layouts::switcher_overflows) instead of covering the last menus.
                let room = ui.available_width() - 8.0;
                let overflow = room < crate::layouts::SWITCHER_WIDTH;
                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(crate::layouts::OVERFLOW_KEY), overflow));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let title_room = if overflow { room } else { room - crate::layouts::SWITCHER_WIDTH - 10.0 };
                    if title_room > 40.0 {
                        ui.scope(|ui| {
                            ui.set_max_width(title_room);
                            ui.add(egui::Label::new(egui::RichText::new(&title).color(t.text_dim)).truncate()).on_hover_text(title.as_str());
                        });
                    }
                    if !overflow {
                        ui.add_space(6.0);
                        crate::layouts::switcher(app, ui);
                    }
                });
            });
        },
    );
}

fn menu_node(app: &mut SoundApp, ui: &mut egui::Ui, n: &MenuNode, extra: &[(&str, &str)]) {
    if !n.children.is_empty() {
        ui.menu_button(&n.label, |ui| {
            for c in &n.children {
                menu_node(app, ui, c, extra);
            }
        });
        return;
    }
    let id = command_for_path(&n.path, extra);
    let (enabled, checked) = item_state(app, &n.path, id.as_deref());
    let label = if checked { format!("✔ {}", n.label) } else { n.label.clone() };
    // The active profile's chord (R13), not the registry's Pro Tools string.
    let shortcut = id.as_deref().and_then(|i| app.shortcut_labels.get(i)).cloned().unwrap_or_default();
    let btn = egui::Button::new(label).shortcut_text(crate::shortcuts::shortcut_label(&shortcut, ui.ctx().os().is_mac()));
    // A disabled item says why (e.g. "not implemented in this build"), never a silent grey entry.
    let reason = if enabled { None } else { id.as_deref().and_then(soundcraft_engine::find_command).and_then(|c| (c.enabled)(&app.engine).err()) };
    let resp = ui.add_enabled(enabled, btn);
    let resp = match reason {
        Some(r) => resp.on_disabled_hover_text(r),
        None => resp,
    };
    if resp.clicked()
        && let Some(id) = id
    {
        invoke_menu(app, &id, &n.path);
        ui.close();
    }
}

/// Resolve the command behind a menu leaf, including UI dialog overrides.
pub fn command_for_path(path: &str, extra: &[(&str, &str)]) -> Option<String> {
    catalog::implemented_by(path, extra).or_else(|| MENU_WINDOWS.iter().find(|(p, _)| *p == path).map(|(_, w)| w.to_string()))
}

/// Enabled and checked state shared by all menu presentations.
pub fn item_state(app: &SoundApp, path: &str, id: Option<&str>) -> (bool, bool) {
    let enabled = match id {
        Some(i)
            if i.starts_with("window.") || i.starts_with("view.mix_section") || matches!(i, "audiosuite.process" | "ui.dock" | "ui.dock.reset") =>
        {
            true
        }
        Some(i) => soundcraft_engine::find_command(i).is_some_and(|c| (c.enabled)(&app.engine).is_ok()),
        None => false,
    };
    (enabled, checked_state(app, path, id))
}

fn checked_state(app: &SoundApp, path: &str, id: Option<&str>) -> bool {
    let e = &app.engine.session().edit;
    match id {
        Some("options.loop_playback") => e.loop_playback,
        Some("options.loop_record") => e.loop_record,
        // Disabled (B04/RC03): a legacy session's flag is kept but never shown as active.
        Some("options.quickpunch") => false,
        Some("options.pre_post_roll") => e.pre_post_roll,
        Some("options.link_timeline_edit") => e.link_timeline_edit,
        Some("options.link_track_edit") => e.link_track_edit,
        Some("options.insertion_follows_playback") => e.insertion_follows_playback,
        Some("options.tab_to_transient") => e.tab_to_transient,
        Some("options.automation_follows_edit") => e.automation_follows_edit,
        Some("options.markers_follow_edit") => e.markers_follow_edit,
        Some("options.click") => e.click,
        Some("options.delay_compensation") => e.delay_compensation,
        Some("options.pre_fader_metering") => e.pre_fader_metering,
        Some("view.ruler") => path.rsplit(" > ").next().map(ruler_id).is_some_and(|r| e.rulers.iter().any(|x| x == r)),
        Some("view.main_counter") => path.rsplit(" > ").next().map(ruler_id).is_some_and(|r| e.main_counter.id() == r),
        Some("window.mix") => app.ui.window == MainWindow::Mix,
        Some("window.edit") => app.ui.window == MainWindow::Edit,
        Some("window.record") => app.ui.window == MainWindow::Record,
        Some("window.master") => app.ui.window == MainWindow::Master,
        Some("window.remote") => app.ui.show_remote,
        Some("window.help") => app.ui.show_help,
        Some("window.assistant") => app.ui.show_assistant,
        Some("window.track_list") => app.ui.show_tracks_list,
        Some("window.clip_list_view") | Some("window.clip_list") => app.ui.show_clip_list,
        Some("window.narrow_mix") => app.ui.narrow_mix,
        Some("window.renderer") => app.extra.show_renderer,
        Some("window.ui_customization") => app.extra.show_ui_customization,
        Some("window.config_auto_update") => app.extra.auto_update_config,
        Some("view.mix_section") => path.rsplit(" > ").next().map(mix_section_id).is_some_and(|s| app.ui.mix_views.iter().any(|v| v == s)),
        _ => false,
    }
}

fn ruler_id(label: &str) -> &'static str {
    match label {
        "Bars|Beats" => "bars_beats",
        "Min:Secs" => "min_secs",
        "Timecode" => "timecode",
        "Feet+Frames" => "feet_frames",
        "Samples" => "samples",
        "Tempo" => "tempo",
        "Meter" => "meter",
        "Markers" => "markers",
        "Key Signature" => "key",
        "Chord Symbols" => "chords",
        _ => "",
    }
}

fn mix_section_id(label: &str) -> &'static str {
    match label {
        "Inserts A-E" => "inserts_ae",
        "Inserts F-J" => "inserts_fj",
        "Sends A-E" => "sends_ae",
        "Sends F-J" => "sends_fj",
        "I/O" => "io",
        "Track Color" => "color",
        "Comments" => "comments",
        "EQ Curve" => "eq_curve",
        "Meters and Faders" => "meters",
        "All" => "all",
        "Minimal" => "minimal",
        "Mic Preamps" => "mic_preamps",
        "Instruments" => "instruments",
        "Object" => "object",
        _ => "",
    }
}

/// Parameters implied by the menu item a command was reached through.
fn params_for(path: &str, id: &str) -> Value {
    let leaf = path.rsplit(" > ").next().unwrap_or("");
    match id {
        "view.ruler" => json!({"ruler": ruler_id(leaf)}),
        "view.main_counter" => json!({"format": ruler_id(leaf)}),
        "options.scrolling" => {
            json!({"mode": match leaf { "No Scrolling" => "none", "After Playback" => "after_playback", "Continuous" => "continuous", "Center Playhead" => "center", _ => "page" }})
        }
        "options.solo_mode" => {
            json!({"mode": if leaf.starts_with("AFL") { "afl" } else if leaf.starts_with("PFL") { "pfl" } else { "sip" }, "xor": leaf.starts_with("X-OR")})
        }
        "clip.rating" => json!({"rating": leaf.parse::<u8>().unwrap_or(0)}),
        "track.change_width" => json!({"format": leaf}),
        "view.mix_section" => json!({"section": mix_section_id(leaf)}),
        "audiosuite.process" => json!({"process": AUDIOSUITE.iter().find(|(p, _)| *p == path).map_or("", |(_, x)| *x)}),
        _ => json!({}),
    }
}

/// Menu items that open a SoundCraft window when clicked (programmatic calls still run the command).
const MENU_WINDOWS: &[(&str, &str)] = &[
    ("Setup > Appearance", "window.ui_customization"),
    ("Setup > Hardware...", "window.playback_engine"),
    ("Setup > Playback Engine...", "window.playback_engine"),
    ("Setup > I/O...", "window.io_setup"),
    ("Setup > Keyboard Shortcuts...", "window.shortcuts"),
    ("Setup > Session", "setup.session"),
    ("Event > Beat Detective", "window.beat_detective"),
    ("Event > Tempo Operations > Tempo Operations Window", "window.tempo_ops"),
    ("Event > Time Operations > Time Operations Window", "window.time_ops"),
    ("Event > MIDI Operations > MIDI Operations Window", "window.midi_ops"),
    ("Event > MIDI Real-Time Properties", "window.rtp"),
];

/// Menu-style invocation: may open a dialog.
pub fn invoke_menu(app: &mut SoundApp, id: &str, path: &str) {
    if let Some((_, w)) = MENU_WINDOWS.iter().find(|(p, _)| *p == path)
        && w.starts_with("window.")
    {
        let _ = app.run(w, json!({"value": true}));
        return;
    }
    if id == "audiosuite.process" {
        let p = params_for(path, id);
        app.ui.audiosuite = p.get("process").and_then(Value::as_str).map(str::to_string);
        return;
    }
    if wants_dialog(id) && app.dialogs.open_for_command(&app.engine, id) {
        return;
    }
    // These ids only exist to open a dialog from the menu. Programmatic `app.run` refuses them.
    if id == "ui.new_tracks_dialog" {
        let _ = app.dialogs.open_for_command(&app.engine, "track.new");
        return;
    }
    if id == "ui.bounce_dialog" {
        let _ = app.dialogs.open_for_command(&app.engine, "file.bounce_mix");
        return;
    }
    let params = params_for(path, id);
    let _ = app.run(id, params);
}

/// Keyboard shortcut for a dialog command: open its dialog. Returns true when handled.
pub fn invoke_shortcut_dialog(app: &mut SoundApp, id: &str) -> bool {
    wants_dialog(id) && app.dialogs.open_for_command(&app.engine, id)
}

/// Commands that open a dialog first: the listed ones, Score Setup, and anything that needs a path.
fn wants_dialog(id: &str) -> bool {
    DIALOG_COMMANDS.contains(&id) || id == "file.score_setup" || crate::dialogs::takes_path(id)
}

/// Handle UI-layer commands. Returns None when `id` is not a UI command.
pub fn run_ui_command(app: &mut SoundApp, id: &str, p: &Value) -> Option<Result<Value, String>> {
    if matches!(id, "ui.dock" | "ui.dock.reset") {
        return Some(crate::docking::command(app, id == "ui.dock.reset", p));
    }
    if let Some(v) = crate::extra_windows::run(app, id, p) {
        return Some(Ok(v));
    }
    let toggle = |v: &mut bool| {
        *v = p.get("value").and_then(Value::as_bool).unwrap_or(!*v);
        json!({"value": *v})
    };
    let r = match id {
        "window.mix" => {
            app.ui.window = MainWindow::Mix;
            json!({"window": "mix"})
        }
        "window.edit" => {
            app.ui.window = MainWindow::Edit;
            json!({"window": "edit"})
        }
        "window.toggle_mix_edit" => {
            app.ui.window = if app.ui.window.is_edit() { MainWindow::Mix } else { MainWindow::Edit };
            json!({"window": app.ui.window})
        }
        "window.record" => {
            app.ui.window = MainWindow::Record;
            json!({"window": "record"})
        }
        "window.master" => {
            app.ui.window = MainWindow::Master;
            json!({"window": "master"})
        }
        "window.remote" => toggle(&mut app.ui.show_remote),
        "window.help" => toggle(&mut app.ui.show_help),
        "window.assistant" => toggle(&mut app.ui.show_assistant),
        "remote.start" => match app.remote_start(p.get("bind").and_then(Value::as_str)) {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
        },
        "remote.stop" => {
            app.remote_stop();
            json!({"running": false})
        }
        #[cfg(target_arch = "wasm32")]
        "remote.pair" | "remote.revoke" | "remote.status" => return Some(Err("SML Remote is not available in the web build".into())),
        #[cfg(not(target_arch = "wasm32"))]
        "remote.pair" => match &app.remote {
            Some(s) => {
                let code = s.start_pairing();
                app.remote_code = Some(code.clone());
                json!({"code": code, "url": s.url(), "expires_s": s.pairing_remaining()})
            }
            None => return Some(Err("SML Remote is not running (remote.start first)".into())),
        },
        #[cfg(not(target_arch = "wasm32"))]
        "remote.revoke" => match &app.remote {
            Some(s) => match p.get("id").and_then(Value::as_str) {
                Some(id) => json!({"revoked": s.revoke(id)}),
                None => {
                    s.revoke_all();
                    json!({"revoked": "all"})
                }
            },
            None => return Some(Err("SML Remote is not running".into())),
        },
        #[cfg(not(target_arch = "wasm32"))]
        "remote.status" => match &app.remote {
            Some(s) => {
                json!({"running": true, "url": s.url(), "https": false, "pairing_open": s.pairing_remaining(), "clients": s.tokens(), "events": s.events()})
            }
            None => json!({"running": false, "bind": app.ui.remote_bind}),
        },
        "monitor.volume" => {
            let Some(db) = p.get("db").and_then(Value::as_f64) else { return Some(Ok(json!({"db": app.ui.monitor.volume_db}))) };
            if !db.is_finite() {
                return Some(Err("`db` must be finite".into()));
            }
            app.ui.monitor.volume_db = (db as f32).clamp(-90.0, 0.0);
            json!({"db": app.ui.monitor.volume_db, "path": "monitor (speaker) only; bounces unaffected"})
        }
        "monitor.dim" => {
            app.ui.monitor.dim = p.get("value").and_then(Value::as_bool).unwrap_or(!app.ui.monitor.dim);
            json!({"dim": app.ui.monitor.dim, "attenuation_db": -20.0})
        }
        "monitor.mono" => {
            app.ui.monitor.mono = p.get("value").and_then(Value::as_bool).unwrap_or(!app.ui.monitor.mono);
            json!({"mono": app.ui.monitor.mono})
        }
        "monitor.cue" => match app.cue_path() {
            Some(path) => {
                json!({"cue": path, "control": "unavailable in this build: a dedicated cue path is detected but its separate level/mix is not implemented (reported, not relabelled)"})
            }
            None => {
                return Some(Err(
                    "headphone control unavailable: no dedicated cue output path in I/O Setup (the Main fader is never relabelled as headphones)"
                        .into(),
                ));
            }
        },
        "window.transport" => toggle(&mut app.ui.show_transport),
        "window.big_counter" => toggle(&mut app.ui.show_big_counter),
        "window.memory_locations" => toggle(&mut app.ui.show_memory_locations),
        "window.undo_history" => toggle(&mut app.ui.show_undo_history),
        "window.clip_list" | "window.clip_list_view" => toggle(&mut app.ui.show_clip_list),
        "window.track_list" => toggle(&mut app.ui.show_tracks_list),
        "window.narrow_mix" => toggle(&mut app.ui.narrow_mix),
        "window.session_info" => toggle(&mut app.ui.show_session_info),
        "window.recording" => toggle(&mut app.ui.show_recording),
        "window.audio_health" => {
            app.audio_health_report = None;
            toggle(&mut app.ui.show_audio_health)
        }
        "window.about" => toggle(&mut app.ui.show_about),
        "window.midi_editor" | "window.midi_front" => toggle(&mut app.ui.show_midi_editor),
        "window.midi_back" => {
            app.ui.show_midi_editor = false;
            json!({})
        }
        "window.automation" => toggle(&mut app.ui.show_automation),
        "window.clip_effects" | "window.clip_effects_dock" => toggle(&mut app.ui.show_clip_effects),
        "window.search" => toggle(&mut app.ui.show_search),
        "window.score" => toggle(&mut app.ui.show_score),
        "window.beat_detective" => toggle(&mut app.ui.show_beat_detective),
        "window.tempo_ops" => toggle(&mut app.ui.show_tempo_ops),
        "window.time_ops" => toggle(&mut app.ui.show_time_ops),
        "window.midi_ops" => toggle(&mut app.ui.show_midi_ops),
        "window.rtp" => toggle(&mut app.ui.show_rtp),
        "window.playback_engine" => toggle(&mut app.ui.show_playback_engine),
        "window.audio_setup" => {
            // HF06: the same window as Setup › Playback Engine…; {track?} focuses a track's input.
            if let Some(t) = p.get("track").and_then(Value::as_u64) {
                app.ui.audio_focus_track = Some(soundcraft_model::TrackId(t));
            }
            app.ui.show_playback_engine = p.get("value").and_then(Value::as_bool).unwrap_or(true);
            json!({"value": app.ui.show_playback_engine})
        }
        "window.recover" => toggle(&mut app.ui.show_recover),
        "window.io_setup" => toggle(&mut app.ui.show_io_setup),
        "window.shortcuts" => toggle(&mut app.ui.show_shortcuts),
        "window.color_palette" => toggle(&mut app.ui.show_color_palette),
        "window.disk_usage" => toggle(&mut app.ui.show_disk_usage),
        "window.system_usage" => toggle(&mut app.ui.show_system_usage),
        "window.task_manager" => toggle(&mut app.ui.show_task_manager),
        "window.metadata" => toggle(&mut app.ui.show_metadata),
        "window.event_list" => toggle(&mut app.ui.show_event_list),
        "window.midi_keyboard" => toggle(&mut app.ui.show_midi_keyboard),
        "window.workspace" | "window.workspace_presets" | "window.workspace_sounds" | "window.workspace_front" => {
            app.ui.show_workspace = true;
            json!({"value": true})
        }
        "window.workspace_close" => {
            app.ui.show_workspace = false;
            json!({"value": false})
        }
        "window.configurations" => toggle(&mut app.ui.show_configurations),
        "window.config_new" | "window.config_update" => {
            if let Err(error) = app.ui.docking.validate().and_then(|()| app.ui.mix_docking.validate_mix()) {
                return Some(Err(error));
            }
            let mut snap = app.ui.clone();
            snap.configurations.clear();
            let v = serde_json::to_value(&snap).unwrap_or_default();
            if id == "window.config_update" && !app.ui.configurations.is_empty() {
                if let Some(last) = app.ui.configurations.last_mut() {
                    last.1 = v;
                }
            } else {
                let n = app.ui.configurations.len() + 1;
                app.ui.configurations.push((format!("Configuration {n}"), v));
            }
            json!({"configurations": app.ui.configurations.len()})
        }
        "window.arrange_tile" | "window.arrange_tile_h" | "window.arrange_tile_v" | "window.arrange_cascade" => {
            if let Err(error) = crate::docking::arrange(app) {
                app.ui.status = format!("Panel windows could not be arranged: {error}");
            }
            app.arrange_request = true;
            json!({})
        }
        "window.universe" => toggle(&mut app.ui.show_universe),
        "window.video" => toggle(&mut app.ui.show_video),
        "window.video_universe" => toggle(&mut app.ui.show_video_universe),
        "window.close" | "window.hide_floating" => {
            app.ui.show_transport = false;
            app.ui.show_big_counter = false;
            app.ui.show_memory_locations = false;
            app.ui.show_undo_history = false;
            app.ui.show_session_info = false;
            app.ui.show_audio_health = false;
            app.ui.plugin_windows.clear();
            app.ui.audiosuite = None;
            json!({})
        }
        "view.mix_section" => {
            let sec = p.get("section").and_then(Value::as_str).unwrap_or("");
            match sec {
                "all" => {
                    app.ui.mix_views =
                        ["inserts_ae", "inserts_fj", "sends_ae", "sends_fj", "io", "comments", "color"].iter().map(|s| s.to_string()).collect()
                }
                "minimal" => app.ui.mix_views.clear(),
                "" => return Some(Err("`section` required".into())),
                s => {
                    if app.ui.mix_views.iter().any(|v| v == s) {
                        app.ui.mix_views.retain(|v| v != s);
                    } else {
                        app.ui.mix_views.push(s.to_string());
                    }
                }
            }
            json!({"mix_views": app.ui.mix_views})
        }
        "ui.new_tracks_dialog" => {
            return Some(Err("ui.new_tracks_dialog opens a dialog from the menu only; call track.new with parameters".into()));
        }
        "ui.bounce_dialog" => {
            return Some(Err("ui.bounce_dialog opens a dialog from the menu only; call file.bounce_mix with a path".into()));
        }
        "ui.theme_import" => {
            let text = match (p.get("text").and_then(Value::as_str), p.get("path").and_then(Value::as_str)) {
                (Some(t), _) => t.to_string(),
                (None, Some(path)) => match std::fs::read_to_string(path) {
                    Ok(t) => t,
                    Err(e) => return Some(Err(format!("cannot read {path}: {e}"))),
                },
                (None, None) => match app.services.pick_open.as_ref().and_then(|f| f("Import theme", &["json"])) {
                    Some(path) => match std::fs::read_to_string(&path) {
                        Ok(t) => t,
                        Err(e) => return Some(Err(format!("cannot read {path}: {e}"))),
                    },
                    None => return Some(Err("import cancelled".into())),
                },
            };
            match crate::theme::import(&text) {
                Ok(t) => {
                    let name = t.name.clone();
                    app.ui.theme_custom = Some(t);
                    app.ui.theme = crate::theme::ThemeMode::Custom;
                    json!({"mode": "custom", "name": name})
                }
                // The current theme stays; the reason is the error.
                Err(e) => return Some(Err(format!("theme not applied: {e}"))),
            }
        }
        "ui.theme_export" => {
            let name = p.get("name").and_then(Value::as_str).unwrap_or("SML theme");
            let data = match (&app.ui.theme, &app.ui.theme_custom) {
                (crate::theme::ThemeMode::Custom, Some(t)) => t.clone(),
                (mode, _) => crate::theme::export(
                    name,
                    if *mode == crate::theme::ThemeMode::Light {
                        crate::theme::ThemeMode::Light
                    } else if *mode == crate::theme::ThemeMode::HighContrast {
                        crate::theme::ThemeMode::HighContrast
                    } else {
                        crate::theme::ThemeMode::Dark
                    },
                    app.extra.ui_scale,
                ),
            };
            let text = serde_json::to_string_pretty(&data).unwrap_or_default();
            let path = p
                .get("path")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| app.services.pick_save.as_ref().and_then(|f| f("theme.json", "json")));
            match path {
                Some(path) => match std::fs::write(&path, text) {
                    Ok(()) => json!({"path": path}),
                    Err(e) => return Some(Err(format!("cannot write {path}: {e}"))),
                },
                None => json!({"text": text}),
            }
        }
        "ui.mix_split" => {
            // HF01: the Split view's Visual share; {fraction: 0.1..0.9} or {reset: true} / {}.
            match p.get("fraction").and_then(Value::as_f64) {
                Some(f) if (0.1..=0.9).contains(&f) => app.ui.mix_split = f as f32,
                Some(f) => return Some(Err(format!("`fraction` must be within 0.1..0.9, not {f}"))),
                None => app.ui.mix_split = crate::mix_window::SPLIT_DEFAULT,
            }
            json!({"fraction": app.ui.mix_split})
        }
        "ui.theme_reset" => {
            app.ui.theme_custom = None;
            app.ui.theme = crate::theme::ThemeMode::default();
            json!({"mode": app.ui.theme.id()})
        }
        "ui.theme_token" => {
            // Theme editor: edit one token of the Custom theme (started from the active palette)
            // and preview it live; a change that breaks validation is refused and reported.
            let Some(name) = p.get("name").and_then(Value::as_str) else { return Some(Err("`name` required".into())) };
            let rgba: Vec<u8> = p
                .get("rgba")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_u64).map(|v| v.min(255) as u8).collect())
                .unwrap_or_default();
            let scale = p.get("typography_scale").and_then(Value::as_f64).map(|v| v as f32);
            let mut t = match (&app.ui.theme, &app.ui.theme_custom) {
                (crate::theme::ThemeMode::Custom, Some(t)) => t.clone(),
                (mode, _) => {
                    let base = match mode {
                        crate::theme::ThemeMode::Light => crate::theme::ThemeMode::Light,
                        crate::theme::ThemeMode::HighContrast => crate::theme::ThemeMode::HighContrast,
                        _ => crate::theme::ThemeMode::Dark,
                    };
                    crate::theme::export("Custom", base, app.extra.ui_scale)
                }
            };
            if let Some(sc) = scale {
                t.typography_scale = sc;
            }
            if !name.is_empty() {
                let Some((_, slot)) = t.tokens.fields_mut().into_iter().find(|(n, _)| *n == name) else {
                    return Some(Err(format!("unknown token `{name}`")));
                };
                if rgba.len() != 4 {
                    return Some(Err("`rgba` must be [r, g, b, a]".into()));
                }
                *slot = egui::Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]);
            }
            if let Err(e) = crate::theme::validate(&t) {
                return Some(Err(format!("not applied: {e}")));
            }
            app.ui.theme_custom = Some(t);
            app.ui.theme = crate::theme::ThemeMode::Custom;
            json!({"mode": "custom", "token": name})
        }
        "window.theme_editor" => {
            app.extra.show_theme_editor = !app.extra.show_theme_editor;
            json!({"value": app.extra.show_theme_editor})
        }
        "ui.shortcut_profile" => {
            use crate::shortcut_profiles::Profile;
            if let Some(v) = p.get("profile").and_then(Value::as_str) {
                let Some(prof) = Profile::parse(v) else { return Some(Err(format!("unknown profile `{v}` (pro_tools, studio_one, custom)"))) };
                app.ui.shortcut_profile = prof;
            }
            if let Some(v) = p.get("base").and_then(Value::as_str) {
                let Some(base) = Profile::parse(v).filter(|b| *b != Profile::Custom) else {
                    return Some(Err(format!("unknown base `{v}` (pro_tools or studio_one)")));
                };
                app.ui.shortcut_base = base;
            }
            app.shortcut_capture = None;
            json!({"profile": app.ui.shortcut_profile.id(), "base": app.ui.shortcut_base.id(), "bindings": app.shortcut_table().len()})
        }
        "ui.shortcut_set" => {
            // Reassign in the Custom profile (switching to it when another profile is active, so
            // the built-in tables are never edited in place).
            let key = match (p.get("key").and_then(Value::as_str), p.get("command").and_then(Value::as_str)) {
                (Some(k), _) => k.to_string(),
                (None, Some(c)) => match p.get("params") {
                    Some(v) if !v.is_null() => format!("{c}|{v}"),
                    _ => c.to_string(),
                },
                (None, None) => return Some(Err("`key` or `command` required".into())),
            };
            let chord = p.get("chord").and_then(Value::as_str).unwrap_or("").to_string();
            if !chord.is_empty() && crate::shortcuts::parse(&chord).is_none() {
                return Some(Err(format!("`{chord}` is not a chord this build can bind")));
            }
            let command = key.split_once('|').map_or(key.as_str(), |(c, _)| c).to_string();
            if soundcraft_engine::find_command(&command).is_none() && !UI_COMMANDS.iter().any(|c| c.0 == command) {
                return Some(Err(format!("unknown command `{command}`")));
            }
            if app.ui.shortcut_profile != crate::shortcut_profiles::Profile::Custom {
                app.ui.shortcut_base = app.ui.shortcut_profile;
                app.ui.shortcut_profile = crate::shortcut_profiles::Profile::Custom;
            }
            app.ui.shortcut_overrides.insert(key.clone(), chord.clone());
            let conflicts = crate::shortcut_profiles::conflicts(&app.shortcut_table()).into_iter().filter(|c| c.chord == chord).collect::<Vec<_>>();
            json!({"key": key, "chord": chord, "conflicts": conflicts})
        }
        "ui.shortcuts_reset" => {
            app.ui.shortcut_overrides.clear();
            app.shortcut_capture = None;
            json!({"profile": app.ui.shortcut_profile.id()})
        }
        "ui.shortcuts_conflicts" => {
            json!({"profile": app.ui.shortcut_profile.id(), "conflicts": crate::shortcut_profiles::conflicts(&app.shortcut_table())})
        }
        "ui.shortcuts_export" => {
            let text = crate::shortcut_profiles::export(app.ui.shortcut_base, &app.ui.shortcut_overrides);
            let path = p
                .get("path")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| app.services.pick_save.as_ref().and_then(|f| f("shortcuts.json", "json")));
            match path {
                Some(path) => match std::fs::write(&path, text) {
                    Ok(()) => json!({"path": path}),
                    Err(e) => return Some(Err(format!("cannot write {path}: {e}"))),
                },
                None => json!({"text": text}),
            }
        }
        "ui.shortcuts_import" => {
            let text = match (p.get("text").and_then(Value::as_str), p.get("path").and_then(Value::as_str)) {
                (Some(t), _) => t.to_string(),
                (None, Some(path)) => match std::fs::read_to_string(path) {
                    Ok(t) => t,
                    Err(e) => return Some(Err(format!("cannot read {path}: {e}"))),
                },
                (None, None) => match app.services.pick_open.as_ref().and_then(|f| f("Import shortcuts", &["json"])) {
                    Some(path) => match std::fs::read_to_string(&path) {
                        Ok(t) => t,
                        Err(e) => return Some(Err(format!("cannot read {path}: {e}"))),
                    },
                    None => return Some(Err("import cancelled".into())),
                },
            };
            match crate::shortcut_profiles::import(&text) {
                Ok(f) => {
                    app.ui.shortcut_base = f.base;
                    app.ui.shortcut_overrides = f.overrides;
                    app.ui.shortcut_profile = crate::shortcut_profiles::Profile::Custom;
                    json!({"base": f.base.id(), "overrides": app.ui.shortcut_overrides.len()})
                }
                Err(e) => return Some(Err(e)),
            }
        }
        "ui.theme" => {
            let Some(mode) = p.get("mode").and_then(Value::as_str) else {
                return Some(Ok(json!({"mode": app.ui.theme.id()})));
            };
            let Some(mode) = crate::theme::ThemeMode::parse(mode) else {
                return Some(Err(format!("unknown theme `{mode}` (system, light, dark, high_contrast or custom)")));
            };
            app.ui.theme = mode;
            json!({"mode": mode.id()})
        }
        "ui.set" => match serde_json::to_value(&app.ui) {
            Ok(mut cur) => {
                if let (Some(obj), Some(patch)) = (cur.as_object_mut(), p.as_object()) {
                    for (k, v) in patch {
                        obj.insert(k.clone(), v.clone());
                    }
                }
                match serde_json::from_value::<crate::UiState>(cur) {
                    Ok(ns) => {
                        if let Err(error) = ns.docking.validate().and_then(|()| ns.mix_docking.validate_mix()) {
                            return Some(Err(error));
                        }
                        crate::docking::cancel_replaced_state(app, &ns);
                        app.ui = ns;
                        json!({"ui": app.ui})
                    }
                    Err(e) => return Some(Err(format!("bad ui state: {e}"))),
                }
            }
            Err(e) => return Some(Err(e.to_string())),
        },
        _ => return None,
    };
    Some(Ok(r))
}

/// Parity including UI-layer commands.
pub fn parity() -> Value {
    let extra = ui_aliases();
    catalog::parity_with(&extra)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Services, SoundApp};

    #[test]
    fn mix_view_aliases_are_allocated_once() {
        let first = ui_aliases();
        let second = ui_aliases();
        let a = first.iter().find(|(path, _)| path.contains("Mix Window Views > All")).map(|(path, _)| *path).unwrap();
        let b = second.iter().find(|(path, _)| path.contains("Mix Window Views > All")).map(|(path, _)| *path).unwrap();
        assert!(std::ptr::eq(a, b));
        assert_eq!(first.iter().filter(|(path, _)| path.contains("Mix Window Views")).count(), 14);
    }

    #[test]
    fn programmatic_dialog_commands_stay_closed_and_the_menu_opens_them() {
        let mut app = SoundApp::new(soundcraft_engine::Engine::default(), None, Services::default());
        let err = app.run("ui.new_tracks_dialog", json!({})).unwrap_err();
        assert!(err.contains("track.new"), "{err}");
        let err = app.run("ui.bounce_dialog", json!({})).unwrap_err();
        assert!(err.contains("file.bounce_mix"), "{err}");
        assert!(app.dialogs.open_name().is_none());

        invoke_menu(&mut app, "ui.new_tracks_dialog", "Track > New...");
        assert_eq!(app.dialogs.open_name(), Some("new_tracks"));
        app.dialogs.open = None;
        invoke_menu(&mut app, "ui.bounce_dialog", "File > Bounce Mix...");
        assert_eq!(app.dialogs.open_name(), Some("bounce"));
    }

    /// RC03: a session saved with the legacy QuickPunch/TrackPunch/destructive flags keeps them,
    /// but the menus show those modes disabled and unchecked (they do nothing in this build).
    #[test]
    fn legacy_record_mode_flags_are_never_shown_as_active() {
        let mut engine = soundcraft_engine::Engine::default();
        engine.session_mut().edit.quickpunch = true;
        engine.session_mut().edit.record_mode = "trackpunch".into();
        let app = SoundApp::new(engine, None, Services::default());
        for id in ["options.quickpunch", "options.trackpunch", "options.destructive_record", "options.destructive_punch"] {
            assert_eq!(item_state(&app, "Options > X", Some(id)), (false, false), "{id}");
        }
        assert!(app.engine.session().edit.quickpunch, "the legacy flag itself is kept");
    }
}
