//! Shortcut profiles (R13, D10): separate Pro Tools-style and Studio One-style tables plus a
//! Custom profile (a base table with per-command overrides). A profile switch is explicit and
//! reversible; conflicting maps are never merged automatically. The resolver turns the active
//! table into one `(modifiers, key) → (command, params)` map; `shortcuts.rs` consults it before
//! its context keys. The conflict inspector lists duplicate chords, chords the operating system
//! reserves (shown with their alternate, never hooked globally) and chords this build cannot
//! deliver (keypad location and the Windows/Super key: egui-winit 0.36, see B03).
//!
//! Provenance: the Pro Tools table is the command registry's own `shortcut` strings (carried
//! from the incumbent's menu catalog) plus the context keys documented in `shortcuts.rs`. The
//! Studio One table is a best-known default set; the reference manuals (S03/S05) were not
//! reachable from the build environment, so every Studio One row is reported `pending native
//! verification` in `docs/evidence/next_cycle/sprint_02/shortcut-inventory.csv` until checked.

use egui::{Key, Modifiers};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    #[default]
    ProTools,
    StudioOne,
    Custom,
}

impl Profile {
    pub const ALL: [Profile; 3] = [Profile::ProTools, Profile::StudioOne, Profile::Custom];

    pub fn id(self) -> &'static str {
        match self {
            Profile::ProTools => "pro_tools",
            Profile::StudioOne => "studio_one",
            Profile::Custom => "custom",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Profile::ProTools => "Pro Tools style",
            Profile::StudioOne => "Studio One style",
            Profile::Custom => "Custom",
        }
    }

    pub fn parse(s: &str) -> Option<Profile> {
        Profile::ALL.into_iter().find(|p| p.id() == s.trim().to_ascii_lowercase())
    }
}

/// One table row: a chord bound to a command (with fixed parameters for parameterized commands
/// such as `edit.tool`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Entry {
    pub chord: String,
    pub command: String,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub params: Value,
    pub label: String,
    /// `registry`, `context` (handled by the context-key handler), `studio_one`, `override`.
    pub source: &'static str,
    /// Whether the binding is confirmed against its reference manual (Studio One rows are not).
    pub verified: bool,
}

impl Entry {
    /// The override key: `command` or `command|{"tool":"arrow"}` for parameterized rows.
    pub fn key(&self) -> String {
        if self.params.is_null() { self.command.clone() } else { format!("{}|{}", self.command, self.params) }
    }
}

