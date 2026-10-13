//! `soundcraft-cli`: SoundCraft without a window.
//!
//! ```text
//! soundcraft-cli info FILE                       audio file or .scraft summary
//! soundcraft-cli convert IN OUT [--bit-depth 16|24|32f] [--rate HZ] [--normalize]
//! soundcraft-cli run [--in S.scraft | --demo] [--cmd ID[=JSON]]… [--bounce OUT] [--start T --end T] [--save S.scraft] [--inspect]
//! soundcraft-cli script FILE [--in S | --demo] [--save S] [--bounce OUT]   (one `ID JSON` per line)
//! soundcraft-cli commands [FILTER]               list commands
//! soundcraft-cli describe ID                     one command's details
//! soundcraft-cli app [--port P] METHOD [JSON]     call a running app's control channel
//! soundcraft-cli mcp [--connect PORT] [--demo]    MCP server on stdio
//! soundcraft-cli token <path|show|rotate|revoke>  the control channel's token (HF21)
//! soundcraft-cli serve [--port P] [--demo]        authenticated control server over a headless engine (SDK tests)
//! soundcraft-cli parity [--write PATH]            parity report (markdown)
//! soundcraft-cli plugins                          list built-in plugins
//! ```
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use serde_json::{Value, json};
use soundcraft_automation::{Backend, Headless, Remote, mcp::Server};
use soundcraft_engine::Engine;
use std::io::Write;
use std::process::ExitCode;

/// Print without panicking on a closed pipe.
macro_rules! outln {
    ($($t:tt)*) => {{
        let mut o = std::io::stdout().lock();
        if writeln!(o, $($t)*).is_err() { return ExitCode::SUCCESS; }
    }};
}

fn arg_value(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned()
}

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("soundcraft-cli: {msg}");
    ExitCode::FAILURE
}

