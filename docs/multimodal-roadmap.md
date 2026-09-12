# Multimodal Roadmap

## Why This Exists

`QunMind` is still a WeChat-group AI hub first. But the project will likely need
multimodal inputs later, especially when daily reports or alerts should come
from:

- livestreams
- recorded videos
- screen captures
- visual dashboards
- image-heavy social posts

The goal is to preserve that context now without pulling a heavy video runtime
into the current message-and-report core.

## Reference Project

- JoyAI-VL-Interaction
  - code: https://github.com/jd-opensource/JoyAI-VL-Interaction
  - model: https://huggingface.co/jdopensource/JoyAI-VL-Interaction-Preview
  - dataset: https://huggingface.co/datasets/jdopensource/JoyAI-VL-Interaction

## What Is Worth Learning From It

- real-time video-language interaction
- deciding when to speak, stay silent, or delegate
- multimodal agent runtime design
- event extraction from continuous visual streams
- a practical split between model runtime and product surface

## What Not To Do Right Now

- do not embed a heavy video inference stack into `QunMind` main process
- do not block current wx-cli / report / publisher progress on multimodal work
- do not mix livestream processing concerns into scheduler or bot handler logic

## Recommended Architecture

### In QunMind

- accept multimodal summaries as external signals later
- ingest visual-event summaries as structured text or source items
- keep WeChat reply, storage, and report orchestration unchanged

### Outside QunMind

- video capture / stream ingestion
- multimodal inference runtime
- ASR / TTS / live commentary loops
- GPU-heavy deployment and latency control

## Practical Near-Term Path

1. Treat multimodal as a separate PoC track.
2. Reuse results, not runtime, inside `QunMind`.
3. Start with one narrow use case:
   - livestream monitoring summary
   - screen/dashboard watcher
   - image-to-daily-report material extraction
4. Only integrate back after the PoC can emit compact structured events.

## First Integration Shape

If a multimodal PoC becomes useful, the first safe bridge back into `QunMind`
should be one of:

- a `PublicNewsSource`-like adapter for visual-event summaries
- a future `Connector` that emits normalized events
- a daily-report material feeder that outputs text + links + timestamps
