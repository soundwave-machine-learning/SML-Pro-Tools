# X03 — model / STT / TTS runtime discovery (Sprint 2 gate, started early in Sprint 1)

Date 2026-10-12, environment P (Linux x86_64 container, 4 vCPU, 15 GiB RAM, no GPU, no audio
device). Everything below was observed in this session (INSPECTED/RERUN), not assumed.

## What the environment can reach
| Source | Result | How observed |
|---|---|---|
| huggingface.co, cdn-lfs.huggingface.co, hf-mirror.com | blocked (egress policy, CONNECT 403/000) | curl through the agent proxy |
| ollama.com, download.pytorch.org, dl.fbaipublicfiles.com, openaipublic.azureedge.net (official Whisper weights) | blocked (CONNECT 403) | proxy recentRelayFailures |
| modelscope.cn, gitee.com, kaggle.com, zenodo.org, alphacephei.com | blocked | same |
| github.com / codeload.github.com (other repositories) | 403; GitHub API scoped to this session's repository | curl, api.github.com message |
| crates.io index + static.crates.io | reachable | curl 200 |
| PyPI, npm registry | reachable | curl 200/302 |
| Local disk | no model weights (only magika's file-type classifier), no espeak/flite/festival/piper/whisper/llama/ollama binaries | `find`, `which` |

npm carries third-party repackagings of permissively licensed weights (for example TinyLlama
1.1B GGUF split into packages, Kokoro-82M q8 ONNX shards) and the official PocketSphinx package
(maintainer-published, BSD-2-Clause). Downloading those packages was **denied by this session's
permission classifier** ("untrusted code integration"). That denial is respected; no other
registry was used to route around it.

## Decision
1. **Generative language model: BLOCKED in P.** No official weight host is reachable and the
   reachable third-party mirrors may not be fetched here. What ships instead is real plumbing,
   not a mock: a provider that talks to a *local* OpenAI-compatible endpoint
   (`/v1/chat/completions` on loopback — llama.cpp `llama-server`, Ollama and LM Studio all expose
   it, all no-cost and local), disabled by default, bounded and cancellable, whose output can
   only become a plan through the existing validated, atomic command service. Its tests use a
   loopback fixture server and are protocol/validation evidence only. Real inference stays
   BLOCKED until the owner runs a local model server (setup in TARGET_PENDING.md).
2. **Speech synthesis: built in, own code.** A rule-based formant synthesizer (English
   letter-to-sound rules, cascade formant resonators) written in this repo: no weights, no
   download, no GPL code. It produces real intelligible-but-robotic speech; quality is labelled.
3. **Speech recognition: built in, own code.** A command recognizer (log-mel/cepstral features +
   dynamic time warping against enrolled templates). The user enrolls each phrase once; it
   recognizes the enrolled command vocabulary, not free dictation. In P it is evaluated on
   synthetic utterances (produced by the synthesizer with varied voice parameters and noise),
   which is weaker evidence than human speech; physical microphone use stays H/M pending.
4. Speech output uses the existing monitor-only ring (never capture/export, silent while
   recording by default). No always-listening mode.

Freeze before scoring (RC06): the held-out evaluation set is written to
`evidence/sprint_02/heldout-eval.json` before any tuning, and its scores are reported per case.
