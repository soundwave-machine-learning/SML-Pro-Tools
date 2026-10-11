# LUNA and Harrison workflow research — revision 1.6
Date: 2026-10-11. Primary documentation only; no installed-app comparison or sound measurements. References intentionally pin Mixbus32C v4/v9 behavior rather than claiming a latest-product survey.

## Documented reference behavior
LUNA: integrated Summing selectors on buses/Main and dedicated track/master Tape slots are distinct from ordinary inserts [U1]. Multitrack tape can share machine settings with channel-specific controls. The master-tape extension offers Pre/Post placement with Pre default on Main and Post on buses [U2]. Its utility row sits after summing and before downstream processing; category processing bypass and spill are documented [U3]. A LUNA extension is not necessarily an interchangeable VST3/CLAP plugin.

Harrison Mixbus32C v4 documents independent send panning, bus width, mixbus/VCA spill, and VCA control of assigned channels [H1]. These distinguish changing a control group from routing audio through a subgroup. Version 9 describes integrated EQ, compression, gate/expander, input Drive, bus HP filtering, two tape-saturation colors and master dynamics [H2]. Its visible reduction indicators are processor measurements, not ordinary peak meters. Manufacturer sonic descriptions are not independently verified here.

## What we had missed / decision matrix
| Behavior | Existing plan | Decision |
|---|---|---|
| Dedicated selectable Summing and Tape rows | Fixed provisional console block; no Tape | Build real slot architecture now; defer native sound models |
| Master Tape Pre/Post placement | Absent | Explicit switch and versioned defaults; actual tap order documented |
| Group machine configuration with per-channel state | Absent | Reserve schema now; detailed machine UI/model controls later |
| Independent send pan | Generic sends only | Add Follow/Independent with tests |
| Bus spill | Source reveal/collapse existed | Strengthen exact view-only behavior in both mixers |
| Input/output gain staging and processor meters | Partial | Explicit meter taps/trim; no fake saturation meter |
| Category bypass for comparison | Individual bypass | Summing/Tape/Inserts/All override with state restoration |
| VCA faders | No explicit contract | Candidate later: controls levels without adding audio bus processing |
| Integrated EQ, gate, compressor, channel Drive, bus HPF, master limiter | Bundled catalogue deliberately removed | Document for later; use owner's plugins now |
| Monitor-only controls/cue workflow | Basic monitoring exists | Audit existing implementation; propose dedicated dim/mono/cues later |

Recommendations are engineering judgments. Bus spill, width and controls in strips reinforce ease of use without reproducing Harrison algorithms. VCAs would help ride all vocals while preserving subgroup topology. Monitor-only volume/dim/mono must never change an exported mix. Those candidates do not silently expand this pass.

## Scope correction
The owner has explicitly postponed actual summing/tape processors. The earlier tanh-based SML Console, its Drive scale and fixed oversampling proposal are withdrawn, not an approved emulation. Keep Clean mathematical summing, compatible user-plugin slots and future native provider interfaces. Full native model research needs topology/calibration, parameter units, stereo strategy, oversampling/aliasing, latency, state/history, automation, bypass and measured/listening acceptance. No model-specific pan-law/fader taper changes until their effect on existing sessions and automation is specified.

## Primary sources
- U1: UA, Mixing with LUNA Extensions, updated 2025-12-30: https://help.uaudio.com/hc/en-us/articles/44791240323860-Mixing-with-LUNA-Extensions
- U2: UA, LUNA Ampex ATR-102 Master Tape Extension Manual: https://help.uaudio.com/hc/en-us/articles/360048827272-LUNA-Ampex-ATR-102-Master-Tape-Extension-Manual
- U3: UA, Mixing in LUNA, updated 2026-01-13: https://help.uaudio.com/hc/en-us/articles/360041465732-Mixing-in-LUNA
- H1: Harrison staff, Mixbus32C v4 release explanation, 2017-05-01: https://forum.harrisonconsoles.com/thread-4486.html
- H2: Harrison staff, Mixbus32C v9 release explanation, 2023-03-22: https://forum.harrisonconsoles.com/thread-11251.html

The attempted full v5 PDF fetch failed; no findings depend on unseen PDF content. Only the staff announcements, not user forum sonic opinions, support the Harrison rows. No UA/Harrison model or preset compatibility promised.
