#!/usr/bin/env bash
# Reproducible Windows x64 portable build from Linux (mingw-w64, x86_64-pc-windows-gnu).
#
# Produces in $OUT (default dist/windows-gnu):
#   soundcraft-<version>-windows-x64-gnu-portable.zip   soundcraft.exe, soundcraft-cli.exe, licences, docs/, MANIFEST.json
#   soundcraft-<version>-windows-x64-gnu-portable.zip.sha256
#   MANIFEST.json (also inside the zip)
#
# The manifest links the artifact to its source: full commit SHA, whether the tree had
# uncommitted changes (and which), toolchain versions, target, audio backends compiled in, the
# DLLs each .exe imports, and SHA-256 of every file.
#
# What this is NOT: a signed build, an MSVC build, an installer, or evidence that it runs on
# Windows. The signed MSVC MSI/zip come from packaging/windows/package.ps1 on a Windows host
# (.github/workflows/release.yml). Running it on Windows is a separate (W) check.
#
# Needs: rustup target x86_64-pc-windows-gnu, x86_64-w64-mingw32-gcc / -objdump, python3.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
OUT="${OUT:-$ROOT/dist/windows-gnu}"
TARGET=x86_64-pc-windows-gnu
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
VERSION="$(sed -n '/^\[workspace.package\]/,/^\[/s/^version *= *"\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
[ -n "$VERSION" ] || { echo "cannot read the workspace version" >&2; exit 1; }
SHA="$(git -C "$ROOT" rev-parse HEAD)"
DIRTY="$(git -C "$ROOT" status --porcelain --untracked-files=no)"
command -v x86_64-w64-mingw32-gcc >/dev/null || { echo "x86_64-w64-mingw32-gcc not found" >&2; exit 1; }
rustup target list --installed | grep -qx "$TARGET" || { echo "rustup target $TARGET not installed" >&2; exit 1; }

echo "==> SoundCraft $VERSION for Windows x64 (GNU) from $SHA${DIRTY:+ (DIRTY tree)}"
CARGO_INCREMENTAL=0 cargo build --release --locked -p soundcraft -p soundcraft-cli --target "$TARGET"

BIN="$TARGET_DIR/$TARGET/release"
NAME="soundcraft-$VERSION-windows-x64-gnu-portable"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
STAGE="$WORK/$NAME"
mkdir -p "$STAGE" "$OUT"
cp "$BIN/soundcraft.exe" "$BIN/soundcraft-cli.exe" "$STAGE/"
cp "$ROOT/LICENSE-MIT" "$ROOT/LICENSE-APACHE" "$STAGE/"
cat > "$STAGE/README.txt" <<TXT
SoundCraft $VERSION — portable Windows x64 build (GNU toolchain, unsigned).
Run soundcraft.exe. Settings are stored in the usual per-user settings folder.
Audio backend: WASAPI (ASIO is not compiled into this build; see the project docs).
Windows SmartScreen may warn because this build is not signed.
Source: commit $SHA
TXT
# The user documentation of this source (manual, references, Grok guide, DOCS_MANIFEST.json).
python3 "$ROOT/packaging/docs/build_docs.py" --bundle-dir "$STAGE/docs" >/dev/null
export STAGE SHA DIRTY VERSION TARGET NAME OUT
python3 - <<'PY'
import hashlib, json, os, subprocess, zipfile, datetime
stage, out, name = os.environ["STAGE"], os.environ["OUT"], os.environ["NAME"]
def sha(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()
def run(*a):
    try:
        return subprocess.run(a, capture_output=True, text=True, check=False).stdout.strip()
    except OSError:
        return ""
files = {}
for f in sorted(os.listdir(stage)):
    p = os.path.join(stage, f)
    if os.path.isdir(p):
        continue
    entry = {"bytes": os.path.getsize(p), "sha256": sha(p)}
    if f.endswith(".exe"):
        dump = run("x86_64-w64-mingw32-objdump", "-p", p)
        entry["imports"] = sorted({l.split("DLL Name:")[1].strip() for l in dump.splitlines() if "DLL Name:" in l})
        entry["pe"] = next((l.strip() for l in run("file", p).splitlines()), "")
    files[f] = entry
dirty = os.environ.get("DIRTY", "")
manifest = {
    "artifact": name + ".zip",
    "version": os.environ["VERSION"],
    "source": {"commit": os.environ["SHA"], "dirty": bool(dirty), "uncommitted_changes": dirty.splitlines()},
    "built_utc": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "target": os.environ["TARGET"],
    "toolchain": {"rustc": run("rustc", "-V"), "cargo": run("cargo", "-V"), "mingw_gcc": run("x86_64-w64-mingw32-gcc", "--version").splitlines()[0] if run("x86_64-w64-mingw32-gcc", "--version") else ""},
    "audio_backends": ["WASAPI (cpal default)"],
    "not_included": ["ASIO (needs the Steinberg SDK and the MSVC toolchain; owner build)", "code signing", "installer (MSI comes from package.ps1)", "language model (none ships)"],
    "verification": "cross-built on Linux; NOT executed on Windows by this build (W pending)",
    "documentation": (lambda d: {"folder": "docs/", "product_version": d.get("product_version"), "docs_revision": d.get("docs_revision"), "verified_source_sha": d.get("verification", {}).get("source_sha"), "sha256sums": sha(os.path.join(stage, "docs", "SHA256SUMS"))})(json.load(open(os.path.join(stage, "docs", "DOCS_MANIFEST.json")))),
    "files": files,
}
with open(os.path.join(stage, "MANIFEST.json"), "w") as f:
    json.dump(manifest, f, indent=2)
zpath = os.path.join(out, name + ".zip")
with zipfile.ZipFile(zpath, "w", zipfile.ZIP_DEFLATED) as z:
    for dp, dns, fs in os.walk(stage):
        dns.sort()
        for f in sorted(fs):
            full = os.path.join(dp, f)
            info = zipfile.ZipInfo(name + "/" + os.path.relpath(full, stage).replace(os.sep, "/"), date_time=(2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            with open(full, "rb") as src:
                z.writestr(info, src.read())
with open(zpath + ".sha256", "w") as f:
    f.write(f"{sha(zpath)}  {name}.zip\n")
with open(os.path.join(out, "MANIFEST.json"), "w") as f:
    json.dump(manifest, f, indent=2)
print(f"==> {zpath}\n    sha256 {sha(zpath)}")
for k, v in files.items():
    if "imports" in v:
        print(f"    {k} imports: {', '.join(v['imports'])}")
PY
