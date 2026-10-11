# Dedicated Summing and Tape slots — revision 1.6
Status: PLANNED. Current goal is functioning mixer architecture and host-backed slots. Native summing/tape sound design is reserved for the next owner-directed pass. This document is authoritative over earlier fixed SML Console requirements.

## SL01 — slot types and availability
| Channel | Dedicated slots | Default |
|---|---|---|
| Audio track | Tape | None |
| Audio bus / FX return | Summing; Master Tape | Clean; None |
| Main | Summing; Master Tape | Clean; None |

Each strip has click-to-select slot, provider/model name, enable/bypass, remove/replace, open controls, input/output meter readouts and latency. Support compact and expanded rows consistently across selected channels. Ordinary user inserts remain separate. No restoration of upstream bundled effects/instruments. Existing supported external VST3/CLAP hosting may populate these slots; no new plugin format or proprietary LUNA extension loader. The browser labels installed user plugins versus future native extensions. Do not list unimplemented branded models or imply ownership of a plugin license makes its LUNA extension portable.

## SL02 — fixed identity and explicit signal flow
Original SML graph contract, informed by vendor workflows; not a claim of exact internal reproduction. Signal flow diagram/readout must display actual order, including user insert positions and sends. Defaults are versioned and saved.

The playback source stage includes comp, clip gain/fades and existing time edits. Live capture taps selected physical input before that playback stage and before all mixer processing. These are separate inputs to the mixer source selector; never apply playback clip processing to a newly captured file.

Audio strip: source -> utility trim/polarity -> Tape plus slot trim -> user Pre inserts -> Width -> pre-fader send tap -> fader -> user Post inserts -> post-fader send tap -> main-output panner -> output route/mute.

Bus/Main: incoming contributions [future native source hooks, identity now] -> f64 sum -> Summing provider plus slot trim -> utility trim/polarity -> Master Tape plus slot trim if Pre -> user Pre inserts -> Width -> pre-fader send tap -> fader -> user Post inserts -> Master Tape plus slot trim if Post -> post-fader send tap -> main-output panner (Main stereo identity) -> output route/mute.

Each send branches from a PRE-PAN tap, applies its own gain/mute and exactly one panner: Follow copies channel panner coefficients; Independent uses its saved panner values. Thus post-fader means after fader and declared downstream processing, not after the main-output panner. This avoids trying to undo an already panned/mixed signal. Main-output and send panning never process the same route twice. Post-insert placement relative to pan is an explicit SML choice, not a claim about another DAW. B policy owns mute/solo and legacy behavior. External insert channel-format changes must validate downstream panner/width mappings; reject unsupported layouts rather than discard channels.

Master Tape defaults: Post on buses, Pre on Main, following the LUNA default concept. In this SML graph Pre means before user Pre inserts, allowing a user limiter afterward. Track Tape stays before user inserts. Provider choice never changes slot position. Each plugin instance has one location; moving Pre/Post preserves its ID/state/automation and prepares latency changes outside the callback.

Position/format/graph changes apply while stopped in this pass. Attempting them during playback presents a clear stopped-state action; no silent relocation. Continuous supported parameters and latency-preserving bypass work live. Native Width is after user Pre inserts and before user Post inserts, exactly once. CL03/04 owns tracking deactivation/latency transitions.

Exports: Raw Source and Edited Dry are defined in VOCAL_EDITING_SPEC.md V13. Processed Track/Bus uses the channel final output before destination bus processing unless include-destination processing is explicitly selected and shown. Full Mix uses Main output after all processing and before monitor-only attenuation. Tail range is explicit. Do not call bypassing inserts equivalent to Edited Dry. No guaranteed sum-of-stems reconstruction with shared/nonlinear processing.

## SL03 — two kinds of summing provider
1. Native extension API: reserve per-source-edge processing plus a destination summing/bus stage. Pin stable route IDs; never process an edge twice. Provider declares whether source stages are used, format/rate limits, latency, state/version and parameters. Clean identity provider implements no coloration. Source-stage and bus-stage latency both enter compensation. Nested buses intentionally traverse each enabled destination once.
2. User plugin adapter: compatible ordinary plugin sees one summed mono/stereo bus buffer through the existing host. Label “Bus processor” in provider details. Never pretend it can inspect all original tracks or model their individual nonlinearities. No automatic hidden plugin instantiation on source tracks.

Construct host/provider lifecycle, graph buffers and state outside the audio callback. Bounded commands, no callback allocation/locks/file I/O, correct teardown and missing-provider handling. No summing selection changes fader gain or pan law in this pass. Future model-specific taper/pan laws require an explicit versioned migration and automation mapping contract; preserve Logic-style independence now.

## SL04 — Tape provider contract and groups
User tape plugin adapter uses an independent existing host instance per slot. Native tape API reserves global machine settings plus independent per-channel state. A Tape Group can copy/link approved settings across channels but never shares audio buffers, delay lines or nonlinear history. Store global machine ID separately from channel ID. User plugin linking is limited to explicit mapped parameter IDs; unknown controls remain editable in the plugin window, not guessed from parameter names. Empty slots at unity host trim are exact pass-through with zero added latency. Non-unity trim is visible processing even with no provider; removing a slot resets its host trim to unity in one undo transaction. Clean native identity defaults unity.

This pass implements slot hosting, state, position and group-ready schema. Four-machine global UI, speed/formula/bias/noise/drive semantics, built-in tape calibration and model controls are deferred to the actual tape research pass. No fake knobs. Users can use their own compatible installed tape processors now without waiting for a native model.

