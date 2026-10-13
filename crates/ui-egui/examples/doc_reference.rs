//! Documentation reference generator and checker (docs/DOCS_MAINTENANCE.md).
//!
//! `cargo run -q -p soundcraft-ui-egui --example doc_reference -- --write [ROOT]` regenerates,
//! from this build's real metadata only, the files listed in `GENERATED` below: every engine
//! command (id, label, menu, chords per shortcut profile, parameters, undo, risk, status, stated
//! limits), the UI-layer commands, both shortcut profiles, the control-channel methods and MCP
//! tools (read from their source files), and the stated limits.
//!
//! `--check [ROOT]` (run by `cargo xtask docs-check` and `cargo xtask ci`) fails when:
//! the generated files differ from what this build would write; a document listed in
//! `docs/DOCS_MANIFEST.json` is missing, has a broken relative link or anchor, or names a command
//! / method / MCP tool in backticks that this build does not have; the manifest's chapter map
//! names an unknown command; the manifest or the manual names a different product version. It
//! also reports (without failing) chapters whose mapped source files changed since the commit the
//! documentation was last verified against.

use serde_json::{Value, json};
use soundcraft_ui_egui::shortcut_profiles::{self, Entry};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const SCHEMA: &str = "sml-doc-reference-1";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const GENERATED: &[&str] =
    &["docs/COMMAND_REFERENCE.md", "docs/reference/commands.json", "docs/reference/SHORTCUTS.md", "docs/reference/CAPABILITIES.md"];
/// Last path segments that mark a file name, not a command id (`ui.json`, `song.scraft`).
const FILE_EXT: &[&str] = &[
    "json",
    "md",
    "wav",
    "aif",
    "aiff",
    "flac",
    "scraft",
    "csv",
    "txt",
    "toml",
    "zip",
    "exe",
    "log",
    "png",
    "rs",
    "sh",
    "ps1",
    "gz",
    "msi",
    "mp3",
    "html",
    "pdf",
    "yml",
    "jsonl",
    "tmp",
    "dll",
    "so",
    "vst3",
    "clap",
    "component",
    "py",
    "zsync",
    "deb",
    "rpm",
    "appimage",
    "sha256",
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.first().map(String::as_str).unwrap_or("--check");
    let root = args.get(1).map_or_else(|| PathBuf::from("."), PathBuf::from);
    let files = generate(&root);
    let code = match mode {
        "--write" => {
            for (path, text) in &files {
                let p = root.join(path);
                if let Some(d) = p.parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                if let Err(e) = std::fs::write(&p, text) {
                    eprintln!("{}: {e}", p.display());
                    std::process::exit(1);
                }
                println!("wrote {path}");
            }
            0
        }
        "--check" => check(&root, &files),
        _ => {
            eprintln!("usage: doc_reference --write|--check [ROOT]");
            2
        }
    };
    std::process::exit(code);
}

/// Engine knowledge rows plus per-profile chords (Windows/Linux key names).
struct Row {
    k: Value,
    pro_tools: Vec<String>,
    studio_one: Vec<String>,
}

/// Who actually receives a chord: the app runs the first row of the table for each chord
/// (`shortcut_profiles::resolve`), so a later row with the same chord never fires.
fn winner(table: &[Entry], e: &Entry) -> Option<String> {
    let me = soundcraft_ui_egui::shortcuts::parse(&e.chord)?;
    let first = table.iter().find(|o| soundcraft_ui_egui::shortcuts::parse(&o.chord) == Some(me))?;
    (first.command != e.command || first.params != e.params)
        .then(|| if first.params.is_null() { first.command.clone() } else { format!("{} {}", first.command, first.params) })
}

fn chords(table: &[Entry]) -> BTreeMap<String, Vec<String>> {
    let mut m: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for e in table {
        let shown = soundcraft_ui_egui::shortcuts::shortcut_label(&e.chord, false);
        if shown.is_empty() {
            continue;
        }
        let mut s = shown;
        if !e.params.is_null() {
            s.push_str(&format!(" ({})", e.params));
        }
        if let Some(w) = winner(table, e) {
            s.push_str(&format!(" [shadowed: runs {w}]"));
        }
        if !e.verified {
            s.push_str(" [unverified]");
        }
        let v = m.entry(e.command.clone()).or_default();
        if !v.contains(&s) {
            v.push(s);
        }
    }
    m
}

