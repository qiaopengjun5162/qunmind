#!/usr/bin/env python3
"""抓取 X 推文并写入 QunMind 可消费的 JSON。

机制：经本机 Clash 代理调用 fxtwitter / vxtwitter 镜像 API（零凭据，与 Hermes 的
x-post-retrieval skill 同原理）。x.com 无原生 RSS，且公共 RSSHub 被 Cloudflare 墙，
故走镜像 API 取单条 status 正文最稳。

用法：
  python3 scripts/fetch_x_tweets.py \
      --watchlist data/x_watchlist.json \
      --out data/x_tweets.json \
      [--proxy http://127.0.0.1:7890]

输出 JSON 结构（供 src/source/x_bridge.rs 读取）：
  {
    "fetched_at": "ISO8601",
    "items": [
      {
        "source": "XBridge",
        "title": "<推文前 80 字>",
        "text": "<完整正文>",
        "url": "https://x.com/<handle>/status/<id>",
        "author_name": "...", "author_handle": "...",
        "created_at": "Sat Sep 05 ...", "likes": N, "retweets": N, "views": N,
        "category": "<handle>"
      }, ...
    ]
  }
"""
import argparse
import json
import os
import sys
import urllib.request
import urllib.error

DEFAULT_PROXY = "http://127.0.0.1:7890"
# fxtwitter 优先；vxtwitter 兜底
STATUS_URLS = [
    "https://api.fxtwitter.com/{handle}/status/{id}",
    "https://api.vxtwitter.com/{handle}/status/{id}",
]


def fetch_json(url: str, proxy: str):
    req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
    handler = urllib.request.ProxyHandler({"http": proxy, "https": proxy})
    opener = urllib.request.build_opener(handler)
    with opener.open(req, timeout=25) as r:
        return json.loads(r.read().decode("utf-8", "replace"))


def extract(d: dict) -> dict | None:
    t = d.get("tweet") or {}
    if not t:
        return None
    text = (t.get("text") or "").strip()
    if not text:
        return None
    author = t.get("author") or {}
    handle = author.get("screen_name") or ""
    tid = t.get("id") or ""
    url = t.get("url") or (f"https://x.com/{handle}/status/{tid}" if handle and tid else "")
    return {
        "source": "XBridge",
        "title": text[:80],
        "text": text,
        "url": url,
        "author_name": author.get("name"),
        "author_handle": handle,
        "created_at": t.get("created_at"),
        "likes": t.get("likes"),
        "retweets": t.get("retweets"),
        "views": t.get("views"),
        "category": handle or "x",
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--watchlist", default="data/x_watchlist.json")
    ap.add_argument("--out", default="data/x_tweets.json")
    ap.add_argument("--proxy", default=os.environ.get("X_PROXY", DEFAULT_PROXY))
    args = ap.parse_args()

    wl = json.load(open(args.watchlist, encoding="utf-8"))
    items_meta = wl.get("items", [])
    out_items = []
    errors = []
    for m in items_meta:
        handle, tid = m["handle"], m["id"]
        ok = False
        for tpl in STATUS_URLS:
            url = tpl.format(handle=handle, id=tid)
            try:
                d = fetch_json(url, args.proxy)
                rec = extract(d)
                if rec:
                    out_items.append(rec)
                    ok = True
                    print(f"[OK] @{handle}/{tid}", file=sys.stderr)
                    break
                else:
                    print(f"[EMPTY] @{handle}/{tid} via {tpl.split('/')[2]}", file=sys.stderr)
            except Exception as e:
                print(f"[ERR] @{handle}/{tid} {tpl.split('/')[2]}: {e}", file=sys.stderr)
        if not ok:
            errors.append(f"{handle}/{tid}")

    result = {
        "fetched_at": __import__("datetime").datetime.now(
            __import__("datetime").timezone.utc
        ).isoformat(),
        "count": len(out_items),
        "items": out_items,
        "errors": errors,
    }
    os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(result, f, ensure_ascii=False, indent=2)
    print(f"wrote {len(out_items)} items -> {args.out}" + (f"  errors={errors}" if errors else ""),
          file=sys.stderr)


if __name__ == "__main__":
    main()
