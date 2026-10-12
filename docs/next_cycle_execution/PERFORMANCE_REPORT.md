# Measured performance report (portable, this cycle)

All numbers below come from headless runs in container P (Linux x86_64, no audio device, no
GUI, debug builds unless stated). They bound the software cost of the paths named; the native
W1/W2 targets of VALIDATION_AND_EXPERIMENTS.md remain TARGET_PENDING. Files are under
`docs/evidence/next_cycle/`.

| What | Result | File |
|---|---|---|
| B02 64-frame control (release) | control misses 98/45 000 periods from scheduler preemption; workload p99 ≤ 5 % of budget | `sprint_01/callback-profile-control.csv` |
| E02 autosave/media durability | fault-injected commit paths pass; power loss unverified | `sprint_01/ci-b01-b04-e02.log` |
| E03 IPC probe (release) | round trip p50 ≈ 40 µs, p99 ≈ 0.12 ms, tails to 7 ms; crash 6.8 ms / hang 16 ms detection | `sprint_01/ipc-probe.csv` |
| E04 menu / search at W1 (software renderer) | menu open p95 12.5 ms, search p95 14.7 ms | `sprint_02/menu-timing.csv` |
| Sandboxed instance exchange (debug) | p50 23–72 µs, p99 64–500 µs by block; 0–3 late blocks of 2000; restart 61 ms; hang declared ≤ 1.05 s | `sprint_03/sandbox-e03.csv` |
| W1 hosted chain under autosave (debug) | render p50 277 / p99 1323 / max 8399 µs of a 5333 µs budget at 256 frames; 8 of 3750 blocks over | `sprint_03/w1-hosted-load.csv` |
| E07 meter conformance | see Sprint 4 `e07-meters.csv` | `sprint_04/` |

Native (N) measurements, xrun counts, delivered frame intervals, input-to-sound latency, the
30-minute recordings and the soak runs have not been performed in this environment.
