#!/usr/bin/env python3
"""X05: compare SoundCraft's loudness meter with FFmpeg's ebur128 filter on the frozen cases.

Usage: compare_meters.py <frozen.json> <dir with Xnn.wav and ours.csv> <out meter.csv>

`ours.csv` (written by `cargo run --release -p soundcraft-engine --example meter_vectors -- <dir>`)
holds SoundCraft's values per case; this script measures every Xnn.wav with ffmpeg and applies
the frozen tolerances to (a) ours vs ffmpeg and (b) each vs the analytic value when one exists.
"""
import csv, json, math, subprocess, sys

frozen = json.load(open(sys.argv[1]))
d, out = sys.argv[2], sys.argv[3]
tol = frozen["tolerances"]
ours = {r["case"]: r for r in csv.DictReader(open(f"{d}/ours.csv"))}

def ffmpeg_measure(path):
    cmd = ["ffmpeg", "-hide_banner", "-nostats", "-i", path, "-af",
           "ebur128=metadata=1:peak=true,ametadata=print:file=-", "-f", "null", "-"]
    text = subprocess.run(cmd, capture_output=True, text=True, check=True).stdout
    last, mmax, tp = {}, -math.inf, 0.0
    for line in text.splitlines():
        if not line.startswith("lavfi.r128."):
            continue
        k, v = line[len("lavfi.r128."):].split("=", 1)
        try:
            x = float(v)
        except ValueError:
            continue
        last[k] = x
        if k == "M":
            mmax = max(mmax, x)
        if k.startswith("true_peaks_ch"):
            tp = max(tp, x)
    return {
        "integrated_lufs": last.get("I"),
        "max_momentary_lufs": mmax,
        "loudness_range_lu": last.get("LRA"),
        "true_peak_db": 20 * math.log10(tp) if tp > 0 else -math.inf,
    }

rows, fails = [], 0
for case in frozen["cases"]:
    cid = case["id"]
    ff = ffmpeg_measure(f"{d}/{cid}.wav")
    o = ours[cid]
    for metric in ["integrated_lufs", "max_momentary_lufs", "loudness_range_lu", "true_peak_db"]:
        a, b = float(o[metric]), ff[metric]
        t = tol[metric]
        agree = b is not None and math.isfinite(a) and math.isfinite(b) and abs(a - b) <= t + 1e-9
        exp = case["analytic"].get(metric)
        ours_ok = "" if exp is None else str(abs(a - exp) <= t + 1e-9).lower()
        ff_ok = "" if exp is None else str(b is not None and abs(b - exp) <= t + 1e-9).lower()
        if not agree or ours_ok == "false":
            fails += 1
        rows.append([cid, metric, f"{a:.3f}", "" if b is None else f"{b:.3f}", "" if b is None else f"{a - b:+.3f}", t,
                     "" if exp is None else exp, str(agree).lower(), ours_ok, ff_ok, case["signal"]])

with open(out, "w", newline="") as f:
    w = csv.writer(f)
    w.writerow(["case", "metric", "soundcraft", "ffmpeg", "difference", "tolerance", "analytic", "agree", "soundcraft_vs_analytic", "ffmpeg_vs_analytic", "signal"])
    w.writerows(rows)
print(f"{len(rows)} comparisons, {fails} outside tolerance (agreement, or SoundCraft vs analytic)")
sys.exit(1 if fails else 0)
