# Product scope — revision 1.6
Goal: select interface -> record dry vocals -> comp/edit/gain/fade/nudge -> create simple named buses -> use personal plugins -> mix through Console or Visual -> reopen/export accurately.

## Owner direction and design authority
The owner requested exactly four sprints within six hours of Fable usage, recording ease and responsiveness, external plugins with upstream bundled plugins removed, professional mixer/buses, familiar vocal tools and a dedicated Nudge section, Logic-style behavior, LUNA/Harrison-inspired selectable Summing/Tape slots, a native spatial mixer and clock/latency improvements. Actual native summing/tape processors are explicitly deferred to another pass. `REQUIREMENTS_MATRIX.md` tracks every R01–R26 requirement.

The behavioral formulas, proposed numerical thresholds, native Windows assumption and interaction defaults are SML engineering decisions. Vendor documentation provides narrow references, not evidence of exact sonic/UI parity. This audit consolidates the owner's scope; it does not establish that the DAW works. Runtime measurements and implementation are NOT EXECUTED.

## Current product boundaries
Reuse Rust/egui/playback/mix and existing host architecture; no replacement DAW engine/framework. Stereo recording/export at 44.1/48 kHz; optional 96 kHz testing does not establish supported release capability. Windows x64 is the default planned target unless the owner already selected another. Native host/driver/plugin access is unknown. Portable implementation can proceed without asking again; a new platform port requires revised scope.

Use supported CPAL/native backend capabilities, including vendor ASIO where build/runtime support exists; no bespoke ASIO driver. Preserve visible WASAPI fallback limitations. Display effective device settings. Record dry source input; software input monitoring initially off. No automatic normalization, limiter, EQ or compressor on Main.

Remove selectable upstream insert effects/instruments, direct factories, demos and obsolete AudioSuite catalogue processors. Preserve/extract gain/pan/fades/clip envelopes/meters/resampling/click/dither/stretch services and external Plugin traits/hosts. Existing sessions retain removed IDs/state as inactive missing placeholders; no silent replacement or original-media overwrite. Do not delete unrelated mature DAW features just to simplify the default workspace.

Logic-style is a defined pan/profile option with precise clean summing, not a warmth preset. Console means the traditional mixer VIEW. Native analog summing/tape models are deferred, and their sonic tests are not current completion gates. Dedicated slots accept supported user plugins and expose an identity/native-provider contract. A stereo bus plugin does not gain per-track processing powers by occupying the Summing slot.

Console and Visual share one state/engine. Spatial controls require real gain/pan/width/undo/automation and snapshots. Visual reference interactions use original SML presentation; no claim of pixel-exact or complete iZotope parity. ARA/note-level pitch editor, new automatic alignment engine, AAX, new instruments, external MIDI hardware, video/immersive work, hardware DSP emulation, aggregate-device drift correction and new cloud features are excluded. VCA/foldback/monitor expansion and detailed tape-machine UI remain documented future candidates.

Product working name: SML Recording. Remove restricted upstream branding assets while preserving licenses/authorship; do not rename internal crates merely for branding. App launches only local control endpoints unless separately authorized; inspect flags before running servers.

## Authority and limits
`DOCUMENT_INDEX.md` assigns each topic one authoritative specification. `SPRINT_PLAN.md` owns dependency/order, `VALIDATION_PLAN.md` owns gate class/metric semantics, and the master owns authorized execution/budget. Research and changelog are non-normative. Contradictions must be resolved from the latest user scope and these authorities; never invent a favorable native result.

Numeric targets remain proposed acceptance criteria. Do not weaken them silently. If an actual required target fails, preserve evidence and report blocked/partial. Six-hour exhaustion produces an honest resumable checkpoint; it never waives tests or authorizes removing requested features.
