# Local Publisher Network Diagnostics

QunMind should treat local proxy tools such as Mihomo, Clash Verge, or Clash as
operator-side dependencies for publishing diagnostics, not as part of the daily
report generation pipeline.

The `mcncarl/yichen-skills` repository is a useful reference candidate for
workflow boundaries, but it should be reviewed from source before copying any
implementation. The relevant direction for QunMind is:

- keep account state, cookies, local paths, and private app profiles outside the
  project repo
- run risky browser or platform automation in isolated helper boundaries
- return structured output that an agent can inspect before taking the next step
- avoid final publish-style actions unless the caller explicitly asks

For QunMind, this translates into a small diagnostic boundary around the
`QunMind -> moonpub -> WeChat OpenAPI` path.

## What To Diagnose

When WeChat returns `errcode=40164 invalid ip`, the first question is no longer
"did QunMind generate the report?" The report may be fine. The diagnostic should
answer:

- which publisher command is being used
- whether `WECHAT_APPID` and `WECHAT_SECRET` are present
- whether `HTTP_PROXY`, `HTTPS_PROXY`, or `ALL_PROXY` are set for the publisher
- whether the local proxy port is reachable
- whether Mihomo / Clash has an external controller enabled
- which route matched `api.weixin.qq.com`
- which exit IP WeChat currently sees
- whether that IP is stable enough to add to the WeChat IP allowlist

Also keep the two WeChat tokens separate:

- OpenAPI `access_token`: fetched with `appid` / `secret` from
  `api.weixin.qq.com/cgi-bin/token`; it is enough for upload image, create draft,
  list draft, and similar API operations.
- WeChat MP backend page `token=`: extracted from `mp.weixin.qq.com/cgi-bin/...`
  after a browser QR-code login; it belongs to the web session used by preview
  and backend automation.

If a draft push succeeds but the follow-up says `login timeout: QR code not
scanned within 120s`, the OpenAPI token was valid. The failed part is the browser
session for WeChat backend preview/configuration, not the API draft creation.

## Mihomo / Clash Boundary

The current CLI helper is `report-network-status`. It is best-effort and
read-only by default. Safe operations:

- read macOS proxy settings with `scutil --proxy`
- read environment proxy variables
- detect local proxy ports such as `127.0.0.1:7890`
- query a configured Mihomo external controller such as `/proxies`, `/rules`, or
  `/connections` when the controller URL and secret are explicitly configured
- redact node names, subscription URLs, tokens, passwords, cookies, and local
  profile paths before returning JSON

Risky operations that should stay out of the default path:

- editing Clash Verge profiles automatically
- switching the active proxy profile
- exporting full subscription YAML
- committing proxy configs or tokens
- relying on a Cloudflare anycast / preferred route as a stable WeChat allowlist
  IP

## Recommended Shape

The CLI helper is `report-network-status`; an MCP counterpart can later be added
as `report_network_status`. The output shape is:

```json
{
  "ok": true,
  "proxy_env": {
    "http_proxy": "set",
    "https_proxy": "set",
    "all_proxy": "unset"
  },
  "local_proxy": {
    "url": "http://127.0.0.1:7890",
    "reachable": true
  },
  "mihomo": {
    "controller_configured": true,
    "controller_reachable": true,
    "api_weixin_route": "ProxyGroup(redacted)",
    "selected_node": "redacted"
  },
  "wechat_openapi": {
    "last_error": "errcode=40164 invalid ip",
    "current_ip": "104.28.x.x",
    "recommendation": "switch_to_fixed_node_or_add_current_ip_to_allowlist"
  }
}
```

This helper should only diagnose by default. If it ever supports mutating proxy
state, it must require an explicit flag such as `--apply`, and it should describe
the exact file or controller endpoint it will change before doing so.

## Operational Notes

For WeChat public-account publishing, a stable fixed-node exit is usually better
than a fast-but-floating anycast exit. If `api.weixin.qq.com` routes through a
Cloudflare preferred route and WeChat keeps reporting different `104.28.*.*`
addresses, adding one IP at a time will be fragile. Prefer selecting a fixed
ordinary node, then verify the WeChat-reported IP before pushing the draft again.

If the operator disables transparent proxy / TUN style tools and WeChat starts
reporting the same direct exit IP on repeated attempts, the diagnosis changes.
At that point the main question is no longer "is the publisher path still using
the wrong route?" but "has that exact IP really been added to the WeChat
OpenAPI allowlist, and has the backend accepted the change yet?"

On `2026-07-08`, the path moved from floating `104.28.*.*` exits to a stable
direct IP `117.22.121.195` after closing Warp, Clash Verge, and EasyConnect.
Repeated `errcode=40164 invalid ip` responses with that same IP confirmed that
the remaining blocker was the WeChat-side allowlist state, not markdown
generation, lint, `moonpub`, or proxy drift. Treat this as the default playbook
for future repeats of the same pattern.
