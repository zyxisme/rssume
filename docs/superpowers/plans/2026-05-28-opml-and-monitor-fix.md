# OPML RSS Source Fix & Monitor Stream Auto-Scroll

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix OPML export to use rssume's own feed URLs instead of upstream URLs, and make the monitor page auto-scroll to show the latest streaming translation text.

**Architecture:** Two independent fixes — (1) extract base URL from HTTP request headers in the OPML handler and pass it to `generate_opml`, (2) change CSS overflow and add htmx `after-swap` JS to auto-scroll streamed text containers.

**Tech Stack:** Rust, Axum 0.8, Tera templates, htmx

---

### Task 1: OPML Export — Use rssume Feed URLs

**Files:**
- Modify: `src/opml.rs:4-14`
- Modify: `src/web/api.rs:81-95`

- [ ] **Step 1: Update `generate_opml` signature and URL logic**

In `src/opml.rs`, change the function to accept a `base_url` parameter and use rssume's own URLs:

```rust
use crate::config::FeedConfig;
use crate::rss::generate::esc;

pub fn generate_opml(feeds: &[FeedConfig], base_url: &str) -> String {
    let outlines: String = feeds
        .iter()
        .filter(|f| f.enabled)
        .map(|f| {
            let feed_url = format!("{}/feeds/{}", base_url.trim_end_matches('/'), f.name);
            let panel_url = format!(
                "{}/panel/feed/{}/monitor",
                base_url.trim_end_matches('/'),
                f.name
            );
            format!(
                r#"      <outline type="rss" text="{}" title="{}" xmlUrl="{}" htmlUrl="{}"/>"#,
                esc(&f.name),
                esc(&f.name),
                esc(&feed_url),
                esc(&panel_url)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <head>
    <title>rssume subscriptions</title>
    <dateCreated>{}</dateCreated>
  </head>
  <body>
    <outline text="rssume" title="rssume">
{}
    </outline>
  </body>
</opml>"#,
        chrono::Utc::now().to_rfc3339(),
        outlines
    )
}
```

- [ ] **Step 2: Add `base_url_from_headers` helper in `web/api.rs`**

Add this helper function before `export_opml` (around line 80):

```rust
fn base_url_from_headers(headers: &axum::http::HeaderMap, cfg: &crate::config::Config) -> String {
    // Reverse proxy headers
    if let (Some(proto), Some(host)) = (
        headers
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok()),
        headers
            .get("x-forwarded-host")
            .and_then(|v| v.to_str().ok()),
    ) {
        return format!("{}://{}", proto, host);
    }
    // Direct request
    if let Some(host) = headers.get("host").and_then(|v| v.to_str().ok()) {
        return format!("http://{}", host);
    }
    // Fallback to config
    format!("http://{}:{}", cfg.server.host, cfg.server.port)
}
```

- [ ] **Step 3: Update `export_opml` handler to pass base URL**

Change the handler signature and body:

```rust
async fn export_opml(
    Extension(s): Extension<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let cfg = s.config.read().await;
    let base_url = base_url_from_headers(&headers, &cfg);
    let opml = crate::opml::generate_opml(&cfg.feeds, &base_url);

    (
        [
            ("Content-Type", "application/xml; charset=utf-8"),
            (
                "Content-Disposition",
                r#"attachment; filename="rssume-subscriptions.opml""#,
            ),
        ],
        opml,
    )
}
```

- [ ] **Step 4: Build and verify**

```bash
cargo check
```

Expected: Compiles without errors.

- [ ] **Step 5: Commit**

```bash
git add src/opml.rs src/web/api.rs
git commit -m "fix: opml export uses rssume feed URLs from request headers"
```

---

### Task 2: Monitor Auto-Scroll for Streamed Text

**Files:**
- Modify: `templates/partials/monitor_status.html:34`
- Modify: `templates/partials/feed_monitor_status.html:38`
- Modify: `templates/monitor.html:7`
- Modify: `templates/feed_monitor.html:14`

- [ ] **Step 1: Fix global monitor partial — CSS and class**

In `templates/partials/monitor_status.html`, change line 34 from:

```html
<div style="margin-top:8px;font-size:13px;color:var(--mute);max-height:80px;overflow:hidden;white-space:pre-wrap;">{{ t.streamed_text }}</div>
```

to:

```html
<div class="stream-text" style="margin-top:8px;font-size:13px;color:var(--mute);max-height:80px;overflow-y:auto;white-space:pre-wrap;">{{ t.streamed_text }}</div>
```

- [ ] **Step 2: Fix feed monitor partial — CSS and class**

In `templates/partials/feed_monitor_status.html`, change line 38 from:

```html
<div style="margin-top:8px;font-size:13px;color:var(--mute);white-space:pre-wrap;">{{ t.streamed_text }}</div>
```

to:

```html
<div class="stream-text" style="margin-top:8px;font-size:13px;color:var(--mute);max-height:200px;overflow-y:auto;white-space:pre-wrap;">{{ t.streamed_text }}</div>
```

- [ ] **Step 3: Add auto-scroll to global monitor template**

In `templates/monitor.html`, change line 7 from:

```html
<div hx-get="/api/monitor/translating" hx-trigger="every 2s" hx-target="#monitor-content" hx-swap="innerHTML" aria-hidden="true"></div>
```

to:

```html
<div hx-get="/api/monitor/translating" hx-trigger="every 2s" hx-target="#monitor-content" hx-swap="innerHTML" hx-on:after-swap="document.querySelectorAll('.stream-text').forEach(el => el.scrollTop = el.scrollHeight)" aria-hidden="true"></div>
```

- [ ] **Step 4: Add auto-scroll to feed monitor template**

In `templates/feed_monitor.html`, change line 14 from:

```html
<div hx-get="/api/monitor/feed/{{ feed_name }}/translating" hx-trigger="every 500ms" hx-target="#feed-monitor-content" hx-swap="innerHTML" aria-hidden="true"></div>
```

to:

```html
<div hx-get="/api/monitor/feed/{{ feed_name }}/translating" hx-trigger="every 500ms" hx-target="#feed-monitor-content" hx-swap="innerHTML" hx-on:after-swap="document.querySelectorAll('.stream-text').forEach(el => el.scrollTop = el.scrollHeight)" aria-hidden="true"></div>
```

- [ ] **Step 5: Build and verify**

```bash
cargo check
```

Expected: Compiles without errors.

- [ ] **Step 6: Commit**

```bash
git add templates/partials/monitor_status.html templates/partials/feed_monitor_status.html templates/monitor.html templates/feed_monitor.html
git commit -m "fix: auto-scroll streamed text to latest content in monitor pages"
```
