# Mixer sound research — revision 1.6


Research date: 2026-10-11. Documentation research only. No native comparison, audio audition, implementation or benchmark has been performed.

## Findings and limits
Apple documents a 64-bit summing engine [L1]. That describes arithmetic precision; it does not establish a fixed EQ, saturation curve or audible superiority. Apple also documents selectable pan laws [L2] and different mono pan, stereo balance and stereo pan behavior [L3]. These can change levels and stereo placement. Bounce normalization is an explicit option [L4]. No cited source establishes a universal hidden “Logic warmth” processor in clean summing.

Engineering inference: reports that Logic sounds different need controlled comparison of source samples, routing, channel format, pan settings, gains, automation, plugins, time processing, normalization, dither and sample-rate conversion before attributing the difference to summing. No matched files or settings from the owner's Logic sessions have been supplied. The proposed mode reproduces a defined set of documented behaviors, not an unidentified sonic signature.

UA documents optional Neve and API summing extensions on buses and Main [U1,U2], with nonlinear circuit-model behavior, gain staging and integrated controls. UA also describes source/bus pan-law and fader-taper changes. These are explicit processing features. Claims of hardware authenticity are UA's claims, not independently verified here. Public manuals do not supply algorithms sufficient to reproduce these proprietary models. Summing is distinct from LUNA's separate tape processing.

Current decision: a Logic-style linear mix profile plus independent selectable Summing/Tape slots. Native analog models are deferred by owner. No proprietary algorithm or imported UA/Apple assets are supplied; no exact sonic compatibility claim. Model choice does not silently alter pan laws or stored gains.

## Primary sources
- L1 Apple, Logic Pro technical specifications: https://support.apple.com/en-la/111899 (lists 64-bit summing; do not infer all plugin processing is 64-bit).
- L2 Apple, General Audio project settings, guide 12.3: https://support.apple.com/guide/logicpro/general-project-settings-lgcp4f230784/12.3/mac/15.6 (pan-law options and stereo-balancer compensation).
- L3 Apple, pan/balance positions, guide 10.7: https://support.apple.com/guide/logicpro/set-channel-strip-pan-or-balance-positions-lgcpbc21a438/10.7/mac/11.0 (panner distinctions; version deliberately pinned).
- L4 Apple, bounce regions in place: https://support.apple.com/en-uz/guide/logicpro/lgcp8ae5826e/mac (normalization options; not a statement of universal defaults).
- U1 UA, Neve Summing manual, updated 2025-09-10: https://help.uaudio.com/hc/en-us/articles/360041911771-LUNA-Neve-Summing-Extension-Manual
- U2 UA, API Summing manual, updated 2025-09-10: https://help.uaudio.com/hc/en-us/articles/360052390811-LUNA-API-Summing-Extension-Manual

## Reference experiment (pending)
On a recorded Logic version, import identical PCM files at native project rate. Disable plugins, normalization, dither, stretching, master processing and automation; match channel format, pan laws and all route gains. Export floating-point PCM if available. Align exact sample starts and compare first without compensating unexplained gain, then report best-fit gain separately. Use mono center/hard pan/intermediate positions, independent stereo L/R impulses, correlated and uncorrelated multitrack fixtures. Report peak/RMS residuals and spectra; numerical differences alone are not proof of audible difference. Level-match blind listening separately. If a residual remains, isolate one setting at a time. Console-enabled renders are deliberately nonlinear and must not be expected to null against clean Logic.

A LUNA comparison requires owner-accessible licensed extensions and recorded settings; it is optional research, not permission to purchase or a completion prerequisite for current slot infrastructure. No matching claim without evidence.
