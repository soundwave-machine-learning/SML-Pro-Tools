"""Hotfix 03: runs the Grok guide's level-C cookbook through the real authenticated control path.

Starts `soundcraft-cli serve` with a temporary settings folder (its own token), connects with the
Python SDK, builds a disposable session from synthetic audio in a temporary folder and runs each
cookbook row as the guide writes it. Never touches a real session or the owner's settings.

    python3 -I cookbook_serve.py PATH/TO/soundcraft-cli  > cookbook-serve.log
"""
import json
import math
import os
import struct
import subprocess
import sys
import tempfile
import wave
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "sdk" / "python"))
import sml_studio  # noqa: E402

cli = sys.argv[1]
work = Path(tempfile.mkdtemp(prefix="sml-cookbook-"))
cfg = work / "settings"
cfg.mkdir()
env = dict(os.environ, SOUNDCRAFT_CONFIG_DIR=str(cfg))
env.pop("SOUNDCRAFT_CONTROL_TOKEN", None)
os.environ.update(SOUNDCRAFT_CONFIG_DIR=str(cfg))
os.environ.pop("SOUNDCRAFT_CONTROL_TOKEN", None)
srv = subprocess.Popen([cli, "serve", "--port", "0"], env=env, stdout=subprocess.PIPE, text=True)
port = int(srv.stdout.readline().strip().split("=", 1)[1])


def tone(path, freq, secs=3.0, rate=48000, amp=0.3):
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(b"".join(struct.pack("<h", int(amp * 32767 * math.sin(2 * math.pi * freq * i / rate))) for i in range(int(secs * rate))))


for name, f in [("lead", 220), ("dbl_l", 330), ("dbl_r", 440), ("adlib", 550), ("ref", 110)]:
    tone(work / f"{name}.wav", f)

ok = fail = 0


def row(label, fn):
    global ok, fail
    try:
        r = fn()
        ok += 1
        print(f"OK    {label}: {json.dumps(r)[:240]}")
        return r
    except sml_studio.SmlError as e:
        fail += 1
        print(f"FAIL  {label}: {type(e).__name__}: {e}")


def expect_refused(label, fn, needle):
    global ok, fail
    try:
        fn()
        fail += 1
        print(f"FAIL  {label}: expected a refusal")
    except sml_studio.CommandError as e:
        good = needle in str(e)
        ok += good
        fail += not good
        print(f"{'OK   ' if good else 'FAIL '} {label} (refused as documented): {e}")


