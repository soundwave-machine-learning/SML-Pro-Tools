# SML Studio documentation

Documentation for SML Studio (the SoundCraft engine), version 0.5.0. Start with the manual.

## For musicians and engineers
| Document | What it is |
|---|---|
| [USER_GUIDE.md](USER_GUIDE.md) | **The manual** (the one authoritative user document): setup, recording, editing, Nudge, mixing, plugins, mastering, phone remote, assistant, recovery, five professional walkthroughs, known issues. |
| [COMMAND_REFERENCE.md](COMMAND_REFERENCE.md) | Every command of this build: label, menu, chords in both shortcut profiles, parameters, undo, risk, status, limits. Generated from the program. |
| [reference/SHORTCUTS.md](reference/SHORTCUTS.md) | Both shortcut profiles, including chords that are shadowed. Generated. |
| [reference/CAPABILITIES.md](reference/CAPABILITIES.md) | Stated limits, disabled menu entries, external control interfaces. Generated. |

## For an external AI operator (Grok bot) or automation
| Document | What it is |
|---|---|
| [grok/GROK_OPERATOR_GUIDE.md](grok/GROK_OPERATOR_GUIDE.md) | How an external operator works safely with SML Studio: build check, tool access, verified commands, preserving media and undo, measuring before claiming, recovery. |
| [grok/GROK_BOT_INSTRUCTIONS.md](grok/GROK_BOT_INSTRUCTIONS.md) | The same rules as a short instruction block to paste into the bot's settings. |
| [reference/commands.json](reference/commands.json) | Machine-readable command and capability data. |
| [control-protocol.md](control-protocol.md), [mcp.md](mcp.md) | The local control channel and the MCP server. |

## Keeping documentation true
| Document | What it is |
|---|---|
| [DOCS_MANIFEST.json](DOCS_MANIFEST.json) | Which source and builds this documentation describes, what was verified how, what is pending, and which source files each chapter depends on. |
| [DOCS_CHANGELOG.md](DOCS_CHANGELOG.md) | Documentation history. |
| [DOCS_MAINTENANCE.md](DOCS_MAINTENANCE.md) | The update procedure tied to code changes and packaging (`cargo xtask docs`, `cargo xtask docs-check`). |
| [STYLE_GUIDE.md](STYLE_GUIDE.md) | Interface style guide for contributors: semantic colour tokens, typography, spacing, control states, workspace patterns. |
| [polish_pass_01/POLISH_REPORT.md](polish_pass_01/POLISH_REPORT.md) | Interface polish pass 01: what changed per workspace, tests, before/after renders, gaps for native review. |
| [refinement_pass_01/REFINEMENT_REPORT.md](refinement_pass_01/REFINEMENT_REPORT.md) | UI refinement pass 01: Visual labels and coincident channels, mixer density, compact Record toolbar, empty input meters. |
| [refinement_pass_01/REFERENCE_AB_DEFECT.md](refinement_pass_01/REFERENCE_AB_DEFECT.md) | **Open defect K1**: reference A/B plays no audio — source path and reproduction. |
| [manual_archive/](manual_archive/) | Earlier manual editions exactly as compiled into released builds. |
| [hotfix_pass_01/HOTFIX_REPORT.md](hotfix_pass_01/HOTFIX_REPORT.md) | Hotfix Pass 01 (Split divider, + Add Track, control bar with BPM/Loop/Audio Setup): what changed, tests, evidence, pending Windows checks. |

Development and planning documents (specifications, sprint contracts and reports, parity
assessments) are listed in the repository's `docs/DOCUMENT_INDEX.md` and `ROADMAP.md`.
They describe how the software was built, not how to use it; when they disagree with the manual
about current behaviour, the manual (checked against the code) applies.
