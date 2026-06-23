# Multi-Platform Publishing Boundary

## Goal

Keep `QunMind` focused on WeChat-group intelligence orchestration while making
daily-report publishing reusable across multiple external platforms.

## Decision

`QunMind` should own:

- report generation
- publish scheduling
- publish target configuration
- publish readiness diagnostics
- publish result / failure visibility

A dedicated publisher project or subservice should own:

- platform authentication
- platform-specific rendering
- media upload
- review / moderation polling
- retry / backoff around external platform APIs
- account-risk isolation for non-official or browser-automated platforms

## Why

The current product center is still the WeChat group AI hub. If platform
publishing logic grows inside `QunMind`, the project will quickly accumulate:

- account and token management for unrelated platforms
- image/video/template rendering concerns
- moderation-state polling and retry logic
- high-risk browser automation or cookie-based flows

That would blur the existing `Channel -> BotHandler -> AiClient -> report`
architecture and slow down real WeChat validation.

## Current Shape

Inside `QunMind`, publishing now sits behind a generic Rust boundary:

- `src/publisher.rs`
- `publish_markdown(markdown, PublishTarget)`

Current implemented target:

- `PublishTarget::WechatDraft`
  - delegates to local `moonpub`
  - current command path:
    `moonpub --articles <dir> push <temp_markdown_file> --render`

This keeps the scheduler responsible for "when and what to publish", while the
publisher boundary handles "how this target accepts the content".

## Recommended Split

### Inside QunMind

- decide when a report should be generated
- build the report content
- choose a logical publish target
- expose readiness blockers in `doctor`
- persist or emit publish receipts later

### Outside QunMind

- WeChat public-account draft publishing implementation
- Douyin publish implementation
- future Xiaohongshu implementation
- platform-specific media/template rendering
- operator review queue and failure retry dashboard

## Platform Notes

### WeChat public account

- Current route is acceptable as a draft-delivery adapter.
- Best near-term fit for article-style daily reports.

### Douyin

- Prefer official OpenAPI-based publishing where available.
- Better treated as a separate renderer + publisher because the target content
  shape is image/video-first, not markdown-first.

### Xiaohongshu

- Do not assume a stable general-purpose public publish API.
- Treat as manual / semi-automatic / wait-for-official-API by default.

## Near-Term Plan

1. Keep `QunMind` publisher boundary narrow and testable.
2. Add publish receipts / status recording after successful sends.
3. Add target-specific readiness diagnostics before runtime.
4. Start a dedicated publisher project when WeChat + Douyin dual publishing
   becomes a real delivery goal.
