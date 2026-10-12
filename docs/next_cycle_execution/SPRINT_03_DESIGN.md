# Sprint 3 design dossier — plugin hosting hardening (R23–R31)

Written 2026-10-12 after the Sprint 2 close-out, before the Sprint 3 code. Decisions about this
repository; statuses stay in `REQUIREMENT_STATUS.csv`.

## What the host is today (inventory, task 1)
- Formats: CLAP (in-tree fixture `org.soundcraft.test.gain`/`.synth`: params, stepped enum,
  hidden param, latency 3, state, floating GUI stub, note port), VST3 (upstream fixture tests in
  `crates/vst3-host`; editors hosted on macOS only; aux buses kept inactive = no sidechain; no
  64-bit processing; no IMessage), AU (macOS stub). Deterministic providers `test:gain:<dB>`,
  `test:delay:<n>` (mix fixtures). The seven SML plugins: no artifact supplied →
  AWAITING_EXTERNAL_PLUGIN, each with its interface record.
- `Plugin` trait (`soundcraft_dsp`): prepare/reset/params/process/latency/tail/gain
  reduction/notes/state/editor. No sidechain input, no per-block transport/tempo context, no
  "quality" switch. Offline renders (`render_range`) create instances through `create_plugin`
  (missing third-party → pass-through) with `missing_plugin_guard` on bounce commands.

## Decisions
1. **Capability matrix** = a query command `engine.plugin_capabilities {id?}` built from the
   real registries (format, version, audio layouts, sidechain support, parameter count/units,
   editor availability on this platform, state support, declared latency/tail) and written to
   `docs/evidence/next_cycle/sprint_03/host-capability-matrix.csv` by a test. Fixture rows are
   labelled "deterministic host fixture"; no real native plugin row is asserted from P.
2. **Sidechain (R24/R25/R29)**: model `Insert.sidechain: Option<Route>` (serde default None,
   ids untouched); `Plugin::set_sidechain(&mut self, buffers)` default no-op; the mixer fills a
   per-strip side buffer from the named bus/track tap (post-fader, at the insert's node) before
   `process`; `test:sidegate:<dB>` fixture gates by the side input so the routing is provable with
   impulses; VST3 aux-bus activation stays "not hosted" (recorded, not faked) until a real plugin
   with an aux bus is available to test on the target.
3. **Offline render hardening (R31)**: a bounce refuses while a Bend render is pending
   (`warp.is_busy`) and when a required hosted plugin failed to instantiate (no silent
   pass-through master): `render_range` reports instantiation failures; `file.bounce_*` turn
   them into an error with the plugin id unless `allow_missing: true` is passed explicitly.
   Tail handling: render `len + latency + max tail` then trim to the requested range plus an
   explicit `tail` option. Determinism: fixtures with 0/1/63/128/1024-sample delays align within
   one sample (existing PDC tests extended to nested/parallel paths and bypass).
4. **E03 compatibility mode (R31, task 6)**: `sandbox:<plugin id>` provider = the real CLAP/VST3
   plugin hosted in a `soundcraft-cli plugin-worker` child process, one block ahead over pipes
   (declared added latency = one block), silence-and-flag on crash/hang, automatic restart with
   the last state, dry capture untouched (capture is before inserts). Opt-in per insert, never
   default, labelled in the latency report. Measured: added samples, callback deadline misses,
   memory, in P; native N pending.
5. **Bend hardening (R30, task 7)**: `clip.warp_cancel`, re-render of warped clips whose
   derived source is missing after reopen (derived sources are saved with the session as media;
   a missing one re-queues from the original), undo/redo keeping anchors + content consistent
   (already), bounded queue (already), stale rejection (already), immutable media (already).
6. **Host defects (task 2)**: scan on menu open already impossible (cache); editor focus/close/
   unload/reload are covered by the fixture GUI stub portably; native editor behaviour is N.
7. **W1 load (task 9)**: `sml_profile` chain with the fixture CLAP plugin + autosave + menu
   activity as a headless control; native remains pending.
