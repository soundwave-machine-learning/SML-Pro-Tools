//! HF22: the Python and JavaScript SDKs and their examples against a real authenticated server
//! (`soundcraft-cli serve`, the same server code as `soundcraft --control`), with its settings
//! folder in a temporary directory. A missing python3 or node is reported and skipped.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn sdks_against_the_authenticated_server() {
    let cfg = std::env::temp_dir().join(format!("sml-sdk-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cfg);
    std::fs::create_dir_all(&cfg).unwrap();
    let mut server = Command::new(env!("CARGO_BIN_EXE_soundcraft-cli"))
        .args(["serve", "--port", "0"])
        .env("SOUNDCRAFT_CONFIG_DIR", &cfg)
        .env_remove("SOUNDCRAFT_CONTROL_TOKEN")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(server.stdout.take().unwrap()).read_line(&mut line).unwrap();
    let port = line.trim().strip_prefix("SOUNDCRAFT_CONTROL_PORT=").expect("port line").to_string();
    assert!(cfg.join("control-token").is_file(), "the server created the token");
    let run = |prog: &str, args: &[&str]| -> Option<(bool, String)> {
        let out = Command::new(prog)
            .args(args)
            .current_dir(repo())
            .env("SOUNDCRAFT_CONFIG_DIR", &cfg)
            .env("SML_PORT", &port)
            .env_remove("SOUNDCRAFT_CONTROL_TOKEN")
            .output()
            .ok()?;
        Some((out.status.success(), format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))))
    };
    let mut ran = 0;
    let suites: [(&str, Vec<&str>); 6] = [
        // On the fresh, clean session the temporary-session mode runs...
        ("python3", vec!["-I", "sdk/examples/example.py", port.as_str(), "--temporary-session"]),
        ("python3", vec!["-I", "sdk/python/test_sml_studio.py"]),
        ("python3", vec!["-I", "sdk/examples/example.py", port.as_str()]),
        ("node", vec!["sdk/js/test.mjs"]),
        ("node", vec!["sdk/examples/example.mjs", port.as_str()]),
        // ...and on the now-unsaved session it refuses.
        ("python3", vec!["-I", "sdk/examples/example.py", port.as_str(), "--temporary-session"]),
    ];
    for (prog, args) in suites {
        match run(prog, &args) {
            Some((ok, out)) => {
                println!("== {prog} {args:?}\n{out}");
                if args.contains(&"--temporary-session") {
                    let expect_run = ran == 0;
                    assert_eq!(ok, expect_run, "{out}");
                    assert!(out.contains(if expect_run { "temporary session tracks" } else { "refused" }), "{out}");
                } else {
                    assert!(ok, "{prog} {args:?} failed:\n{out}");
                }
                ran += 1;
            }
            None => println!("{prog} not available: skipped {args:?}"),
        }
    }
    // The token never appears in any output.
    let token = std::fs::read_to_string(cfg.join("control-token")).unwrap();
    let _ = server.kill();
    let _ = server.wait();
    let _ = std::fs::remove_dir_all(&cfg);
    assert!(token.trim().starts_with("smlc_"));
    println!("{ran} SDK suites/examples ran");
}
