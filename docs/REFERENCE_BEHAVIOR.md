# Reference behavior and provenance


Revision 1.6 • retrieved 2026-10-11.

This package defines behavior targets and original engineering requirements; it does not contain copied proprietary resources. Older versioned documentation is deliberately named. No native Pro Tools or Studio One comparison was executed. The 2025.12 Pro Tools PDF could not be fetched by the web reader due to size, and the current Studio One manual landing URL redirects; neither establishes latest-version parity.

| ID | Official source | Narrow behavior used |
|---|---|---|
| PT-N1 | [Avid First 12.3 — Defining Nudge](https://apps.avid.com/proToolsFirstHelp/version12.3/enu/Pro%20Tools%20First%20Help/ed4.clips.26.14.html) | Select or type an independent nudge increment |
| PT-N2 | [Avid First 12.3 — Content Nudge](https://apps.avid.com/proToolsFirstHelp/version12.3/enu/Pro%20Tools%20First%20Help/ed4.clips.26.17.html) | Slip material within unchanged boundaries, limited by available handles |
| PT-N3 | [Avid First 12.3 — Trim Nudge](https://apps.avid.com/proToolsFirstHelp/version12.3/enu/Pro%20Tools%20First%20Help/ed4.clips.26.12.html) | Separate start/end trim operations and Windows modifier bindings |
| PT-N4 | [Avid First 12.3 — Selection boundaries](https://apps.avid.com/proToolsFirstHelp/version12.3/enu/Pro%20Tools%20First%20Help/ed3.selections.25.10.html) | Independent selection boundary movement |
| PT-K1 | [Avid First 2019.5 — Edit/Mix shortcuts](https://apps.avid.com/ProToolsFirstHelp/version2019.5/enu/keyshortcuts.6.10.html) | Numeric keypad convention and Windows nudge modifier map; Shift-only chord remains a documented compatibility gap; explicit commands are defined |
| S1-B1 | [PreSonus Studio One 4.1 manual, Busing p.162](https://pae-web.presonusmusic.com/downloads/products/pdf/Studio_One_4.1_Reference_Manual1.pdf) | Create bus for selected channels with automatic routing; hide/show sources |
| S1-V1 | [PreSonus Studio One 5.5 manual, Comping p.102](https://pae-web.presonusmusic.com/downloads/products/pdf/Studio_One_5.5_Reference_Manual_EN_20052022.pdf) | Preserve takes in layers and construct a performance from chosen phrases |

PT sources were read as official HTML; PreSonus relevant passages were retrieved from indexed official manuals. No full-manual audit is claimed. Nudge rounding, atomic rejection, SML presets, no-silent-route policy, performance criteria and test fixtures are original design choices. Exact behavioral deviations must be surfaced rather than called Pro Tools-standard automatically.

Additional repository inspection at pinned SHA 7414719f3760de1c9359ec9723351a7232442e61:
- crates/engine/src/cmd/edit.rs: existing nudge computes one step at selection start times count, moves selected clips and selection; strip-silence and playlist operations exist. Fractional-duration repeated rounding, tempo-boundary behavior and missing target modes need tests.
- crates/ui-egui/src/shortcuts.rs: broad Plus/Minus handler invokes generic edit.nudge when !command; it does not establish the full target/modifier matrix. Test Alt/Shift context dispatch and numpad distinctions.
- crates/time/src/grid.rs: existing step_samples and musical/frame/sample types are reuse candidates.
- crates/engine/src/cmd/edit_more.rs, track_more.rs: playlist copy/promotion and target helpers exist; end-to-end comp UI quality remains unverified.
- crates/engine/src/cmd/clip.rs: clip gain/envelope-related commands and time-warp path exist; removal of bundled effects must preserve editing services.
- crates/engine/src/cmd/mix.rs: sends already expose pre_fader and named bus arguments. The convenient atomic create-and-route workflow still needs inspection/implementation.
No code has been changed or tested by this planning update.

## Logic/LUNA extension
MIXER_SOUND_RESEARCH.md records new primary sources and evidence limits. MIXER_MODES_SPEC.md records our original formulas, UI and signal-path choices. Exact pan-curve matching and sonic equivalence remain unverified. Pan law/profile and nonlinear summing are separate controls.

## Visual Mixer reference (revision 1.6)
Primary versioned reference is iZotope Neutron 4.0.2 Visual Mixer documentation: https://s3.amazonaws.com/izotopedownloads/docs/neutron4/en/visual-mixer/index.html
VISUAL_MIXER_SPEC.md records observed control families and original native-DAW differences. No installed iZotope comparison or pixel/layout audit executed; do not claim exact UI or DSP replication. Latest iZotope version is not asserted.

## LUNA/Harrison sources and decisions
See LUNA_HARRISON_WORKFLOW_RESEARCH.md for versioned primary references and considered/deferred features. SL02 is our explicit graph contract. Exact UA/Harrison DSP and UI parity is not established. Models are deferred by owner; no interpretation of vendor marketing as a measured transfer function.

## Timing references
CLOCK_LATENCY_RESEARCH.md records Avid native/DSP distinctions, Reason versioned batching/threading behavior and Microsoft device timestamps. CLOCK_LATENCY_SPEC.md defines original proposed measurable goals; no native comparison executed.

## Audit reread, 2026-10-11
PT-K1 and dedicated PT-N3/PT-N4 HTML were reopened during revision 1.6 audit. Alt/Ctrl clip trim and their Shift selection-boundary variants are supported in the respective contexts. Shift-only is not asserted as Selection-only compatibility; NUDGE_SPEC.md leaves that reference chord unassigned while retaining the explicit SML command. Full native Pro Tools matching remains NOT EXECUTED.
