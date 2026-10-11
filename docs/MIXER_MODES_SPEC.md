# Mixer modes — revision 1.6
Status: PLANNED. Latest owner instruction replaces the provisional SML Console algorithm with dedicated Summing/Tape slot infrastructure. Actual summing and tape models are DEFERRED BY OWNER to a separate research pass. No tanh model, oversampling factor, drive range or proprietary emulation is approved for implementation in these four sprints.

## M01 — independent profile and slots
Project profile Standard / Logic-style remains. Bus/Main get a dedicated Summing insert slot. Audio tracks get Tape; buses/Main get Master Tape. Read MIXER_SLOT_SPEC.md for exact positions, lifecycle and plugin/extension distinctions. Ordinary user insert chains remain available. Defaults: Clean summing and no Tape. Console/Visual/Split views share all state. Do not expose nonexistent model controls as working features.

## M02 — Logic-style profile

Use f64 accumulation at bus/main sum boundaries in both profiles, converting at existing host/output boundaries as needed. Do not require every external plugin to process f64. Version the mixer graph/arithmetic/panner behavior. Existing projects without a version are loaded through preserved legacy semantics until explicit Save As/migration; never infer identity after changing order or precision. If legacy execution cannot be preserved, report that compatibility gap and block original-sound claims; keep source/state/media intact.

Expose mono Pan, stereo Balance, and stereo Pan (independent left/right positions with linked width/center controls). Stereo balance attenuates the opposite side without moving its content across channels; stereo pan routes each input side through its own mono panner. No automatic stereo widening or saturation.

Initial pan-law choices: 0 dB, -3 dB and -3 dB compensated. Default Logic-style preset: -3 dB, mono Pan/stereo Balance; this is OUR chosen preset, not a claim about Logic defaults. Show the stereo-balancer compensation setting explicitly, initially off. Additional -4.5/-6 choices are deferred and must not be advertised as implemented.

For our provisional mono interpolation, p is in [-1,1], u=(p+1)/2. The -3 law uses (cos(pi*u/2),sin(pi*u/2)); the compensated variant multiplies by sqrt(2). The 0 law uses unity on the nearer side and cos(pi*abs(p)/2) on the opposite side. These give the documented endpoints/center, with the equal-power center approximately -3.0103 dB. They are original interpolation choices, NOT established Logic curve equivalence. Publish exact implemented coefficients and test intermediates. Uncompensated Balance uses the 0-law gains independently on L/R, no crossfeed. With compensation enabled use the selected mono-law gain pair independently. Stereo Pan processes each side through the mono law and sums; correlated sides can increase level, so meter honestly. Identity-centered stereo buses/Main use the B01 neutral override, avoiding repeated compensated gain. For a silent coefficient, compare exact zero/absolute residual rather than undefined dB at -infinity. No hidden normalization.


## M03 — extension points, not a prescribed sonic model
Implement the clean bus accumulator and slot processing API per SL01–SL06. Reserve source-edge plus bus hooks for future integrated summing without applying invented coloration. A normal stereo plugin in the Summing slot processes the already summed bus only; it cannot be advertised as a distributed console model. Audio-track Tape and Bus/Main Master Tape have distinct graph locations. Native summing/tape models require a later approved DSP contract before implementation.

## M04 — latency and recording safety
Shared processing lifecycle includes model/plugin latency, parallel path compensation, off-callback graph construction and safe bypass. Preserve dry capture before mixer processing. Explicit Low Latency Monitor may bypass affected hosted processors only with visible indication; print/export behavior is separately declared. Never silently reduce quality. Playback/export use the same configured graph. Do not infer model sound quality from slot plumbing tests.

## M05 — persistence and dual-view integration
Stable slot IDs, provider/model versions, position, bypass, parameter/state blobs and error states survive undo/reload. Unknown providers stay named missing placeholders. Never silently substitute a different processor. Both mixer views use the same slot inspector. Native Stereo Width follows VISUAL_MIXER_SPEC.md; its panner-spread distinction is unchanged. Visual A/B/C mix snapshots exclude slot assignments and processor settings explicitly.

## M tests — current scope and owner deferrals
- M-T1: retain clean deterministic sum/cancellation/64-input f64-reference fixtures; peak error <=1e-6 for normalized output <=1 with documented f32 boundaries; no intermediate hard clipping.
- M-T2: retain pan endpoints/center within 0.01 dB of specified coefficients, sweeps, stereo isolation, automation and save/reload. Exact Logic matching remains unverified.
- M-T3: nonlinear model transfer/harmonics/IMD — DEFERRED BY OWNER, pending actual model contract.
- M-T4: model anti-alias/oversampling quality — DEFERRED BY OWNER; former fixed 4x/tanh prescription withdrawn.
- M-T5: run slot/host impulse alignment <=1 sample relative error through compensated parallel paths and deterministic real-time/offline residual <=1e-5; SL-T tests provide fixtures.
- M-T6: persistence/migration/unknown provider and dry capture identity with slots configured; see SL-T4/5.
- M-T7: existing 16-track CPU/xrun/UI gates with empty slots and known host test processors. Label fixtures; never call them tested summing/tape sound models.
- M-T8: owner sonic audition of future summing/tape — DEFERRED BY OWNER. Current mixer usability/recording H gates still apply.

Current A3 requires M-T1/2/5/6 plus SL software gates; M-T7 is a native H3 performance gate, not deferred model tests. Current A4 repeats available target checks. Deferred sonic tests do not count as implementation failure for this infrastructure-only milestone and cannot support a claim that analog sound is finished. Evidence: clean-mix-tests.log, pan-matrix.csv and slot evidence named in MIXER_SLOT_SPEC.md. No runtime tests executed by this package.