try:
    with sml_studio.Client(port, client="cookbook") as c:
        print(f"server protocol {c.server['protocol']}, app {c.server['app_version']}; settings in a temporary folder; token not shown")
        x = c.execute
        row("new disposable session", lambda: x("session.new", {"name": "Cookbook (disposable)"}))
        for n in ["lead", "dbl_l", "dbl_r", "adlib"]:
            row(f"import {n}", lambda n=n: x("file.import_audio", {"path": str(work / f"{n}.wav")}))
        s = c.inspect()
        names = [t["name"] for t in s["tracks"]]
        print("tracks:", names)
        row("Rename a track", lambda: x("track.rename", {"track": "lead", "name": "Lead"}))
        row("rename dbl_l", lambda: x("track.rename", {"track": "dbl_l", "name": "Dbl L"}))
        row("rename dbl_r", lambda: x("track.rename", {"track": "dbl_r", "name": "Dbl R"}))
        row("rename adlib", lambda: x("track.rename", {"track": "adlib", "name": "Adlib"}))
        row("Select tracks and a range", lambda: x("edit.select", {"tracks": ["Lead"], "start": 0, "end": 480}))
        row("Fade over the selection", lambda: x("edit.fades_create", {"shape": "equal power"}))
        clips = [cl["id"] for t in c.call("session.inspect", {"detail": "full"})["tracks"] for cl in t.get("clips", [])]
        row("Clip gain", lambda: x("clip.gain", {"clips": clips[:2], "db": -6}))
        row("Nudge set", lambda: x("nudge.set", {"value": 10, "unit": "ms", "target": "clips"}))
        row("select clip", lambda: x("edit.select", {"clips": [clips[0]]}))
        row("Nudge later", lambda: x("nudge.clips", {"direction": 1}))
        row("Nudge back", lambda: x("nudge.clips", {"direction": -1}))
        row("Fader", lambda: x("mix.volume", {"tracks": ["Dbl L", "Dbl R"], "db": -8}))
        row("Pan", lambda: x("mix.pan", {"tracks": ["Dbl L"], "pan": -0.6}))
        row("Mute", lambda: x("mix.mute", {"tracks": ["Adlib"], "value": True}))
        row("select for bus", lambda: x("edit.select", {"tracks": ["Lead", "Dbl L", "Dbl R", "Adlib"], "exact": True}))
        row("Vocal bus", lambda: x("bus.create_from_selection", {"name": "Vocal Bus"}))
        row("Reverb return", lambda: x("bus.create_fx_return", {"tracks": ["Lead", "Dbl L", "Dbl R", "Adlib"], "name": "Vocal Verb"}))
        row("Send level", lambda: x("mix.send_level", {"track": "Lead", "slot": 0, "db": -18}))
        row("bus.validate", lambda: x("bus.validate", {}))
        row("mix.signal_flow", lambda: x("mix.signal_flow", {"track": "Lead"}))
        row("mix.latency_report", lambda: x("mix.latency_report", {}))
        row("Snapshot save", lambda: x("snapshot.save", {"slot": "A"}))
        row("Snapshot recall", lambda: x("snapshot.recall", {"slot": "A"}))
        row("Measure", lambda: x("mastering.meters", {}))
        row("import reference", lambda: x("file.import_audio", {"path": str(work / "ref.wav")}))
        src = [so["id"] for so in c.inspect()["sources"] if "ref" in json.dumps(so)]
        row("delete reference track", lambda: x("track.delete", {"track": "ref"}))
        row("register reference", lambda: x("mastering.reference", {"source": src[-1]}))
        row("level match", lambda: x("mastering.level_match", {}))
        # Hotfix 03 rows
        row("HF15 A/B on", lambda: x("mastering.reference", {"listening": True}))
        row("HF15 A/B off", lambda: x("mastering.reference", {"listening": False}))
        m = row("HF16 add a Master track", lambda: x("track.new", {"kind": "master"}))
        master = m["tracks"][0]
        row("HF16 Master mute", lambda: x("mix.mute", {"tracks": [master], "value": True}))
        r = row("HF16 export with Master muted (warns)", lambda: x("file.bounce_mix", {"path": str(work / "muted.wav"), "start": 0, "end": 48000}))
        print("      master_muted:", r and r.get("master_muted"), "| warning:", r and r.get("warning"))
        row("HF16 Master unmute", lambda: x("mix.mute", {"tracks": [master], "value": False}))
        row("Export with verification", lambda: x("file.bounce_mix", {"path": str(work / "mix.wav"), "bit_depth": "24", "tail": "auto", "report": True}))
        if sys.platform == "win32":
            row("HF20 MP3 export", lambda: x("file.bounce_mix", {"path": str(work / "mix.mp3"), "bitrate": 320}))
        else:
            expect_refused("HF20 MP3 export off Windows", lambda: x("file.bounce_mix", {"path": str(work / "mix.mp3"), "bitrate": 320}), "not available on this platform")
        expect_refused("HF20 VBR refused", lambda: x("file.bounce_mix", {"path": str(work / "v.mp3"), "mode": "vbr"}), "constant bit rate")
        expect_refused("HF20 extension mismatch refused", lambda: x("file.bounce_mix", {"path": str(work / "v.wav"), "format": "mp3"}), "end in .mp3")
        row("Undo history", lambda: x("engine.history", {}))
        row("Undo", lambda: x("edit.undo", {}))
        row("Tempo", lambda: x("event.tempo", {"bpm": 120, "at": 0}))
        row("Meter", lambda: x("event.meter", {"numerator": 6, "denominator": 8, "at": 0}))
        row("loop.info", lambda: x("loop.info", {}))
        row("loop.set", lambda: x("loop.set", {"start": 48000, "end": 96000}))
        row("loop.clear", lambda: x("loop.clear", {}))
        row("track for move", lambda: x("track.new", {"count": 1, "format": "mono", "name": "Lead Dbl"}))
        lead_clip = [cl["id"] for t in c.call("session.inspect", {"detail": "full"})["tracks"] if t["name"] == "Lead" for cl in t.get("clips", [])]
        row("Move clips to another track", lambda: x("edit.move_clips", {"clips": lead_clip[:1], "track": "Lead Dbl", "from": "Lead"}))
        expect_refused("Plugin scan job (headless serve has no scan service)", lambda: c.scan_start(), "scan")
        print(f"\n{ok} OK, {fail} failed; ui.*/window.* rows need the desktop app and were not run here.")
finally:
    srv.kill()
    srv.wait()
