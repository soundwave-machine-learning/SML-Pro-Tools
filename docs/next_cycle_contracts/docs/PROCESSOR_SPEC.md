# Plugin hosting and integration specification — revision 2.2
Requirements R23–R31. The owner develops the seven plugins individually. This DAW cycle implements hosting and integration, not their algorithms, interfaces or proprietary emulations. Feature-level references do not constitute completed DSP research.

## Scope
The future individual projects are Channel EQ, Gate/Expander, flexible Compressor, Optical Leveler, Tune, Vocal Editor and Align. Their ideas are retained in [INDIVIDUAL_PLUGIN_BRIEF.md](INDIVIDUAL_PLUGIN_BRIEF.md). None must be created or supplied before the DAW can satisfy its own release contract. Preserve the removal of the upstream bundled effects. Do not produce replacement stock plugin shells, fake vendor models or hidden always-on processing.

Use the existing VST3 host and working CLAP support. Validate real personal plugins available on the target machine and deterministic test providers for generic hosting invariants. An integration-ready host is required now; named future-plugin compatibility is tested only once its actual artifact/interface is supplied. Missing future binaries are AWAITING_EXTERNAL_PLUGIN, not a failed DAW implementation and not a passed plugin test.

## Common host acceptance
Scan discovery/cache/quarantine; plugin identity/version; load/unload/replace; native editor lifecycle and focus; parameter discovery and units; automation; state/presets/save/reopen; bypass and PDC; sidechain routing; latency changes; audio/MIDI capabilities as actually exposed; offline rendering; missing-plugin state retention; crash/hang response; memory/callback bounds. Preserve IDs and avoid authoring proprietary metadata adapters without observed evidence.

Ordinary bypass preserves compensation. Quality/latency changes use a safe graph transition. The plugin may be nondeterministic; separate host signal routing/alignment from vendor DSP quality. Never claim hosting proves sound equivalence. Record plugin hash, format, version, platform, state and test environment. Retain original dry media.

## Seven external-project integration targets
| ID | Future project | Host work and conditional test |
|---|---|---|
| R23 | Channel EQ | Generic parameter/state/automation, stereo layout and display/editor lifecycle; test actual EQ build when supplied |
| R24 | Gate/Expander | Internal/external sidechain routing, detector source visibility, lookahead/PDC and state; test actual gate when supplied |
| R25 | Flexible Compressor | Automation/smoothing boundaries, wet/dry routing as plugin-defined, latency/state and sidechain; no recreation of RComp DSP |
| R26 | Optical Leveler | Save/reload, declared latency/tail, reset lifecycle and long processing; sonic memory behavior belongs to the external project |
| R27 | Tune | Low-latency live monitoring, declared plugin delay, dry-capture integrity and offline processing; no internal autotune implementation |
| R28 | Vocal Editor | Investigate the delivered interface first. A regular insert does not establish clip-linked note editing. Record whether native/ARA/other integration is needed, supported or separately blocked; no guessed SDK/API |
| R29 | Align | Validate delivered guide/dub input contract, sidechain/multi-input support and non-destructive render handoff where provided; no internal alignment algorithm |

## Native editing boundary
Ordinary DAW Bend/warp and pitch-preserving timing edits remain required as audio editing tools. Reuse an existing service or a vetted legally compatible time-stretch dependency, with source/hash/quality/latency validation. That work does not authorize building Tune, Melodyne-style note editing or VocAlign-style alignment. Missing a real warp service still blocks Bend completion. Native edit maps and bounded render/cache ownership remain DAW requirements.

For future clip-aware plugins, prepare a small capability/integration assessment based on the actual artifact. Do not promise generic VST3 automatically provides Melodyne-like native clip exchange. ARA/new formats are separate decisions, not automatic scope additions. Do not create speculative code for interfaces not delivered.

R30 covers non-destructive edit maps and integration boundaries; R31 covers plugin compatibility, timing, callback and rendering evidence. Test existing real personal plugins now and report future binaries separately. Preserve Summing/Tape plugin slots; their custom sound models remain deferred.

## Assistant and mastering
Only registered real plugin capabilities may enter engineer recipes. Use actual parameter IDs/units and available plugins; do not guess vendor parameter names. No required future plugin must block the general assistant, user-plugin mastering or DAW release. Unsupported future actions return a clear explanation without mutation.
