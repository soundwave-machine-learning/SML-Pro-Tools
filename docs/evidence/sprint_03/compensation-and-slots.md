# Compensation and slot fixture results (CL-T4, SL-T1..T7, M-T1/2/5/6)

All from `cargo test -p soundcraft-mix sml_graph` (`slot-graph-tests.log`) and
`cargo test -p soundcraft-engine cmd::slots` (`slot-state-tests.log`):

- CL-T4/SL-T3/M-T5: delay providers 0/1/63/128/1024 samples in a track Tape slot vs an
  uncompensated track: impulse peaks align within ≤1 sample; bypassed slots keep the delay; a
  128-sample delay in a bus Summing slot vs a direct track aligns. (Serial/parallel/nested bus
  paths; sidechains are not supported by this engine → not tested.)
- SL-T1: empty/Clean slots, unity trims, 100 % width, identity provider, mixer view state →
  bit-identical render.
- SL-T2: affine(2x+0.1) Tape before a -6 dB insert; bus Master Tape Post vs Pre with a -6 dB
  fader: values match the SL02 order exactly; hard-left main pan with independent hard-right send.
- SL-T4: slot id/provider/bypass/trim/position across undo and save/reopen; missing provider
  placeholder; trim reset on removal; refusal while playing; audition never saved/exported.
- SL-T5 (dry capture identity with active slots): structural — capture happens in the input
  callback before the mixer (`playback::record`), so slot settings cannot reach the file; no
  separate hardware capture test was possible.
- SL-T7: Independent send pan isolation and -3.0103 dB centre within 0.01 dB; Follow equals the
  channel coefficients; one-sided stereo send without crossfeed.
- M-T1: 64 random inputs on a bus, peak error vs f64 reference ≤ 1e-6 (graph v2 f64 sums); +12 dB
  internal sum not clipped.
- M-T2: Logic-style laws (0 dB, -3 dB, -3 dB compensated) within 0.01 dB of published
  coefficients, exact zeros; balance without crossfeed; dual stereo pan; neutral nested buses add
  no gain under compensated laws.
- M-T3/M-T4/M-T8: DEFERRED BY OWNER (no native models). M-T7/SL-T8: native H3, not run.