/// The usage block of the module doc comment, up to its closing code fence.
fn help_text() -> String {
    include_str!("main.rs")
        .lines()
        .skip(3)
        .take_while(|l| l.trim_end() != "//! ```")
        .map(|l| l.trim_start_matches("//! "))
        .collect::<Vec<_>>()
        .join("\n")
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first().cloned() else {
        eprintln!("usage: soundcraft-cli <info|convert|run|script|commands|describe|app|mcp|token|parity|plugins> …  (see --help)");
        return ExitCode::FAILURE;
    };
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let code = match cmd.as_str() {
        "--version" | "-V" | "version" => {
            outln!("SoundCraft CLI {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        "--help" | "-h" | "help" => {
            outln!("{}", help_text());
            ExitCode::SUCCESS
        }
        "info" => info(&rest),
        "convert" => convert(&rest),
        "run" => run(&rest),
        "script" => script(&rest),
        "commands" => commands(&rest),
        "describe" => describe(&rest),
        "app" => app(&rest),
        "mcp" => mcp(&rest),
        "token" => token(&rest),
        "serve" => serve(&rest),
        "parity" => parity(&rest),
        // Isolated plugin-scan worker (R07): loads ONE bundle in this process and prints its
        // descriptors as JSON; a crash takes only this process down.
        "scan-plugin" | "--scan-plugin" => {
            let Some(path) = rest.first() else {
                eprintln!("usage: soundcraft-cli scan-plugin <bundle>");
                return ExitCode::FAILURE;
            };
            let code = soundcraft_mix::scan_isolated::worker_main(std::path::Path::new(path));
            return if code == 0 { ExitCode::SUCCESS } else { ExitCode::from(u8::try_from(code).unwrap_or(1)) };
        }
        // Sandboxed plugin worker (R31): hosts ONE plugin instance for the parent process.
        "plugin-worker" | "--plugin-worker" => {
            let Some(id) = rest.first() else {
                eprintln!("usage: soundcraft-cli plugin-worker <plugin id> [search dirs…]");
                return ExitCode::FAILURE;
            };
            let dirs: Vec<std::path::PathBuf> = rest.iter().skip(1).map(std::path::PathBuf::from).collect();
            std::process::exit(soundcraft_mix::sandbox::worker_main(id, &dirs));
        }
        // E03 experiment worker: a minimal out-of-process "plugin" (gain) over stdin/stdout.
        // Protocol: 4-byte little-endian frame count N, then N f32 samples; replies the same.
        // `--hang-after K` stops answering after K blocks; `--crash-after K` exits with SIGABRT.
        "plugin-worker-probe" => {
            let hang_after = rest.iter().position(|a| a == "--hang-after").and_then(|i| rest.get(i + 1)).and_then(|v| v.parse::<u64>().ok());
            let crash_after = rest.iter().position(|a| a == "--crash-after").and_then(|i| rest.get(i + 1)).and_then(|v| v.parse::<u64>().ok());
            use std::io::{Read, Write};
            let stdin = std::io::stdin();
            let stdout = std::io::stdout();
            let (mut inp, mut out) = (stdin.lock(), stdout.lock());
            let mut head = [0u8; 4];
            let mut blocks = 0u64;
            loop {
                if inp.read_exact(&mut head).is_err() {
                    return ExitCode::SUCCESS;
                }
                let n = u32::from_le_bytes(head) as usize;
                if n > 65_536 {
                    return ExitCode::FAILURE;
                }
                let mut buf = vec![0u8; n * 4];
                if inp.read_exact(&mut buf).is_err() {
                    return ExitCode::SUCCESS;
                }
                blocks += 1;
                if hang_after.is_some_and(|k| blocks > k) {
                    std::thread::sleep(std::time::Duration::from_secs(3600));
                }
                if crash_after.is_some_and(|k| blocks > k) {
                    std::process::abort();
                }
                for s in buf.chunks_exact_mut(4) {
                    let v = f32::from_le_bytes([s[0], s[1], s[2], s[3]]) * 0.5;
                    s.copy_from_slice(&v.to_le_bytes());
                }
                if out.write_all(&head).is_err() || out.write_all(&buf).is_err() || out.flush().is_err() {
                    return ExitCode::SUCCESS;
                }
            }
        }
        "plugins" => {
            for p in soundcraft_dsp::catalogue() {
                outln!("{:<20} {:<22} {:?}{}", p.id, p.name, p.category, if p.is_instrument { " (instrument)" } else { "" });
            }
            ExitCode::SUCCESS
        }
        other => fail(format!("unknown subcommand `{other}`")),
    };
    // The subcommand's engine and its plugin instances are gone by now.
    soundcraft_engine::shutdown_plugin_hosts();
    code
}

fn info(args: &[String]) -> ExitCode {
    let Some(path) = args.first() else { return fail("info FILE") };
    if path.ends_with(".scraft") {
        let mut e = Engine::default();
        match soundcraft_engine::io::open_session(&mut e, path) {
            Ok(missing) => warn_missing(&missing),
            Err(err) => return fail(err),
        }
        outln!("{}", soundcraft_engine::inspect::session_text(&e));
        return ExitCode::SUCCESS;
    }
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => return fail(format!("{path}: {e}")),
    };
    let ext = std::path::Path::new(path).extension().and_then(|x| x.to_str());
    match soundcraft_audio_io::probe(&bytes, ext) {
        Ok(i) => {
            outln!("{}", serde_json::to_string_pretty(&json!({"format": i.format, "sample_format": i.sample_format, "sample_rate": i.sample_rate, "channels": i.channels, "frames": i.frames, "seconds": i.duration_secs(), "bwf": i.bwf})).unwrap_or_default());
            ExitCode::SUCCESS
        }
        Err(e) => fail(format!("{path}: {e}")),
    }
}

