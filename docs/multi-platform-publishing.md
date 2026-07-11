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

## External Reference: AiToEarn

[`yikart/AiToEarn`](https://github.com/yikart/AiToEarn) is an active MIT-licensed
TypeScript content-marketing platform. Its useful reference is the separation
between a content system and a platform-facing publish / account service:

- MCP-facing channel operations
- per-platform publish providers and readiness checks
- asynchronous task-status handling around media work

It is not a dependency or an implementation template for `QunMind`. Its OAuth,
Relay, browser automation, account credentials, engagement automation, and
monetization workflow belong outside this repository. If multi-platform
delivery becomes a concrete product goal, evaluate it as a separate publisher
service that receives reviewed report artifacts from `QunMind`.

## Near-Term Plan

1. Keep `QunMind` publisher boundary narrow and testable.
2. Add publish receipts / status recording after successful sends.
3. Add target-specific readiness diagnostics before runtime.
4. Start a dedicated publisher project when WeChat + Douyin dual publishing
   becomes a real delivery goal.

The first persistence step now belongs inside `QunMind`: successful publishes
should record the receipt even before a full external publisher service exists.

The next local step is queryability: `QunMind` should be able to read recent
publish receipts so operators can inspect the last successful handoffs before a
full publisher dashboard exists.