/// Studio One-style defaults: (chord, command, params, label). Bindings that need a keypad key
/// this build cannot tell from the top row (Num0, Num*, NumEnter, Num/, Num,) are deliberately
/// absent (B03) and listed in the inventory as unsupported.
pub const STUDIO_ONE: &[(&str, &str, &str, &str)] = &[
    ("1", "edit.tool", r#"{"tool":"arrow"}"#, "Arrow tool"),
    ("2", "edit.tool", r#"{"tool":"range"}"#, "Range tool"),
    ("3", "edit.tool", r#"{"tool":"split"}"#, "Split tool"),
    ("4", "edit.tool", r#"{"tool":"eraser"}"#, "Eraser tool"),
    ("5", "edit.tool", r#"{"tool":"paint"}"#, "Paint tool"),
    ("6", "edit.tool", r#"{"tool":"mute"}"#, "Mute tool"),
    ("7", "edit.tool", r#"{"tool":"bend"}"#, "Bend tool"),
    ("8", "edit.tool", r#"{"tool":"listen"}"#, "Listen tool"),
    ("Space", "transport.toggle", "", "Start/Stop"),
    ("L", "edit.selection_to_timeline", "", "Set loop range to selection"),
    ("Insert", "markers.add", "", "Insert marker"),
    ("Cmd+Z", "edit.undo", "", "Undo"),
    ("Cmd+Y", "edit.redo", "", "Redo"),
    ("Cmd+X", "edit.cut", "", "Cut"),
    ("Cmd+C", "edit.copy", "", "Copy"),
    ("Cmd+V", "edit.paste", "", "Paste"),
    ("Delete", "edit.clear", "", "Delete"),
    ("D", "edit.duplicate", "", "Duplicate"),
    ("Cmd+A", "edit.select_all", "", "Select all"),
    ("Cmd+Shift+A", "edit.select_none", "", "Deselect all"),
    ("Alt+X", "edit.split_at", "", "Split at cursor"),
    ("Shift+M", "edit.mute_clips", "", "Mute events"),
    ("N", "view.grid", "", "Snap on/off"),
    ("Q", "clip.quantize_to_grid", "", "Quantize"),
    ("E", "view.zoom_in", "", "Zoom in"),
    ("W", "view.zoom_out", "", "Zoom out"),
    ("Shift+S", "view.zoom_to_selection", "", "Zoom to selection"),
    ("Alt+Right", "nudge.clips", r#"{"direction":1}"#, "Nudge right"),
    ("Alt+Left", "nudge.clips", r#"{"direction":-1}"#, "Nudge left"),
    ("M", "mix.mute", "", "Mute selected tracks"),
    ("S", "mix.solo", "", "Solo selected tracks"),
    ("R", "mix.record_arm", "", "Record-arm selected tracks"),
    ("T", "track.new", "", "Add tracks"),
    ("F3", "window.toggle_mix_edit", "", "Console (mix) view"),
    ("F2", "window.edit", "", "Edit view"),
    ("Cmd+S", "session.save", "", "Save"),
    ("Cmd+Shift+S", "session.save_as", "", "Save as"),
    ("Cmd+N", "session.new", "", "New song"),
    ("Cmd+O", "session.open", "", "Open"),
    ("Cmd+W", "session.close", "", "Close"),
    ("Cmd+Q", "app.quit", "", "Quit"),
    ("Cmd+E", "file.bounce_mix", "", "Export mixdown"),
    ("Cmd+B", "edit.consolidate", "", "Bounce selection"),
];

/// Pro Tools-style context keys handled by `shortcuts::fixed` (listed here for the inspector
/// and the inventory; the registry rows are added at runtime).
pub const PRO_TOOLS_CONTEXT: &[(&str, &str, &str, &str)] = &[
    ("Space", "transport.toggle", "", "Play/Stop"),
    ("Cmd+Space", "transport.record", "", "Record"),
    ("Shift+Space", "transport.half_speed", "", "Half-speed"),
    ("F1", "edit.mode", r#"{"mode":"shuffle"}"#, "Shuffle mode"),
    ("F2", "edit.mode", r#"{"mode":"slip"}"#, "Slip mode"),
    ("F3", "edit.mode", r#"{"mode":"spot"}"#, "Spot mode"),
    ("F4", "edit.mode", r#"{"mode":"grid"}"#, "Grid mode"),
    ("F5", "edit.tool", r#"{"tool":"zoom"}"#, "Zoomer"),
    ("F6", "edit.tool", r#"{"tool":"trim"}"#, "Trim"),
    ("F7", "edit.tool", r#"{"tool":"selector"}"#, "Selector"),
    ("F8", "edit.tool", r#"{"tool":"grabber"}"#, "Grabber"),
    ("F9", "edit.tool", r#"{"tool":"scrubber"}"#, "Scrubber"),
    ("F10", "edit.tool", r#"{"tool":"pencil"}"#, "Pencil"),
    ("Enter", "markers.add", "", "New memory location"),
    ("Cmd+=", "window.toggle_mix_edit", "", "Toggle Mix/Edit"),
    ("Tab", "transport.locate", "", "Tab to clip boundary"),
    ("Ctrl+Up", "edit.extend_selection_up", "", "Extend selection up"),
    ("Ctrl+Down", "edit.extend_selection_down", "", "Extend selection down"),
    ("Ctrl+Shift+Up", "clip.gain_nudge_up", "", "Nudge clip gain up"),
    ("Ctrl+Shift+Down", "clip.gain_nudge_down", "", "Nudge clip gain down"),
    ("T", "view.zoom_in", "", "Zoom in (commands focus)"),
    ("R", "view.zoom_out", "", "Zoom out (commands focus)"),
];

/// Chords the operating systems reserve: never hooked globally; the inspector names the alternate.
pub const OS_RESERVED: &[(&str, &str)] = &[
    ("Alt+F4", "Windows closes the window"),
    ("Alt+Tab", "Windows/Linux switch windows"),
    ("Cmd+Tab", "macOS switches apps"),
    ("Cmd+Space", "macOS Spotlight (Record is also in the Transport)"),
    ("Cmd+H", "macOS hides the app"),
    ("Cmd+Q", "macOS quits the app"),
    ("Ctrl+Alt+Delete", "Windows security screen"),
    ("Cmd+Ctrl+Q", "macOS locks the screen"),
];

fn table_from(rows: &[(&str, &str, &str, &str)], source: &'static str, verified: bool) -> Vec<Entry> {
    rows.iter()
        .map(|(chord, cmd, params, label)| Entry {
            chord: (*chord).to_string(),
            command: (*cmd).to_string(),
            params: if params.is_empty() { Value::Null } else { serde_json::from_str(params).unwrap_or(Value::Null) },
            label: (*label).to_string(),
            source,
            verified,
        })
        .collect()
}

/// The Pro Tools-style table: registry shortcut strings (parseable ones) plus the context keys.
pub fn pro_tools_table() -> Vec<Entry> {
    let mut v: Vec<Entry> = soundcraft_engine::command_specs()
        .iter()
        .filter_map(|c| {
            let sc = c.shortcut?;
            crate::shortcuts::parse(sc)?;
            Some(Entry {
                chord: sc.to_string(),
                command: c.id.to_string(),
                params: Value::Null,
                label: c.label.to_string(),
                source: "registry",
                verified: true,
            })
        })
        .collect();
    for (id, label, _, sc) in crate::menus::UI_COMMANDS {
        if let Some(sc) = sc
            && crate::shortcuts::parse(sc).is_some()
        {
            v.push(Entry {
                chord: (*sc).to_string(),
                command: (*id).to_string(),
                params: Value::Null,
                label: (*label).to_string(),
                source: "registry",
                verified: true,
            });
        }
    }
    v.extend(table_from(PRO_TOOLS_CONTEXT, "context", true));
    v
}

pub fn studio_one_table() -> Vec<Entry> {
    table_from(STUDIO_ONE, "studio_one", false)
}

/// The table a profile resolves to. Custom = `base` with `overrides` (key → chord; an empty
/// chord unbinds; a key the base lacks adds a row for a registry command).
pub fn table(profile: Profile, base: Profile, overrides: &BTreeMap<String, String>) -> Vec<Entry> {
    let mut v = match profile {
        Profile::ProTools => pro_tools_table(),
        Profile::StudioOne => studio_one_table(),
        Profile::Custom => match base {
            Profile::StudioOne => studio_one_table(),
            _ => pro_tools_table(),
        },
    };
    if profile != Profile::Custom {
        return v;
    }
    for (key, chord) in overrides {
        let (command, params) = match key.split_once('|') {
            Some((c, p)) => (c.to_string(), serde_json::from_str(p).unwrap_or(Value::Null)),
            None => (key.clone(), Value::Null),
        };
        if let Some(e) = v.iter_mut().find(|e| e.key() == *key) {
            e.chord = chord.clone();
            e.source = "override";
        } else if !chord.is_empty() {
            let label = soundcraft_engine::find_command(&command).map(|c| c.label.to_string()).unwrap_or_else(|| command.clone());
            v.push(Entry { chord: chord.clone(), command, params, label, source: "override", verified: false });
        }
    }
    v.retain(|e| !e.chord.is_empty());
    v
}

/// A resolved row: parsed chord plus what it runs.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub mods: Modifiers,
    pub key: Key,
    pub command: String,
    pub params: Value,
}

/// The resolver: every parseable chord of the table in table order (the first row with a chord
/// wins, as the inspector states). Context rows that `shortcuts::fixed` also handles are included
/// so lookups and the inspector agree.
pub fn resolve(table: &[Entry]) -> Vec<Resolved> {
    let mut seen = HashMap::new();
    let mut v = Vec::new();
    for e in table {
        if let Some((mods, key)) = crate::shortcuts::parse(&e.chord)
            && seen.insert((mods, key), ()).is_none()
        {
            v.push(Resolved { mods, key, command: e.command.clone(), params: e.params.clone() });
        }
    }
    v
}

/// Look a key event up (modifier matching follows `shortcuts::mods_match`: Ctrl is Cmd off macOS).
pub fn lookup(rows: &[Resolved], key: Key, mods: Modifiers, mac: bool) -> Option<&Resolved> {
    rows.iter().find(|r| r.key == key && crate::shortcuts::mods_match(r.mods, mods, mac))
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum ConflictKind {
    /// Two or more rows share one chord: the first wins, the others are dead until reassigned.
    Duplicate,
    /// The operating system takes the chord before the app sees it.
    OsReserved,
    /// This build cannot deliver the chord (keypad location, Windows/Super key).
    Undeliverable,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Conflict {
    pub chord: String,
    pub kind: ConflictKind,
    pub commands: Vec<String>,
    pub note: String,
}

/// The conflict inspector.
pub fn conflicts(table: &[Entry]) -> Vec<Conflict> {
    let mut by_chord: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for e in table {
        if e.chord.is_empty() {
            continue;
        }
        by_chord.entry(e.chord.clone()).or_default().push(format!("{} ({})", e.label, e.key()));
    }
    let mut out = Vec::new();
    for (chord, cmds) in &by_chord {
        if cmds.len() > 1 {
            out.push(Conflict {
                chord: chord.clone(),
                kind: ConflictKind::Duplicate,
                commands: cmds.clone(),
                note: "the first row wins; reassign the others".into(),
            });
        }
        if let Some((_, why)) = OS_RESERVED.iter().find(|(c, _)| c == chord) {
            out.push(Conflict {
                chord: chord.clone(),
                kind: ConflictKind::OsReserved,
                commands: cmds.clone(),
                note: format!("{why}; use the menu or search, or reassign"),
            });
        }
        if chord.contains("Num") || chord.contains("Win+") || chord.contains("Super+") {
            out.push(Conflict {
                chord: chord.clone(),
                kind: ConflictKind::Undeliverable,
                commands: cmds.clone(),
                note: "egui-winit 0.36 folds the keypad into the top row and drops the Windows key (B03): reachable from the Nudge panel and search"
                    .into(),
            });
        }
    }
    out
}

/// Export/import of a Custom profile's overrides (JSON).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProfileFile {
    pub format: String,
    pub base: Profile,
    pub overrides: BTreeMap<String, String>,
}

pub const FORMAT: &str = "sml-shortcuts-1";

pub fn export(base: Profile, overrides: &BTreeMap<String, String>) -> String {
    serde_json::to_string_pretty(&ProfileFile { format: FORMAT.into(), base, overrides: overrides.clone() }).unwrap_or_default()
}

/// Validate an imported file: known format, every chord parseable, every command known.
pub fn import(text: &str) -> Result<ProfileFile, String> {
    let f: ProfileFile = serde_json::from_str(text).map_err(|e| format!("not a shortcut file: {e}"))?;
    if f.format != FORMAT {
        return Err(format!("unsupported shortcut file format `{}` (expected {FORMAT})", f.format));
    }
    for (key, chord) in &f.overrides {
        let command = key.split_once('|').map_or(key.as_str(), |(c, _)| c);
        if soundcraft_engine::find_command(command).is_none() && !crate::menus::UI_COMMANDS.iter().any(|c| c.0 == command) {
            return Err(format!("unknown command `{command}`"));
        }
        if !chord.is_empty() && crate::shortcuts::parse(chord).is_none() {
            return Err(format!("`{chord}` is not a chord this build can bind"));
        }
    }
    Ok(f)
}

/// A chord string for a key event (the inverse of `shortcuts::parse`, for capture/reassign).
pub fn chord_of(key: Key, m: Modifiers, mac: bool) -> String {
    let mut parts = Vec::new();
    if m.command || (!mac && m.ctrl) {
        parts.push("Cmd");
    }
    if mac && m.ctrl {
        parts.push("Ctrl");
    }
    if m.shift {
        parts.push("Shift");
    }
    if m.alt {
        parts.push("Alt");
    }
    let name = match key {
        Key::OpenBracket => "[",
        Key::CloseBracket => "]",
        Key::Comma => ",",
        Key::Period => ".",
        Key::Slash => "/",
        Key::Quote => "'",
        Key::Equals => "=",
        k => k.name(),
    };
    parts.push(name);
    parts.join("+")
}

/// Rows of the inventory ledger for the active table (CSV columns match
/// `docs/evidence/next_cycle/sprint_02/shortcut-inventory.csv`).
pub fn inventory_rows(profile: Profile, table: &[Entry]) -> Vec<Vec<String>> {
    table
        .iter()
        .map(|e| {
            let handler = soundcraft_engine::find_command(&e.command).is_some() || crate::menus::UI_COMMANDS.iter().any(|c| c.0 == e.command);
            let status = if !handler {
                "unsupported"
            } else if e.verified {
                "implemented-and-mapped"
            } else {
                "pending native verification"
            };
            vec![profile.id().to_string(), e.label.clone(), e.chord.clone(), e.source.to_string(), e.key(), e.chord.clone(), status.to_string()]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R13: every row of both profiles names a command with a real handler (no blank mappings),
    /// profiles are distinct, Custom overrides apply and can unbind, round-trip through the file.
    #[test]
    fn profiles_are_distinct_and_every_row_has_a_handler() {
        let pt = pro_tools_table();
        let s1 = studio_one_table();
        for e in pt.iter().chain(&s1) {
            assert!(
                soundcraft_engine::find_command(&e.command).is_some() || crate::menus::UI_COMMANDS.iter().any(|c| c.0 == e.command),
                "{} → {} has no handler",
                e.chord,
                e.command
            );
            assert!(crate::shortcuts::parse(&e.chord).is_some(), "{} is not a bindable chord", e.chord);
        }
        assert!(s1.iter().any(|e| e.chord == "1" && e.params["tool"] == "arrow"));
        assert!(!pt.iter().any(|e| e.chord == "1"), "the Pro Tools table does not bind the top-row digits to tools");
        // Distinct meaning for one chord: F3 is Spot mode in one and the mixer in the other.
        let f3 = |t: &[Entry]| t.iter().find(|e| e.chord == "F3").map(|e| e.command.clone());
        assert_ne!(f3(&pt), f3(&s1));
        // The keypad Nudge preset is not overwritten by the Studio One table.
        assert!(!s1.iter().any(|e| e.chord.contains("Num")));
        // Custom: an override rebinds and an empty chord unbinds.
        let mut ov = BTreeMap::new();
        ov.insert(r#"edit.tool|{"tool":"arrow"}"#.to_string(), "Cmd+1".to_string());
        ov.insert("edit.undo".to_string(), String::new());
        ov.insert("edit.heal".to_string(), "Cmd+Shift+H".to_string());
        let custom = table(Profile::Custom, Profile::StudioOne, &ov);
        assert!(custom.iter().any(|e| e.chord == "Cmd+1" && e.params["tool"] == "arrow"));
        assert!(!custom.iter().any(|e| e.command == "edit.undo"));
        assert!(custom.iter().any(|e| e.command == "edit.heal" && e.chord == "Cmd+Shift+H"));
        let rows = resolve(&custom);
        let pc_ctrl = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
        assert_eq!(lookup(&rows, Key::Num1, pc_ctrl, false).map(|x| x.command.as_str()), Some("edit.tool"));
        assert!(lookup(&rows, Key::Num1, Modifiers::NONE, false).is_none(), "the override replaced the plain 1");
        let text = export(Profile::StudioOne, &ov);
        let back = import(&text).unwrap();
        assert_eq!(back.overrides, ov);
        assert!(import("{\"format\":\"other\"}").is_err());
        assert!(import(r#"{"format":"sml-shortcuts-1","base":"pro_tools","overrides":{"no.such":"Cmd+1"}}"#).is_err());
        assert!(import(r#"{"format":"sml-shortcuts-1","base":"pro_tools","overrides":{"edit.undo":"Hyper+Z"}}"#).is_err());
    }

    /// Writes the inventory ledger from the real tables when `SML_WRITE_INVENTORY` names a path
    /// (evidence generation, not an assertion about the product).
    #[test]
    fn inventory_ledger_matches_the_tables() {
        let mut rows = inventory_rows(Profile::ProTools, &pro_tools_table());
        rows.extend(inventory_rows(Profile::StudioOne, &studio_one_table()));
        assert!(rows.iter().all(|r| r[6] != "unsupported"), "every table row has a handler");
        if let Some(path) = std::env::var_os("SML_WRITE_INVENTORY") {
            let mut out = String::from("profile,reference_command,reference_binding,source,sml_command,sml_binding,status,reason\n");
            let esc = |v: &str| if v.contains(',') || v.contains('"') { format!("\"{}\"", v.replace('"', "\"\"")) } else { v.to_string() };
            for r in rows {
                let reason = match (r[0].as_str(), r[6].as_str()) {
                    ("pro_tools", _) => {
                        "registry/context chord carried from the incumbent catalog; S02 PDF unreachable in P (proxy 403); keyboard delivery is a native gate"
                    }
                    (_, "pending native verification") => {
                        "best-known Studio One default; manual S03/S05 unreachable in P (proxy 403); verify on the target machine"
                    }
                    _ => "",
                };
                out.push_str(&format!("{}\n", r.iter().map(|v| esc(v)).chain(std::iter::once(esc(reason))).collect::<Vec<_>>().join(",")));
            }
            // Rows this build cannot bind, reported as such (not silently dropped).
            for (label, chord, why) in [
                ("Stop", "Num0", "keypad location not delivered by egui-winit 0.36 (B03)"),
                ("Start", "NumEnter", "keypad location not delivered by egui-winit 0.36 (B03)"),
                ("Record", "Num*", "keypad location not delivered by egui-winit 0.36 (B03)"),
                ("Return to zero", "Num,", "keypad location not delivered by egui-winit 0.36 (B03)"),
                ("Loop active", "Num/", "keypad location not delivered by egui-winit 0.36 (B03)"),
                ("Melodyne/ARA editing", "n/a", "no ARA/note editor exists in this build (V12 BLOCKED)"),
                ("Chord/Arranger track, instrument tools", "n/a", "outside the recording/mix/master scope (contract)"),
                ("Content nudge", "Win+Num+/-", "Windows key dropped by egui-winit 0.36 (B03); reachable from the Nudge panel"),
            ] {
                let status = if chord == "n/a" { "out-of-scope" } else { "unsupported" };
                out.push_str(&format!(
                    "{},{},{},reference,,,{},{}\n",
                    if label.starts_with("Content") { "pro_tools" } else { "studio_one" },
                    esc(label),
                    esc(chord),
                    status,
                    esc(why)
                ));
            }
            std::fs::write(path, out).unwrap();
        }
    }

    #[test]
    fn inspector_reports_duplicates_os_and_undeliverable_chords() {
        let mut ov = BTreeMap::new();
        ov.insert("edit.heal".to_string(), "Cmd+Z".to_string());
        ov.insert("edit.repeat".to_string(), "Alt+F4".to_string());
        ov.insert("nudge.contents".to_string(), "Win+Num+".to_string());
        let t = table(Profile::Custom, Profile::ProTools, &ov);
        let c = conflicts(&t);
        assert!(c.iter().any(|x| x.chord == "Cmd+Z" && x.kind == ConflictKind::Duplicate && x.commands.len() == 2), "{c:?}");
        assert!(c.iter().any(|x| x.chord == "Alt+F4" && x.kind == ConflictKind::OsReserved));
        assert!(c.iter().any(|x| x.chord == "Win+Num+" && x.kind == ConflictKind::Undeliverable));
        assert!(conflicts(&studio_one_table()).iter().all(|x| x.kind != ConflictKind::Duplicate), "{:?}", conflicts(&studio_one_table()));
        assert_eq!(chord_of(Key::S, Modifiers { ctrl: true, command: true, ..Modifiers::NONE }, false), "Cmd+S");
        assert_eq!(chord_of(Key::Num1, Modifiers::NONE, false), "1");
        assert_eq!(chord_of(Key::OpenBracket, Modifiers { command: true, mac_cmd: true, alt: true, ..Modifiers::NONE }, true), "Cmd+Alt+[");
    }
}
