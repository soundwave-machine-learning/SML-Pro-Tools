# Signal flow as implemented (graph version 2) — SL02

Source: `crates/mix/src/lib.rs` (`process_strip`, `route_strip`, master section). Inspect any
channel at runtime with `mix.signal_flow {track}`. Tests: `cargo test -p soundcraft-mix sml_graph`.

**Audio/instrument track**: source (comp, clip gain/fades, edits) → utility trim/polarity →
**Tape** slot (+ slot trim) → user inserts → **Width** (exact pass-through at 100 %) →
pre-fader send tap → fader/mute → post-fader send tap → output panner → output route.

**Bus / FX return (Aux)**: incoming contributions (f64 accumulation) → **Summing** slot (+ trim)
→ utility trim/polarity → **Master Tape if Pre** → user inserts → Width → pre-fader send tap →
fader/mute → **Master Tape if Post** (default for buses) → post-fader send tap → output panner
(neutral full-width bus = exact identity) → output route.

**Main (Master fader)**: summed mix (f64) → Summing → **Master Tape if Pre** (default on Main) →
inserts → fader → Master Tape if Post → output (stereo identity, no extra pan stage).

**Sends**: branch from pre-pan taps; each applies its own gain/mute and exactly one panner —
Follow copies the channel's main-output coefficients, Independent uses the send's own position.

**Live capture**: taps the device input in the input callback before any of the above (dry).

Differences from SL02 stated plainly:
- User inserts are all *pre-fader* in this build; there is no separate "user Post inserts"
  section yet, so "Master Tape Post" sits directly after the fader. (PARTIAL)
- Aux/bus latency used for compensation is measured from the previous block (stable after the
  first block) — inherited engine behaviour.
- Position/provider changes are refused while playing (stopped-state rule); trims/bypass are live.
- Legacy (version 1) sessions keep the upstream order/f32 sums/send policy until
  `mix.graph_migrate`.
