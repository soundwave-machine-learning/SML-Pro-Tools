//! HF21: authentication for the local control channel (protocol 2).
//!
//! The app's `--control` server admits a connection only after its first message is
//! `{"method":"auth","params":{"token":"…","client":"name","protocol":2}}` with the current
//! token. Anything else first — a command, malformed JSON, an HTTP request from a browser — is
//! answered with an error and the connection is closed before any command is dispatched.
//!
//! The token is 32 random bytes (OS randomness via `getrandom`) written as `smlc_` + 64 hex
//! digits to `control-token` in the SoundCraft settings folder (the owner's profile only; on
//! Unix the file is created with mode 0600). The server re-reads the file for every connection,
//! so rotating or revoking it takes effect at once. Clients read the same file, or
//! `SOUNDCRAFT_CONTROL_TOKEN`. Tokens are never logged; [`redact`] shows only the last four
//! digits. Comparison is constant-time.
//!
//! Migration: protocol 1 (no authentication) is not accepted any more; an old client gets a
//! one-line error naming the token file and the `auth` message, then the connection closes.

use std::path::{Path, PathBuf};

/// Control protocol version: 2 = authenticated (Hotfix Pass 03).
pub const PROTOCOL: u64 = 2;
/// Token file name inside the settings folder.
pub const TOKEN_FILE: &str = "control-token";
/// Environment variable a client may use instead of the token file.
pub const TOKEN_ENV: &str = "SOUNDCRAFT_CONTROL_TOKEN";
const PREFIX: &str = "smlc_";

/// SoundCraft's settings folder (same rules as the app: `$SOUNDCRAFT_CONFIG_DIR`, else
/// `%APPDATA%\SoundCraft`, `~/Library/Application Support/SoundCraft`, `$XDG_CONFIG_HOME/soundcraft`).
pub fn config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("SOUNDCRAFT_CONFIG_DIR") {
        return (!dir.is_empty()).then(|| PathBuf::from(dir));
    }
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("SoundCraft"))
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support/SoundCraft"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|c| c.join("soundcraft"))
    }
}

/// The token file in `dir`.
pub fn token_path(dir: &Path) -> PathBuf {
    dir.join(TOKEN_FILE)
}

/// The default token file, if there is a settings folder.
pub fn default_token_path() -> Option<PathBuf> {
    config_dir().map(|d| token_path(&d))
}

#[cfg(not(target_arch = "wasm32"))]
fn os_random(b: &mut [u8]) -> Result<(), String> {
    getrandom::fill(b).map_err(|e| format!("no OS randomness: {e}"))
}

/// The browser build has no control server and never creates a token.
#[cfg(target_arch = "wasm32")]
fn os_random(_: &mut [u8]) -> Result<(), String> {
    Err("control tokens are not available in the browser build".into())
}

/// A new random token.
pub fn generate() -> Result<String, String> {
    let mut b = [0u8; 32];
    os_random(&mut b)?;
    let mut s = String::with_capacity(PREFIX.len() + 64);
    s.push_str(PREFIX);
    for x in b {
        s.push_str(&format!("{x:02x}"));
    }
    Ok(s)
}

/// Whether `t` has the token shape (so a stray file is not mistaken for one).
pub fn well_formed(t: &str) -> bool {
    t.len() == PREFIX.len() + 64 && t.starts_with(PREFIX) && t[PREFIX.len()..].bytes().all(|c| c.is_ascii_hexdigit())
}

/// The token in `path`, if the file holds one.
pub fn load(path: &Path) -> Option<String> {
    let t = std::fs::read_to_string(path).ok()?;
    let t = t.trim().to_string();
    well_formed(&t).then_some(t)
}

/// Writes `token` to `path` atomically, readable by the owner only (Unix mode 0600; on Windows
/// the per-user settings folder's permissions apply).
fn write(path: &Path, token: &str) -> Result<(), String> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut o = std::fs::OpenOptions::new();
        o.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            o.mode(0o600);
        }
        let mut f = o.open(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
        use std::io::Write;
        f.write_all(token.as_bytes()).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// The token in `path`, creating one when there is none.
pub fn ensure(path: &Path) -> Result<String, String> {
    match load(path) {
        Some(t) => Ok(t),
        None => {
            let t = generate()?;
            write(path, &t)?;
            Ok(t)
        }
    }
}

/// Replaces the token (connections opened with the old one keep running; new ones need the new
/// token). Returns the new token.
pub fn rotate(path: &Path) -> Result<String, String> {
    let t = generate()?;
    write(path, &t)?;
    Ok(t)
}

/// Removes the token: no client can connect until a new one exists (the app creates one when it
/// next starts with `--control`).
pub fn revoke(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Constant-time comparison of a presented token with the expected one.
pub fn verify(expected: &str, presented: &str) -> bool {
    let (a, b) = (expected.as_bytes(), presented.as_bytes());
    let mut diff = a.len() ^ b.len();
    for i in 0..a.len().max(b.len()) {
        diff |= usize::from(a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0xff));
    }
    diff == 0 && well_formed(expected)
}

/// A token as it may appear in logs and screens: `smlc_…1a2b`.
pub fn redact(t: &str) -> String {
    let tail: String = t.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    format!("{PREFIX}…{tail}")
}

/// The message an old (protocol 1) client gets.
pub fn migration_message(path: Option<&Path>) -> String {
    format!(
        "authentication required (control protocol {PROTOCOL}): send {{\"method\":\"auth\",\"params\":{{\"token\":\"<token>\",\"client\":\"<name>\",\"protocol\":{PROTOCOL}}}}} first. The token is in {} (or set {TOKEN_ENV} for soundcraft-cli). See docs/control-protocol.md.",
        path.map_or_else(|| "the SoundCraft settings folder, file control-token".to_string(), |p| p.display().to_string())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_random_well_formed_and_rotatable() {
        let dir = std::env::temp_dir().join(format!("sml-auth-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p = token_path(&dir);
        assert_eq!(load(&p), None);
        let t1 = ensure(&p).unwrap();
        assert!(well_formed(&t1));
        assert_eq!(ensure(&p).unwrap(), t1, "ensure keeps an existing token");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600);
        }
        let t2 = rotate(&p).unwrap();
        assert_ne!(t1, t2);
        assert_eq!(load(&p).as_deref(), Some(t2.as_str()));
        assert!(verify(&t2, &t2) && !verify(&t2, &t1) && !verify(&t2, "") && !verify(&t2, &t2[..20]));
        assert!(!verify("short", "short"), "a malformed expected token never matches");
        revoke(&p).unwrap();
        assert_eq!(load(&p), None);
        revoke(&p).unwrap();
        std::fs::write(&p, "not a token").unwrap();
        assert_eq!(load(&p), None, "a stray file is not a token");
        let r = redact(&t2);
        assert!(r.starts_with("smlc_…") && r.len() < 16 && !r.contains(&t2[5..20]));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
