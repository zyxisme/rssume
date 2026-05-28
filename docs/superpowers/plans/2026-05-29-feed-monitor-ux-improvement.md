# Feed Monitor UX 改进实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让单 Feed Monitor 页面流式翻译内容完整展示，无需滚动，并加快轮询至 500ms。

**Architecture:** 仅修改两个模板文件，不涉及后端逻辑。轮询间隔从 2s 改为 500ms，流式文本区域去掉高度限制。

**Tech Stack:** Tera templates, htmx

---

### Task 1: 加快轮询频率

**Files:**
- Modify: `templates/feed_monitor.html:14`

- [ ] **Step 1: 修改轮询间隔**

将 `hx-trigger="every 2s"` 改为 `hx-trigger="every 500ms"`：

```html
<div hx-get="/api/monitor/feed/{{ feed_name }}/translating" hx-trigger="every 500ms" hx-target="#feed-monitor-content" hx-swap="innerHTML" aria-hidden="true"></div>
```

- [ ] **Step 2: 验证**

Run: `cargo build --release`
Expected: 编译成功（模板在编译时嵌入）

- [ ] **Step 3: Commit**

```bash
git add templates/feed_monitor.html
git commit -m "feat: speed up feed monitor polling to 500ms"
```

---

### Task 2: 去掉流式文本高度限制

**Files:**
- Modify: `templates/partials/feed_monitor_status.html:35`

- [ ] **Step 1: 去掉 max-height 和 overflow**

将：
```html
<div style="margin-top:8px;font-size:13px;color:var(--mute);max-height:120px;overflow-y:auto;white-space:pre-wrap;">{{ t.streamed_text }}</div>
```

改为：
```html
<div style="margin-top:8px;font-size:13px;color:var(--mute);white-space:pre-wrap;">{{ t.streamed_text }}</div>
```

- [ ] **Step 2: 验证**

Run: `cargo build --release`
Expected: 编译成功

- [ ] **Step 3: Commit**

```bash
git add templates/partials/feed_monitor_status.html
git commit -m "feat: remove max-height limit on streamed translation text"
```