/// Names a source file defines between quotes before `=>` (match arms), e.g. control methods.
fn match_arm_names(src: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in src.lines() {
        let t = line.trim_start();
        if !t.starts_with('"') || !t.contains("=>") {
            continue;
        }
        let head = t.split("=>").next().unwrap_or("");
        for part in head.split('|') {
            let s = part.trim().trim_matches('"');
            // Dotted names only: the same file also matches key names ("tab", "esc").
            if s.contains('.') && s.chars().all(|c| c.is_ascii_lowercase() || c == '.' || c == '_') {
                out.insert(s.to_string());
            }
        }
    }
    out
}

/// MCP tool names: the first string literal after each `tool(` in the definitions.
fn mcp_tools(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    // Only inside the definitions (the helper `fn tool(name, …)` itself is above them).
    let mut rest = src.find("fn tool_definitions").map_or("", |i| &src[i..]);
    while let Some(i) = rest.find("tool(") {
        rest = &rest[i + 5..];
        let Some(q) = rest.find('"') else { break };
        let after = &rest[q + 1..];
        let Some(e) = after.find('"') else { break };
        let name = &after[..e];
        if !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c == '_') && !out.iter().any(|n| n == name) {
            out.push(name.to_string());
        }
    }
    out
}

fn esc(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn generate(root: &Path) -> Vec<(String, String)> {
    let e = soundcraft_engine::Engine::default();
    let pt = chords(&shortcut_profiles::pro_tools_table());
    let s1 = chords(&shortcut_profiles::studio_one_table());
    let mut rows: Vec<Row> = soundcraft_engine::envelope::knowledge(&e)
        .into_iter()
        .map(|k| {
            let mut v = serde_json::to_value(&k).unwrap_or(Value::Null);
            if let Some(o) = v.as_object_mut() {
                // State-dependent (depends on what is selected); not reference data.
                o.remove("disabled_reason");
                o.insert("limits".into(), json!(soundcraft_engine::cmd::help::limits_for(k.id)));
            }
            Row { pro_tools: pt.get(k.id).cloned().unwrap_or_default(), studio_one: s1.get(k.id).cloned().unwrap_or_default(), k: v }
        })
        .collect();
    rows.sort_by(|a, b| a.k["id"].as_str().cmp(&b.k["id"].as_str()));
    let ui: Vec<Value> = soundcraft_ui_egui::menus::UI_COMMANDS
        .iter()
        .map(|(id, label, path, sc)| {
            json!({"id": id, "label": label, "menu": path, "pro_tools": sc.map(|s| soundcraft_ui_egui::shortcuts::shortcut_label(s, false)), "studio_one": s1.get(*id), "layer": "ui (desktop app only; not available to the headless CLI or a standalone MCP server)"})
        })
        .collect();
    let control_src = std::fs::read_to_string(root.join("crates/ui-egui/src/control.rs")).unwrap_or_default();
    let mcp_src = std::fs::read_to_string(root.join("crates/automation/src/mcp.rs")).unwrap_or_default();
    let control: Vec<String> = match_arm_names(&control_src).into_iter().collect();
    let tools = mcp_tools(&mcp_src);
    let limits: Vec<Value> = soundcraft_engine::cmd::help::LIMITS.iter().map(|(k, v)| json!({"applies_to": k, "limit": v})).collect();

    let commands_json = json!({
        "schema": SCHEMA,
        "product_version": VERSION,
        "generated_by": "cargo xtask docs (crates/ui-egui/examples/doc_reference.rs)",
        "note": "Generated from the command registry, shortcut tables, control.rs and mcp.rs of this source tree. Status/risk/undo come from the registry; risk is the registry's prefix rule, not a guarantee. Chords use Windows/Linux key names (Cmd = Ctrl); [unverified] marks Studio One-style rows not checked against that product's manual.",
        "counts": {"engine_commands": rows.len(), "ui_commands": ui.len(), "control_methods": control.len(), "mcp_tools": tools.len()},
        "engine_commands": rows.iter().map(|r| { let mut v = r.k.clone(); v["pro_tools"] = json!(r.pro_tools); v["studio_one"] = json!(r.studio_one); v }).collect::<Vec<_>>(),
        "ui_commands": ui,
        "control_methods": control,
        "mcp_tools": tools,
        "limits": limits,
    });

    // COMMAND_REFERENCE.md, grouped by namespace.
    let mut md = String::new();
    let _ = writeln!(md, "# Command reference — SoundCraft / SML Studio {VERSION}\n");
    let _ = writeln!(
        md,
        "<!-- GENERATED by `cargo xtask docs` from the registry of this source tree. Do not edit by hand; `cargo xtask docs-check` fails when it is stale. -->\n"
    );
    let _ = writeln!(
        md,
        "Every command this build registers ({} engine commands, {} UI-layer commands). Search this page for an id, a label or a menu name. **Pro Tools** and **Studio One** are the default chords of the two shortcut profiles (Windows/Linux names: `Cmd` is `Ctrl`); `[unverified]` marks Studio One-style rows not yet checked against that product's manual; `[shadowed: runs …]` marks a chord this command declares but never receives, because an earlier row with the same chord wins. **Undo**: `yes` = one undo step; `no` = not on the undo stack (view, transport, file and setup actions). **Risk** is the registry's own classification (destructive = writes or replaces files or the open session). **Status** `DISABLED` rows exist in menus but do nothing. Parameters are the registry's documentation strings, exactly as the CLI, control channel and MCP accept them. Machine-readable copy: [reference/commands.json](reference/commands.json). How to use a command: [USER_GUIDE.md](USER_GUIDE.md).\n",
        rows.len(),
        ui.len()
    );
    let mut groups: BTreeMap<String, Vec<&Row>> = BTreeMap::new();
    for r in &rows {
        let id = r.k["id"].as_str().unwrap_or("");
        groups.entry(id.split('.').next().unwrap_or("").to_string()).or_default().push(r);
    }
    let _ = writeln!(md, "## Namespaces\n");
    let _ = writeln!(md, "{}\n", groups.keys().map(|g| format!("[`{g}`](#{g})")).collect::<Vec<_>>().join(" · "));
    for (g, rs) in &groups {
        let _ = writeln!(md, "## {g}\n");
        let _ = writeln!(md, "| Command | Label | Menu | Pro Tools | Studio One | Undo | Risk | Status | Parameters and limits |");
        let _ = writeln!(md, "|---|---|---|---|---|---|---|---|---|");
        for r in rs {
            let k = &r.k;
            let menu = k["menu"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" › ")).unwrap_or_default();
            let mut p = k["params"].as_str().unwrap_or("").to_string();
            if let Some(l) = k["limits"].as_array().filter(|l| !l.is_empty()) {
                p.push_str(" — **Limits:** ");
                p.push_str(&l.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("; "));
            }
            let status = if k["status"].as_str() == Some("implemented") { "ok".to_string() } else { "DISABLED".to_string() };
            let _ = writeln!(
                md,
                "| `{}` | {} | {} | {} | {} | {} | {} | {} | {} |",
                k["id"].as_str().unwrap_or(""),
                esc(k["label"].as_str().unwrap_or("")),
                esc(&menu),
                esc(&r.pro_tools.join(", ")),
                esc(&r.studio_one.join(", ")),
                if k["undoable"].as_bool() == Some(true) { "yes" } else { "no" },
                k["risk"].as_str().unwrap_or(""),
                status,
                esc(&p)
            );
        }
        md.push('\n');
    }
    let _ = writeln!(md, "## UI-layer commands\n");
    let _ = writeln!(
        md,
        "Windows, panels and dialogs of the desktop app. They run through the app's menus, search, the control channel (`engine.execute`) and an MCP server connected to the running app, never in the headless CLI.\n"
    );
    let _ = writeln!(md, "| Command | Label | Menu | Pro Tools | Studio One |");
    let _ = writeln!(md, "|---|---|---|---|---|");
    for u in commands_json["ui_commands"].as_array().into_iter().flatten() {
        let s1v = u["studio_one"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).unwrap_or_default();
        let _ = writeln!(
            md,
            "| `{}` | {} | {} | {} | {} |",
            u["id"].as_str().unwrap_or(""),
            esc(u["label"].as_str().unwrap_or("")),
            esc(&u["menu"].as_str().unwrap_or("").replace(" > ", " › ")),
            esc(u["pro_tools"].as_str().unwrap_or("")),
            esc(&s1v)
        );
    }

    // SHORTCUTS.md: both profiles in table order.
    let mut sc = String::new();
    let _ = writeln!(sc, "# Shortcut profiles — SoundCraft / SML Studio {VERSION}\n");
    let _ = writeln!(sc, "<!-- GENERATED by `cargo xtask docs`. Do not edit by hand. -->\n");
    let _ = writeln!(
        sc,
        "The two built-in profiles of Setup › Keyboard Shortcuts…, as compiled into this build, with Windows/Linux key names (`Cmd` = `Ctrl`; a macOS-only chord with both is omitted). The app runs the **first** row for each chord; a later row with the same chord never fires (marked *shadowed* below, with what the key actually runs). Custom = one of these plus your own overrides (stored in your settings, not listed here). Limits that apply to every profile: keypad keys cannot be told from the main-row keys, and the Windows key is not reported to the app, so chords that need either cannot be delivered (the Nudge panel and command search always work). See [USER_GUIDE.md](../USER_GUIDE.md#21-shortcut-profiles).\n"
    );
    for (name, table) in [("Pro Tools style", shortcut_profiles::pro_tools_table()), ("Studio One style", shortcut_profiles::studio_one_table())] {
        let _ = writeln!(sc, "## {name}\n");
        let _ = writeln!(sc, "| Chord | Command | Parameters | Label | Source | Verified | Actually runs |");
        let _ = writeln!(sc, "|---|---|---|---|---|---|---|");
        for e in &table {
            let shown = soundcraft_ui_egui::shortcuts::shortcut_label(&e.chord, false);
            if shown.is_empty() {
                continue;
            }
            let _ = writeln!(
                sc,
                "| {} | `{}` | {} | {} | {} | {} | {} |",
                esc(&shown),
                e.command,
                if e.params.is_null() { String::new() } else { esc(&e.params.to_string()) },
                esc(&e.label),
                e.source,
                if e.verified { "yes" } else { "no" },
                winner(&table, e).map_or_else(|| "this row".to_string(), |w| format!("**shadowed** — `{w}`"))
            );
        }
        sc.push('\n');
    }

    // CAPABILITIES.md: stated limits, disabled entries, external interfaces.
    let mut cap = String::new();
    let _ = writeln!(cap, "# Capability reference — SoundCraft / SML Studio {VERSION}\n");
    let _ = writeln!(cap, "<!-- GENERATED by `cargo xtask docs`. Do not edit by hand. -->\n");
    let _ = writeln!(
        cap,
        "What this build states about itself. Hand-reviewed capability status (what was observed on which machine) is in [USER_GUIDE.md](../USER_GUIDE.md#2-capability-status); this page is generated from code and cannot say what works on your hardware.\n"
    );
    let _ = writeln!(cap, "## Stated limits (`help.limits`)\n");
    let _ = writeln!(cap, "| Applies to | Limit |");
    let _ = writeln!(cap, "|---|---|");
    for (k, v) in soundcraft_engine::cmd::help::LIMITS {
        // A key that is not itself a command is a prefix: written with `*`.
        let shown = if soundcraft_engine::find_command(k).is_some() { (*k).to_string() } else { format!("{}*", k.trim_end_matches('.')) };
        let _ = writeln!(cap, "| `{shown}` | {} |", esc(v));
    }
    let _ = writeln!(cap, "\n## Present in menus but not implemented\n");
    let _ = writeln!(cap, "| Command | Label |");
    let _ = writeln!(cap, "|---|---|");
    for r in rows.iter().filter(|r| r.k["status"].as_str() != Some("implemented")) {
        let _ = writeln!(cap, "| `{}` | {} |", r.k["id"].as_str().unwrap_or(""), esc(r.k["label"].as_str().unwrap_or("")));
    }
    let _ = writeln!(cap, "\n## External control interfaces\n");
    let _ = writeln!(
        cap,
        "Control-channel methods (JSON lines over TCP to the running desktop app; [control-protocol.md](../control-protocol.md)): {}.\n",
        control.iter().map(|m| format!("`{m}`")).collect::<Vec<_>>().join(", ")
    );
    let _ = writeln!(
        cap,
        "MCP tools (`soundcraft-cli mcp`, optionally `--connect PORT` to the running app; [mcp.md](../mcp.md)): {}.\n",
        commands_json["mcp_tools"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|m| format!("`{m}`"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let _ = writeln!(
        cap,
        "Counts: {} engine commands ({} undoable, {} destructive by the registry's rule, {} disabled), {} UI-layer commands.",
        rows.len(),
        rows.iter().filter(|r| r.k["undoable"].as_bool() == Some(true)).count(),
        rows.iter().filter(|r| r.k["risk"].as_str() == Some("destructive")).count(),
        rows.iter().filter(|r| r.k["status"].as_str() != Some("implemented")).count(),
        commands_json["ui_commands"].as_array().map_or(0, Vec::len)
    );

    vec![
        (GENERATED[0].to_string(), md),
        (GENERATED[1].to_string(), format!("{}\n", serde_json::to_string_pretty(&commands_json).unwrap_or_default())),
        (GENERATED[2].to_string(), sc),
        (GENERATED[3].to_string(), cap),
    ]
}

/// GitHub-style heading anchor.
fn slug(h: &str) -> String {
    let mut s = String::new();
    for c in h.trim().to_lowercase().chars() {
        if c.is_alphanumeric() || c == '-' || c == '_' {
            s.push(c);
        } else if c == ' ' {
            s.push('-');
        }
    }
    s
}

fn anchors(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
        }
        if fence {
            continue;
        }
        let t = line.trim_start();
        if t.starts_with('#') {
            let h = t.trim_start_matches('#');
            if h.starts_with(' ') {
                let base = slug(&h.replace('`', ""));
                let n = seen.entry(base.clone()).or_insert(0);
                out.insert(if *n == 0 { base.clone() } else { format!("{base}-{n}") });
                *n += 1;
            }
        }
    }
    out
}

/// Relative link targets `](target)` outside code fences.
fn links(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        let mut rest = line;
        while let Some(i) = rest.find("](") {
            rest = &rest[i + 2..];
            let Some(e) = rest.find(')') else { break };
            out.push(rest[..e].trim().to_string());
            rest = &rest[e..];
        }
    }
    out
}