fn convert(args: &[String]) -> ExitCode {
    let (Some(input), Some(output)) = (args.first(), args.get(1)) else { return fail("convert IN OUT") };
    let bytes = match std::fs::read(input) {
        Ok(b) => b,
        Err(e) => return fail(format!("{input}: {e}")),
    };
    let ext = std::path::Path::new(input).extension().and_then(|x| x.to_str());
    let (_, mut buf) = match soundcraft_audio_io::decode(&bytes, ext) {
        Ok(x) => x,
        Err(e) => return fail(format!("{input}: {e}")),
    };
    if let Some(rate) = arg_value(args, "--rate").and_then(|r| r.parse::<u32>().ok()) {
        buf.channels = soundcraft_dsp::offline::resample(&buf.channels, buf.sample_rate, rate);
        buf.sample_rate = rate;
    }
    if args.iter().any(|a| a == "--normalize") {
        soundcraft_dsp::offline::normalize(&mut buf.channels, -0.1, false);
    }
    let out_ext = std::path::Path::new(output).extension().and_then(|x| x.to_str()).unwrap_or("wav").to_ascii_lowercase();
    let Some(format) = soundcraft_audio_io::encode_format_for(&out_ext) else {
        return fail(format!("{output}: cannot write `.{out_ext}` files (supported: {})", soundcraft_audio_io::ENCODE_EXTENSIONS));
    };
    let bit_depth = match arg_value(args, "--bit-depth").as_deref() {
        Some("16") => soundcraft_audio_io::BitDepth::Int16,
        Some("32") | Some("32f") => soundcraft_audio_io::BitDepth::Float32,
        _ => soundcraft_audio_io::BitDepth::Int24,
    };
    let opts = soundcraft_audio_io::EncodeOptions { format, bit_depth, dither: true, bwf: None };
    match soundcraft_audio_io::encode(&buf, &opts).map_err(|e| e.to_string()).and_then(|b| std::fs::write(output, b).map_err(|e| e.to_string())) {
        Ok(()) => {
            outln!("wrote {output} ({} ch, {} Hz, {:.2} s)", buf.num_channels(), buf.sample_rate, buf.duration_secs());
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

/// One stderr line per media file a session refers to but could not load (the exit code is unchanged).
fn missing_warnings(missing: &[String]) -> Vec<String> {
    missing.iter().map(|m| format!("warning: missing media: {m}")).collect()
}

fn warn_missing(missing: &[String]) {
    for line in missing_warnings(missing) {
        eprintln!("{line}");
    }
}

fn load_engine(args: &[String]) -> Result<Engine, String> {
    if let Some(p) = arg_value(args, "--in") {
        let mut e = Engine::default();
        let missing = soundcraft_engine::io::open_session(&mut e, &p).map_err(|e| e.to_string())?;
        warn_missing(&missing);
        Ok(e)
    } else if args.iter().any(|a| a == "--demo" || a == "--sample") {
        Ok(soundcraft_engine::demo::demo_engine())
    } else {
        Ok(Engine::default())
    }
}

fn finish(e: &mut Engine, args: &[String]) -> ExitCode {
    if let Some(out) = arg_value(args, "--bounce") {
        let mut p = json!({"path": out});
        if let Some(s) = arg_value(args, "--start") {
            p["start"] = time_arg(&s);
        }
        if let Some(s) = arg_value(args, "--end") {
            p["end"] = time_arg(&s);
        }
        match e.execute("file.bounce_mix", &p) {
            Ok(v) => outln!("{v}"),
            Err(err) => return fail(err),
        }
    }
    if let Some(path) = arg_value(args, "--save") {
        match e.execute("session.save", &json!({"path": path})) {
            Ok(v) => outln!("{v}"),
            Err(err) => return fail(err),
        }
    }
    if args.iter().any(|a| a == "--inspect") {
        outln!("{}", serde_json::to_string_pretty(&soundcraft_engine::inspect::session(e, false)).unwrap_or_default());
    }
    ExitCode::SUCCESS
}

/// `12.5` → seconds; anything else is passed through as a time string.
fn time_arg(s: &str) -> Value {
    s.parse::<f64>().map_or_else(|_| json!(s), |f| json!({"seconds": f}))
}

/// `ID` or `ID=JSON` → command id and params; malformed JSON is an error, not `{}`.
fn parse_cmd_spec(spec: &str) -> Result<(String, Value), String> {
    match spec.split_once('=') {
        Some((id, p)) => serde_json::from_str::<Value>(p).map(|v| (id.to_string(), v)).map_err(|err| format!("{id}: bad JSON: {err}")),
        None => Ok((spec.to_string(), json!({}))),
    }
}

fn run(args: &[String]) -> ExitCode {
    let mut e = match load_engine(args) {
        Ok(e) => e,
        Err(err) => return fail(err),
    };
    let mut i = 0;
    while i < args.len() {
        if args.get(i).is_some_and(|a| a == "--cmd")
            && let Some(spec) = args.get(i + 1)
        {
            let (id, params) = match parse_cmd_spec(spec) {
                Ok(v) => v,
                Err(err) => return fail(err),
            };
            match e.execute(&id, &params) {
                Ok(v) => outln!("{id}: {v}"),
                Err(err) => return fail(format!("{id}: {err}")),
            }
            i += 1;
        }
        i += 1;
    }
    finish(&mut e, args)
}

fn script(args: &[String]) -> ExitCode {
    let Some(path) = args.first() else { return fail("script FILE") };
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => return fail(format!("{path}: {e}")),
    };
    let mut e = match load_engine(args) {
        Ok(e) => e,
        Err(err) => return fail(err),
    };
    let keep = args.iter().any(|a| a == "--keep-going");
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (id, p) = line.split_once(char::is_whitespace).unwrap_or((line, "{}"));
        let params = match serde_json::from_str::<Value>(p.trim()) {
            Ok(v) => v,
            Err(err) => return fail(format!("{path}:{}: bad JSON: {err}", n + 1)),
        };
        match e.execute(id, &params) {
            Ok(v) => outln!("{id}: {v}"),
            Err(err) => {
                eprintln!("{path}:{}: {id}: {err}", n + 1);
                if !keep {
                    return ExitCode::FAILURE;
                }
            }
        }
    }
    finish(&mut e, args)
}

fn commands(args: &[String]) -> ExitCode {
    let filter = args.first().cloned().unwrap_or_default();
    let e = Engine::default();
    for c in soundcraft_engine::command_specs() {
        if !filter.is_empty() && !c.id.contains(&filter) && !c.label.to_ascii_lowercase().contains(&filter.to_ascii_lowercase()) {
            continue;
        }
        let i = c.info(&e);
        outln!("{:<36} {:<34} {}", i.id, i.label, i.menu.join(" > "));
    }
    ExitCode::SUCCESS
}

fn describe(args: &[String]) -> ExitCode {
    let Some(id) = args.first() else { return fail("describe ID") };
    match soundcraft_engine::find_command(id) {
        Some(c) => {
            outln!("{}", serde_json::to_string_pretty(&c.info(&Engine::default())).unwrap_or_default());
            ExitCode::SUCCESS
        }
        None => fail(format!("unknown command `{id}`")),
    }
}

fn app(args: &[String]) -> ExitCode {
    let port = arg_value(args, "--port").or_else(|| std::env::var("SOUNDCRAFT_CONTROL_PORT").ok()).unwrap_or_else(|| "7979".into());
    let positional: Vec<&String> = args
        .iter()
        .enumerate()
        .filter(|(i, a)| !(a.starts_with("--") || (*i > 0 && args.get(i - 1).is_some_and(|p| p == "--port"))))
        .map(|(_, a)| a)
        .collect();
    let Some(method) = positional.first() else { return fail("app [--port P] METHOD [JSON]") };
    let params = match positional.get(1) {
        Some(p) => match serde_json::from_str::<Value>(p) {
            Ok(v) => v,
            Err(err) => return fail(format!("{method}: bad JSON: {err}")),
        },
        None => json!({}),
    };
    let mut r = Remote::new(&port);
    // A bare command id is shorthand for engine.execute.
    let (m, p) = if method.contains('.')
        && !method.starts_with("ui.")
        && !method.starts_with("engine.")
        && !method.starts_with("session.")
        && !method.starts_with("app.")
    {
        ("engine.execute".to_string(), json!({"command": method, "params": params}))
    } else {
        (method.to_string(), params)
    };
    match r.call(&m, p) {
        Ok(v) => {
            outln!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

/// HF21: the control channel's token in the settings folder. `show` prints the token itself
/// (for the owner to paste into an operator's settings); the others never print it.
fn token(args: &[String]) -> ExitCode {
    use soundcraft_automation::auth;
    let Some(path) = auth::default_token_path() else { return fail("no settings folder (SOUNDCRAFT_CONFIG_DIR is empty)") };
    match args.first().map(String::as_str) {
        Some("path") | None => {
            outln!("{}", path.display());
            ExitCode::SUCCESS
        }
        Some("show") => match auth::load(&path) {
            Some(t) => {
                outln!("{t}");
                ExitCode::SUCCESS
            }
            None => fail(format!("no token in {} (start SoundCraft with --control, or run `soundcraft-cli token rotate`)", path.display())),
        },
        Some("rotate") => match auth::rotate(&path) {
            Ok(t) => {
                outln!("new token {} written to {}", auth::redact(&t), path.display());
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        },
        Some("revoke") => match auth::revoke(&path) {
            Ok(()) => {
                outln!("token removed: no new control connections until a new one exists");
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        },
        Some(other) => fail(format!("token {other}: use path, show, rotate or revoke")),
    }
}

/// The app's authenticated control server (protocol 2) over a headless engine: the same
/// admission, bounds and token file as `soundcraft --control`, without a window or audio. For the
/// SDKs' end-to-end tests and scripts; `ui.*` methods answer that they need the desktop app.
fn serve(args: &[String]) -> ExitCode {
    use std::sync::{Arc, Mutex};
    let port = arg_value(args, "--port").and_then(|p| p.parse::<u16>().ok()).unwrap_or(0);
    let Some(token_path) = soundcraft_automation::auth::default_token_path() else {
        return fail("no settings folder (SOUNDCRAFT_CONFIG_DIR is empty)");
    };
    let e = if args.iter().any(|a| a == "--demo") { soundcraft_engine::demo::demo_engine() } else { Engine::default() };
    let backend = Arc::new(Mutex::new(Headless::new(e)));
    let dispatch: soundcraft_automation::server::Dispatch = Arc::new(move |method: String, params: Value| {
        let r = match backend.lock() {
            Ok(mut b) => b.call(&method, params),
            Err(_) => Err("engine unavailable".into()),
        };
        match r {
            Ok(v) => json!({"ok": true, "result": v}),
            Err(e) => json!({"ok": false, "code": "error", "error": e}),
        }
    });
    match soundcraft_automation::server::start(port, token_path.clone(), soundcraft_automation::server::Limits::default(), dispatch) {
        Ok(actual) => {
            outln!("SOUNDCRAFT_CONTROL_PORT={actual}");
            outln!("token file: {}", token_path.display());
            loop {
                std::thread::sleep(std::time::Duration::from_secs(3600));
            }
        }
        Err(e) => fail(e),
    }
}

fn mcp(args: &[String]) -> ExitCode {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let r = if let Some(port) = arg_value(args, "--connect") {
        let mut s = Server::new(Remote::new(&port));
        s.serve(stdin.lock(), stdout.lock())
    } else {
        let e = if args.iter().any(|a| a == "--demo") { soundcraft_engine::demo::demo_engine() } else { Engine::default() };
        let mut s = Server::new(Headless::new(e));
        s.serve(stdin.lock(), stdout.lock())
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(e),
    }
}

fn parity(args: &[String]) -> ExitCode {
    let p = soundcraft_engine::catalog::parity_json();
    let mut md = String::new();
    md.push_str("# Parity report\n\nGenerated by `soundcraft-cli parity --write docs/parity-checklist.md` (also `cargo xtask parity`).\n");
    md.push_str("It compares the incumbent's menu tree (feature names observed black-box, `crates/engine/catalog/menus.txt`) with SoundCraft's command registry.\n");
    md.push_str("UI-layer commands (windows, AudioSuite) are counted by the app's own report (`engine.parity` over the control channel).\n\n");
    md.push_str(&format!(
        "**Menu items implemented by the engine: {} / {} ({}%)**\n\n| Menu | Implemented | Total |\n|---|---:|---:|\n",
        p["implemented"], p["total"], p["percent"]
    ));
    for m in p["per_menu"].as_array().cloned().unwrap_or_default() {
        md.push_str(&format!("| {} | {} | {} |\n", m["menu"].as_str().unwrap_or(""), m["implemented"], m["total"]));
    }
    md.push_str("\n## Not yet implemented\n\n");
    for m in p["missing"].as_array().cloned().unwrap_or_default() {
        md.push_str(&format!("- {}\n", m.as_str().unwrap_or("")));
    }
    if let Some(path) = arg_value(args, "--write") {
        if let Err(e) = std::fs::write(&path, &md) {
            return fail(format!("{path}: {e}"));
        }
        outln!("wrote {path}: {}% ({}/{})", p["percent"], p["implemented"], p["total"]);
    } else {
        outln!("{md}");
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_media_gets_one_warning_each() {
        assert!(missing_warnings(&[]).is_empty());
        let w = missing_warnings(&["a.wav".to_string(), "b.wav: decode failed".to_string()]);
        assert_eq!(w, vec!["warning: missing media: a.wav", "warning: missing media: b.wav: decode failed"]);
    }

    #[test]
    fn cmd_spec_parses_id_and_json() {
        assert_eq!(parse_cmd_spec("a.b").unwrap(), ("a.b".to_string(), json!({})));
        assert_eq!(parse_cmd_spec("a.b={\"x\":1}").unwrap(), ("a.b".to_string(), json!({"x": 1})));
    }

    #[test]
    fn cmd_spec_rejects_truncated_json() {
        let err = parse_cmd_spec("a.b={\"x\":").unwrap_err();
        assert!(err.starts_with("a.b: bad JSON"), "{err}");
    }

    #[test]
    fn help_text_is_only_the_usage_block() {
        let help = help_text();
        assert!(help.starts_with("soundcraft-cli info FILE"));
        assert!(help.ends_with("soundcraft-cli plugins                          list built-in plugins"));
        assert!(!help.contains("```"));
        assert!(!help.contains("#![") && !help.contains("use serde_json"));
    }
}
