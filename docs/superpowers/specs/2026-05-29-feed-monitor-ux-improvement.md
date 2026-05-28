# Feed Monitor 页面 UX 改进

## 目标

让单 Feed Monitor 页面（`/panel/feed/:name/monitor`）的翻译流内容完整展示，不需要滚动，并加快轮询让用户更实时地看到流式输出。

## 改动

### 1. 加快轮询频率

文件：`templates/feed_monitor.html`

- `hx-trigger="every 2s"` → `hx-trigger="every 500ms"`

### 2. 去掉流式文本高度限制

文件：`templates/partials/feed_monitor_status.html`

- 去掉流式文本 `<div>` 上的 `max-height:120px; overflow-y:auto;`
- 保留 `white-space:pre-wrap;` 以正确显示换行

## 不改动

- 全局 Monitor 页面（`/panel/monitor`）保持原样
- 后端 API、Monitor 数据结构、流式收集逻辑均不变
- 卡片布局、badge 样式不变

## 验证

- 打开 `/panel/feed/:name/monitor`
- 触发翻译任务
- 确认流式文本完整展示，无滚动条
- 确认更新频率明显快于之前（约 500ms 一次）