/// Backticked spans that look like dotted identifiers (outside code fences): first token only.
fn ident_spans(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        let parts: Vec<&str> = line.split('`').collect();
        for (i, p) in parts.iter().enumerate() {
            if i % 2 == 0 || i + 1 >= parts.len() {
                continue;
            }
            let tok = p.split(|c: char| c.is_whitespace() || c == '{' || c == '=' || c == '(').next().unwrap_or("");
            let segs: Vec<&str> = tok.split('.').collect();
            if segs.len() < 2 || segs.iter().any(|s| s.is_empty() || !s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')) {
                continue;
            }
            if segs.last().is_some_and(|l| FILE_EXT.contains(l)) {
                continue;
            }
            out.push(tok.to_string());
        }
    }
    out
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let o = std::process::Command::new("git").arg("-C").arg(root).args(args).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn check(root: &Path, files: &[(String, String)]) -> i32 {
    let mut errors: Vec<String> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    for (path, text) in files {
        match std::fs::read_to_string(root.join(path)) {
            Ok(on_disk) if on_disk == *text => {}
            Ok(_) => errors.push(format!("{path} is stale: run `cargo xtask docs` and commit the result")),
            Err(_) => errors.push(format!("{path} is missing: run `cargo xtask docs`")),
        }
    }
    let manifest: Value = match std::fs::read_to_string(root.join("docs/DOCS_MANIFEST.json")).ok().and_then(|t| serde_json::from_str(&t).ok()) {
        Some(v) => v,
        None => {
            errors.push("docs/DOCS_MANIFEST.json is missing or not valid JSON".into());
            Value::Null
        }
    };
    if manifest["product_version"].as_str() != Some(VERSION) {
        errors.push(format!(
            "DOCS_MANIFEST.json product_version {:?} ≠ this build's {VERSION}: review the manual for the new version, then update the manifest",
            manifest["product_version"]
        ));
    }
    // Known identifiers.
    let e = soundcraft_engine::Engine::default();
    let mut known: BTreeSet<String> = soundcraft_engine::command_specs().iter().map(|c| c.id.to_string()).collect();
    known.extend(soundcraft_ui_egui::menus::UI_COMMANDS.iter().map(|c| c.0.to_string()));
    known.extend(match_arm_names(&std::fs::read_to_string(root.join("crates/ui-egui/src/control.rs")).unwrap_or_default()));
    let namespaces: BTreeSet<String> = known.iter().filter_map(|k| k.split('.').next().map(str::to_string)).collect();
    let _ = &e;
    let extra: BTreeSet<String> =
        manifest["non_command_identifiers"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
    // Documents: existence, links, anchors, identifiers.
    let docs: Vec<String> = manifest["documents"].as_array().into_iter().flatten().filter_map(|d| d["path"].as_str()).map(str::to_string).collect();
    if docs.is_empty() {
        errors.push("DOCS_MANIFEST.json lists no documents".into());
    }
    for d in &docs {
        let p = root.join(d);
        let Ok(text) = std::fs::read_to_string(&p) else {
            errors.push(format!("{d}: listed in DOCS_MANIFEST.json but missing"));
            continue;
        };
        if !d.ends_with(".md") {
            continue;
        }
        let dir = p.parent().map(Path::to_path_buf).unwrap_or_default();
        for l in links(&text) {
            if l.starts_with("http://") || l.starts_with("https://") || l.starts_with("mailto:") || l.is_empty() {
                continue;
            }
            let (file, anchor) = match l.split_once('#') {
                Some((f, a)) => (f.to_string(), Some(a.to_string())),
                None => (l.clone(), None),
            };
            let target = if file.is_empty() { p.clone() } else { dir.join(&file) };
            if !target.exists() {
                errors.push(format!("{d}: broken link `{l}`"));
                continue;
            }
            if let Some(a) = anchor
                && target.extension().is_some_and(|x| x == "md")
            {
                let t = std::fs::read_to_string(&target).unwrap_or_default();
                if !anchors(&t).contains(&a) {
                    errors.push(format!("{d}: link `{l}` points to a heading that does not exist"));
                }
            }
        }
        for id in ident_spans(&text) {
            let ns = id.split('.').next().unwrap_or("");
            if !namespaces.contains(ns) || known.contains(&id) || extra.contains(&id) {
                continue;
            }
            // A prefix such as `assistant.` written as `assistant.model` family is still checked
            // exactly; prefixes are written with a trailing `*` and are not matched here.
            errors.push(format!("{d}: `{id}` is not a command, UI command or control method of this build"));
        }
    }
    // Chapter map.
    for ch in manifest["chapters"].as_array().into_iter().flatten() {
        let name = ch["chapter"].as_str().unwrap_or("?");
        for c in ch["commands"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            let ok = if let Some(prefix) = c.strip_suffix('*') { known.iter().any(|k| k.starts_with(prefix)) } else { known.contains(c) };
            if !ok {
                errors.push(format!("DOCS_MANIFEST.json chapter {name}: unknown command `{c}`"));
            }
        }
        for s in ch["sources"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            if !root.join(s).exists() {
                errors.push(format!("DOCS_MANIFEST.json chapter {name}: source `{s}` does not exist"));
            }
        }
    }
    // The manual names the version it documents.
    let manual = std::fs::read_to_string(root.join("docs/USER_GUIDE.md")).unwrap_or_default();
    if !manual.lines().take(12).any(|l| l.contains(&format!("version {VERSION}"))) {
        errors.push(format!("docs/USER_GUIDE.md does not state `version {VERSION}` in its header"));
    }
    // Drift report (informational): chapters whose sources changed since verification.
    if let Some(sha) = manifest["verification"]["source_sha"].as_str() {
        match git(root, &["rev-list", "--count", &format!("{sha}..HEAD")]) {
            Some(n) => {
                notes.push(format!("documentation verified against {sha}; {n} commit(s) since"));
                for ch in manifest["chapters"].as_array().into_iter().flatten() {
                    let srcs: Vec<&str> = ch["sources"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                    if srcs.is_empty() {
                        continue;
                    }
                    let mut a = vec!["diff", "--name-only", sha, "HEAD", "--"];
                    a.extend(srcs);
                    if let Some(changed) = git(root, &a).filter(|c| !c.is_empty()) {
                        notes.push(format!(
                            "REVIEW chapter {} — source changed since verification: {}",
                            ch["chapter"].as_str().unwrap_or("?"),
                            changed.lines().collect::<Vec<_>>().join(", ")
                        ));
                    }
                }
            }
            None => notes.push(format!("cannot compare with the verified commit {sha} (no git or commit not present)")),
        }
    }
    for n in &notes {
        println!("docs: {n}");
    }
    if errors.is_empty() {
        println!("docs: ok ({} documents, {} generated files, version {VERSION})", docs.len(), files.len());
        0
    } else {
        for e in &errors {
            eprintln!("docs: ERROR {e}");
        }
        1
    }
}
