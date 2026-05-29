# Design: OPML RSS Source Fix & Monitor Stream Auto-Scroll

## Problem

1. **OPML export uses upstream URLs**: The `/api/feeds/export.opml` endpoint writes the raw upstream RSS feed URL as `xmlUrl` in the OPML outline. Users who import this OPML into an RSS reader will bypass rssume's translation/summarization pipeline and read the original (untranslated) feeds.

2. **Monitor shows oldest stream content**: The "Active Translations" section on `/panel/monitor` and `/panel/feed/{name}/monitor` displays streamed translation text with `overflow:hidden`, which clips the bottom (newest) content. Users see the beginning of the translation stream instead of the latest output.

## Design

### 1. OPML URL from Request Context

**Approach**: Extract the base URL from the incoming HTTP request headers, so the OPML `xmlUrl` always points to rssume itself.

**Base URL resolution order** (in `web/api.rs` handler):
1. `X-Forwarded-Proto` + `X-Forwarded-Host` headers (reverse proxy)
2. `Host` header + `http` scheme
3. Fallback: `http://{config.server.host}:{config.server.port}`

**Changes**:

- `src/opml.rs`:
  - Add `base_url: &str` parameter to `generate_opml`
  - Change `xmlUrl` from `f.url` to `{base_url}/feeds/{f.name}`
  - Add `htmlUrl` attribute: `{base_url}/panel/feed/{f.name}/monitor`

- `src/web/api.rs` (`export_opml` handler):
  - Add `axum::http::HeaderMap` extractor to the handler signature
  - Implement `fn base_url_from_headers(headers, config) -> String` helper
  - Pass constructed base URL to `generate_opml`

### 2. Stream Auto-Scroll in Monitor

**Approach**: Use htmx's `hx-on:after-swap` event to scroll streamed text containers to the bottom after each polling update.

**Changes**:

- `templates/partials/monitor_status.html` (line 34):
  - Change `overflow:hidden` to `overflow-y:auto`
  - Add `class="stream-text"` to the streamed text div
  - Add `hx-on:after-swap` on the polling target: `document.querySelectorAll('.stream-text').forEach(el => el.scrollTop = el.scrollHeight)`

- `templates/partials/feed_monitor_status.html` (line 38):
  - Add `max-height:200px;overflow-y:auto` (currently unbounded)
  - Add `class="stream-text"` to the streamed text div
  - Add `hx-on:after-swap` on the polling target for auto-scroll

## Files to Modify

| File | Change |
|------|--------|
| `src/opml.rs` | Add `base_url` param, use rssume URLs, add `htmlUrl` |
| `src/web/api.rs` | Extract request base URL in `export_opml` handler |
| `templates/partials/monitor_status.html` | CSS fix + auto-scroll JS |
| `templates/partials/feed_monitor_status.html` | CSS fix + auto-scroll JS |

## No Changes Needed

- `src/config.rs`: No new config fields required
- `src/web/panel.rs`: Page handlers unchanged (HTMX polling targets are in the partials)
- `src/monitor.rs`: Data model unchanged
- `src/scheduler.rs`: Streaming logic unchanged
