# Upstream bundled plugin removal — inventory (R04/R05, gate A1)

Provenance: INSPECTED (source search on `fable/recording-core`) and RERUN (tests named below).
Removed ids (26): see `catalogue.json` (`eq_1band … drum_synth`).

## Product routes and what changed

| Route | Before | After | Evidence |
|---|---|---|---|
| Insert menu (Mix window, `mix_window.rs` `plugin_menu`/`instrument_menu`) | Listed `soundcraft_dsp::plugins()` by category | Lists `soundcraft_dsp::catalogue()` = empty; only installed CLAP/VST3/AU appear (`hosted_menus`) | source |
| `mix.insert` command (menus, CLI, control channel, MCP all dispatch it) | Accepted built-in ids | Rejects removed ids with an explicit message; `cmd::clap::plugin_info` no longer resolves built-ins | `cmd::clap::tests::unknown_clap_insert_is_an_error` |
| `mix.instrument` / `track.new kind=instrument` | Default `subtractive_synth` | No bundled default; removed instruments rejected | `cmd::instrument::tests::no_bundled_default_and_removed_instruments_rejected` |
| MIDI file import (`io.rs`) | Assigned `drum_synth`/`subtractive_synth` | Assigns no instrument | source |
| `engine.plugins` query, `soundcraft-cli plugins` | Listed the registry | Empty catalogue | `catalogue-cli.txt` (0 lines) |
| `audiosuite.process` | Any registry plugin id plus named services | Named editing services only (`normalize, reverse, invert, gain, duplicate, time_stretch, pitch_shift, varispeed`); registry ids rejected | source + engine suite |
| Clip Effects (EQ/compressor per clip, `mix` `ClipFx`) | Instantiated bundled EQ/compressor | Not instantiated (parameters stay in `edit.values`, inactive); clip-effects gain retained | source |
| Mix engine `create_plugin` | Built-in first, then hosts | Hosts only (+ `test:` fixtures under `cfg(test)`/`test-providers`); removed ids → `None` = pass-through placeholder | `mix tests::inserts_process_and_survive_unknown_plugins` |
| Demo session (`demo.rs`) | EQ/comp/saturator/chorus/maximizer/plate reverb + synth instruments | Dry synthetic audio; MIDI parts kept with no instrument; "Verb" is an empty FX return with sends at -inf | `demo::tests::demo_builds_and_renders_sound` |

## Retained internal services (not selectable)

Clip gain/envelopes and fades (`mix::render_clip`), metering (`dsp::meter`), pan (`dsp::pan`,
new `dsp::mixmath`), resampling/time-stretch/pitch-shift offline services (`dsp::offline`), click
(`mix::click`), dither (export path), external host traits (`dsp::Plugin`, `clap-host`,
`vst3-host`, `au-host`). **Update (after Sprint 3):** the upstream implementation modules under `crates/dsp/src/plugins/`
(~3.8k lines: EQ, dynamics, reverb, modulation, harmonic, pitch, utility, synth, drums, strip)
were **deleted**. Only their metadata (ids, names, categories, parameter tables) remains in
`crates/dsp/src/removed_meta.rs`, used to show old sessions' placeholders by name and keep their
values; `soundcraft_dsp::create` instantiates nothing. Helper code used only by them was removed.
The EQ-curve displays (Mix strip section and plugin window) that drew the removed EQ's response
were removed; the plugin window now labels such inserts as missing. Retained offline services
(`dsp::offline`: gain, normalize, reverse, invert, fades, resample, time-stretch, pitch-shift,
`apply_plugin`) do not depend on the deleted code. CI: `docs/evidence/sprint_03/ci-module-removal.log`.

## Old sessions (R05)

A removed id in a saved session stays an `Insert` with the same id, parameters, state blob and
slot. The engine treats it as an inactive missing placeholder (audio passes through unchanged,
never replaced by another processor). `session.missing_plugins` lists them; `file.bounce_mix` and
`file.bounce_stems` refuse to export while any active one is present unless
`acknowledge_missing: true` is passed. Source media are never touched.
Tests: `cmd::bus::missing_tests::missing_processors_survive_reload_and_block_silent_export`
(save → reopen keeps id/params/state; export blocked; acknowledged export written).

## Tests changed or retired

| Test | Change | Rationale |
|---|---|---|
| `mix tests::delay_compensation_aligns_latent_tracks` | Uses `test:delay:64` instead of bundled `maximizer` lookahead | Same PDC coverage with a deterministic known-delay fixture |
| `mix tests::inserts_process_and_survive_unknown_plugins` | Now asserts removed built-in = pass-through placeholder with retained params, and that a `test:gain:-6` insert processes exactly -6 dB | Shared insert-processing coverage kept |
| `engine cmd::instrument::tests::sets_an_instrument_tracks_instrument_with_defaults_and_undoes` | Retired → `no_bundled_default_and_removed_instruments_rejected` | Covered only the removed bundled synths |
| `engine cmd::instrument::tests::refuses_effects_unknown_plugins_and_other_track_kinds` | Expects no default instrument | Bundled default removed |
| `engine cmd::clap::tests::unknown_clap_insert_is_an_error` | Expects built-ins unresolvable + explicit removal error | Catalogue removed |
| `engine cmd::plugin_state::tests::*` (2) | Use a hosted-plugin placeholder insert instead of `mix.insert gain` | State-blob storage is independent of the plugin |
| `engine cmd::track_more::tests::bypass_by_category_and_range`, `preset_round_trip` | Place legacy placeholder inserts directly in the session | Category bypass/preset behaviour still covered for old sessions |

No tolerance was loosened and no expected output was regenerated.
| `automation tests/agent_tasks.rs::agent_builds_and_mixes_a_session` | Step 3 (insert bundled `eq_7band` + bypass) replaced by `bus.create_from_selection` over MCP; bundled `mod_delay` insert removed from step 4 (the aux stays an empty return) | Catalogue removed; MCP agent workflow coverage kept with a real SML feature |
| `dsp tests/plugins.rs` (25 tests) | Retired | Exercised only the deleted bundled processors |
| `dsp tests/fuzz.rs::plugins_survive_random_params` | Replaced by `removed_processors_are_never_instantiated` | Nothing to process; verifies ids are never instantiable and metadata is sane |
| `dsp lib tests::built_ins_have_no_state_or_editor` | Replaced by `removed_builtins_have_metadata_only` | Same reason |
| `dsp tests/offline_meters.rs::apply_plugin_compensates_latency` | Uses a local deterministic latency/tail fixture instead of the removed pitch shifter/reverb | `apply_plugin` service coverage kept |
| `mix`/`playback` tests using `dsp::create("gain")` as a stand-in | Use the test-only `test:gain` provider (settable `gain` dB) | Same host-mechanics coverage |
