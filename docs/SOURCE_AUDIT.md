# Planning source audit


Date: 2026-10-11. Evidence is repository inspection, not runtime verification.
Historical planning main resolved through GitHub commit endpoint: 7414719f3760de1c9359ec9723351a7232442e61.

| Observation | Source | Provenance |
|---|---|---|
| Fork of storytold/soundcraft; main branch | GitHub repository metadata | INSPECTED |
| Rust workspace, core minimum 1.90, egui UI and external host crates | Cargo.toml | INSPECTED |
| Pre-alpha and interface/MIDI/interchange gaps | ROADMAP.md, docs/gaps.md | UPSTREAM REPORTED; not rerun |
| Output open_device chooses default_output_device; BLOCK=512 | crates/playback/src/lib.rs | SOURCE INSPECTED |
| Setup command stores engine.device/engine.buffer_size | crates/engine/src/cmd/setup_more.rs | SOURCE INSPECTED |
| Recorder opens default input, collects Vec samples into RAM; callback extends growing buffers | crates/playback/src/record.rs | SOURCE INSPECTED; realtime safety needs runtime instrumentation |
| Bundled effects/instruments in registry/factory | crates/dsp/src/plugins/mod.rs | SOURCE INSPECTED |
| Existing mixer has insert/send/route controls and built-in EQ visual dependency | crates/ui-egui/src/mix_window.rs | SOURCE INSPECTED; correctness not validated |
| Native package minimum macOS 11 | packaging/macos/Info.plist.in | SOURCE INSPECTED |
| Local craftrules AGENTS referenced; remote fetch returned 404 | AGENTS.md + attempted GitHub read | UNAVAILABLE; no contents inferred |

All build/recording/performance/plugin compatibility tests: NOT EXECUTED. Source reads preceded final main SHA resolution; execution must reconcile a moving branch at preflight. Owner supplied no hardware captures, driver version or plugin inventory. No six-hour feasibility measurement exists.

Primary source references should use https://github.com/soundwave-machine-learning/SML-Pro-Tools/blob/7414719f3760de1c9359ec9723351a7232442e61/ plus the exact paths above. Execute against resolved SHA, not assumptions from README marketing or menu-count parity. Q-DRE applied engineering priorities: monitoring judgment, deliberate routing, gain transparency and measured latency; no supplied audio was auditioned.

Revision 1.6 adds pinned-source inspection of edit.rs, edit_more.rs, clip.rs, clip_more.rs, track_more.rs, mix.rs, shortcuts.rs and time/grid.rs. Existing commands support reuse, but completion and correctness remain unverified. Official reference behavior is indexed in REFERENCE_BEHAVIOR.md; no current-version full-manual or runtime parity audit claimed.

## Revision 1.6 audit boundary
Mixer-mode research is vendor documentation only. Existing crates/mix/src/lib.rs, mixer model, graph and pan implementations require a fresh callsite audit in Sprint 1; this package does not establish their present accumulation precision, PDC completeness or anti-alias implementation. Do not treat proposed f64/slot architecture as a description of current code.

## Revision 1.6 source check
Re-read crates/playback/src/lib.rs lines 1–150 and playback/Cargo.toml at pinned planning SHA. Confirmed BLOCK=512, pending output buffers/read cursor, distinct session/device rates, mpsc command channel and between-block plugin state capture. Exact latency not measured. ASIO activation cannot be concluded from playback manifest alone; inspect workspace features/full backend. CLOCK_LATENCY_RESEARCH.md records limits.