## SL05 — controls, gain staging and category bypass
Expose utility input trim and polarity as native services, not bundled plugins. Inspect existing values/ranges before adding controls. Identify Summing input/output, Tape input/output, pre-fader and post-fader meter taps in the expanded inspector. Numerical peak levels are always meaningful; a saturation or gain-reduction meter is shown only when the provider exposes a defined measurement. Do not relabel peak level as saturation.

Provider parameter metadata determines available controls. Do not force one Drive/Headroom scale onto every future model or infer unknown plugin semantics. Manual post-provider trim permits level matching without changing input drive. Keep fader and processor input drive independent and label their positions. Slot output trim defaults unity and belongs to host state; its changes use existing gain smoothing. Slot bypass bypasses provider plus its trim together; retain the allocated delay for compensation. Zero-latency identity paths bypass arithmetic. Do not substitute a plugin internal bypass of unknown latency for this host contract.

Add separate reversible audition bypass controls: Summing, Tape, User Inserts, All. Preserve each slot's individual bypass state under the temporary override, restore exactly, show the override in Console and Visual, and maintain compensation during playback. These are audition overrides: save normal states and reset overrides on reopen. BOTH real-time and offline exports use normal states, with a visible notice when monitoring differs; explicitly printing the audition requires a separate recorded choice. The same rule applies to temporary Tracking deactivations: export uses the saved normal graph, identifies differences, and never silently writes monitoring overrides. Maintain dry capture identity. “Slot infrastructure ready” never means “native console/tape sound ready.”

## SL06 — Harrison-inspired workflow integration
Promote existing bus reveal into explicit Spill: show bus plus feeding channels, including sends; distinguish direct and send contributions. Restore prior visibility on exit and handle nested buses deterministically. Console and Visual share the selected spill root; no rerouting/muting to implement visibility.

Add independent send pan/balance with Follow Channel / Independent mode. Follow reads existing panner behavior; Independent overrides it only on that send. Reuse M02 coefficients and channel formats. Sends carry one pan operation, never the main pan followed by another unintended pan. Audit pre/post taps for this requirement. Add clickable routing/membership and one-click bus creation using existing B commands. Keep width already specified in VM03 and show slot controls inline without requiring a separate window for simple host controls.

VCAs, monitor-only dim/mono/alternate speakers, additional cue/foldback systems and Harrison-style built-in EQ/gate/compressor/drive/limiter models are research candidates, not new mandatory implementations in this six-hour pass. Retain existing working features. Do not reinstate removed processors under a new name. Their benefits and distinctions are recorded in LUNA_HARRISON_WORKFLOW_RESEARCH.md.

## SL-T acceptance
Sprint 1 establishes real commands and harnesses. Use test-only providers: identity, known gain, known sample delay and deterministic affine processor; fixtures must never appear as shipping sonic models.
- SL-T1: empty/Clean slots with unity trims and identity Width are sample-identical to the same graph/profile baseline for deterministic fixtures; open/close/Console/Visual switching does not change audio; each loaded provider invoked once at its specified node.
- SL-T2: use noncommuting affine/gain fixtures to distinguish Pre/Post order, not only two gains. Verify track Tape before inserts, bus Post/Main Pre defaults, move/reload, sends and exports against explicit reference. Reordering stopped only; no duplicate state/processing. Include hard-left main pan with independent hard-right send, a one-sided stereo source and noncommuting Post insert so a post-pan shortcut cannot pass.
- SL-T3: known-delay providers on source hooks, bus and tape through parallel/nested routes: <=1 sample relative alignment; offline/realtime deterministic residual <=1e-5. Bypass preserves active compensation; empty identity adds zero delay.
- SL-T4: stable IDs/provider version/blob/position/bypass/trim across save/reopen/undo, missing plugins, replace/remove, UI teardown. Tape group schema preserves separate channel histories. Category audition overrides restore original bypass states and cannot change exported normal state.
- SL-T5: injected dry-capture PCM identical with active slot fixtures versus empty slots; monitor bypass visibly reports differences. Export test confirms final processing included and monitor attenuation excluded. Pending hardware capture remains explicit.
- SL-T6: existing real supported host fixture loads, edits, serializes, reopens and renders in each allowed slot. Plugin compatibility and personal-plugin native H2 tests remain; fixtures prove plumbing, not universal compatibility.
- SL-T7: Spill/exit preserves exact audio and prior visibility. Independent send pan passes isolated L/R and center tests within 0.01 dB of defined coefficients; Follow mode tracks main behavior without double pan; output routing unchanged.
- SL-T8: run 16-track UI/callback/xrun gates with empty slots, delay/gain fixtures and available real plugin at declared settings. Report counts, quality, CPU and native-versus-headless status; zero xruns in target run. No invented analog-quality score.

A3 software requires SL-T1..7, M current tests and prior B/VM/host checks. A4 repeats SL-T8 and native lifecycle/workflow tests where possible. Required evidence: slot-signal-flow.md, slot-order-tests.log, slot-state-tests.log, slot-latency.csv, slot-host-matrix.csv, slot-performance.csv, spill-send-tests.log. Missing actual native models is DEFERRED BY OWNER, not an infrastructure failure. Unimplemented slot behavior is PARTIAL. Never mark whole sonic product complete before the later model pass.

## Revision 1.6 timing dependency
CLOCK_LATENCY_SPEC.md CL03–04 resolves bypass versus low-latency tracking: ordinary slot/audition bypass preserves PDC; removing delay for tracking needs explicit stopped deactivation/rebuild, with visible restore state. Include all source hooks, slot/host buffers, buses and Main in the path ledger. No new native model DSP is authorized.
