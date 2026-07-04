# Visual Operations Log

Use this log whenever work includes drawing, image generation, diagram creation, UI mockups, or visual asset editing.

## Template

```md
## YYYY-MM-DD

- purpose:
- source:
- output files:
- notes:
```

## 2026-06-23

- purpose: initialize persistent visual-operation log for future drawing and asset work
- source: project workflow requirement
- output files: `docs/visual-operations.md`
- notes: no generated visual assets recorded yet; this entry creates the reusable ledger

## 2026-06-26

- purpose: generate a deterministic local WeChat cover for public-source daily reports so the published draft reflects the actual Web3 lead story instead of a generic fallback cover
- source: `QunMind` daily-report frontmatter (`title`, `digest`) plus focus keywords inferred from the generated article content
- output files: runtime-generated `*.cover.png` files adjacent to report markdown outputs, for example `/tmp/daily.cover.png`
- notes: this visual experiment was reverted in the same day after content review; current production path no longer injects local `cover:` assets and instead leaves cover generation to `moonpub --render`

## 2026-06-26

- purpose: create a fixed reusable WeChat cover for the personal public account column `AI · Web3 最新日报`
- source: user direction that the account is a personal public account named `寻月隐君`; rendered locally as a deterministic cover template
- output files: `docs/assets/wechat/ai-web3-daily-cover.png`, `docs/assets/wechat/ai-web3-daily-cover-900x500.png`, `docs/assets/wechat/render_ai_web3_daily_cover.swift`
- notes: final visual direction is a restrained personal tech-notes cover with moon, signal lines, dark green grid, the fixed column title, and author name; PNG generated via Swift/AppKit for crisp Chinese text; the earlier SVG attempt was removed because local rasterization could not reliably render Chinese text

## 2026-07-03

- purpose: improve WeChat daily cover readability after mobile preview showed white title text was hard to read
- source: existing deterministic Swift/AppKit cover template `docs/assets/wechat/render_ai_web3_daily_cover.swift`
- output files: `docs/assets/wechat/ai-web3-daily-cover-900x500.png`
- notes: changed the cover from dark background with white title text to a warm light background with high-contrast deep green title text; regenerated PNG locally with Swift/AppKit

## 2026-07-03

- purpose: update the active WeChat footer group QR code used by `moonpub` previews and drafts
- source: `/Users/qiaopengjun/Library/Mobile Documents/com~apple~CloudDocs/ObsidianMain/Context/assets/qrcode-group.png`
- output files: `/Users/qiaopengjun/Code/Rust/moonpub-data/qrcode.png`
- notes: copied the latest group QR code into the active `moonpub-data` path referenced by `moonpub.toml`; SHA256 `46ac3dd7142f6707ee4ec0ab2b2b9364bdc8ac8eb6e9bed98d964ce843c85608`
