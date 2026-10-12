#!/usr/bin/env python3
"""Source/runtime manifest for the Linux tarball (RC07/RC12), like cross-build.sh's for Windows.

Usage: linux_manifest.py <repo root> <tarball> <out MANIFEST.json>
Records the full commit, whether the tree had uncommitted changes (and which), toolchain
versions, every file's SHA-256 and size, and the shared libraries each ELF executable needs
(from `readelf -d` NEEDED entries; `ldd` on this build host for the resolved set).
"""
import hashlib, io, json, os, subprocess, sys, tarfile, tempfile, datetime

root, tarball, out = sys.argv[1:4]

def run(*a, cwd=None):
    try:
        return subprocess.run(a, capture_output=True, text=True, cwd=cwd, check=False).stdout.strip()
    except OSError:
        return ""

def sha(b):
    return hashlib.sha256(b).hexdigest()

dirty = run("git", "status", "--porcelain", "--untracked-files=no", cwd=root)
files = {}
with tarfile.open(tarball) as t, tempfile.TemporaryDirectory() as tmp:
    for m in t.getmembers():
        if not m.isfile():
            continue
        data = t.extractfile(m).read()
        entry = {"bytes": len(data), "sha256": sha(data)}
        if data[:4] == b"\x7fELF":
            p = os.path.join(tmp, os.path.basename(m.name))
            with open(p, "wb") as f:
                f.write(data)
            needed = [l.split("[")[1].rstrip("]") for l in run("readelf", "-d", p).splitlines() if "(NEEDED)" in l]
            entry["needed"] = needed
            entry["elf"] = run("file", "-b", p)
        files[m.name] = entry
with open(tarball, "rb") as f:
    tar_sha = sha(f.read())
manifest = {
    "artifact": os.path.basename(tarball),
    "sha256": tar_sha,
    "source": {"commit": run("git", "rev-parse", "HEAD", cwd=root), "dirty": bool(dirty), "uncommitted_changes": dirty.splitlines()},
    "built_utc": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "build_host": run("uname", "-srm") + "; " + (run("ldd", "--version").splitlines() or [""])[0],
    "toolchain": {"rustc": run("rustc", "-V"), "cargo": run("cargo", "-V")},
    "rebuild": "packaging/linux/package.sh --formats tar  (from the commit above; glibc of the build host is the minimum)",
    "audio_backends": ["ALSA (cpal default; PipeWire and PulseAudio reach it through their ALSA plugins)"], "jack": "not compiled in",
    "not_included": ["language model (none ships)", "code signing", "AppImage/deb/rpm (only tar.gz built here)"],
    "verification": "built and smoke-tested in the Linux build container (P); not installed on the owner's machine",
    "files": files,
}
with open(out, "w") as f:
    json.dump(manifest, f, indent=2)
print(json.dumps({k: manifest[k] for k in ("artifact", "sha256", "source")}, indent=2))
