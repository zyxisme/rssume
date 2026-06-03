<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
  xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
  xmlns:content="http://purl.org/rss/1.0/modules/content/"
  xmlns:dc="http://purl.org/dc/elements/1.1/"
  xmlns:atom="http://www.w3.org/2005/Atom">
  <xsl:output method="html" indent="yes" encoding="UTF-8"/>

  <xsl:template match="/">
    <html lang="zh-CN">
      <head>
        <meta charset="UTF-8"/>
        <meta name="viewport" content="width=device-width, initial-scale=1.0"/>
        <title><xsl:value-of select="rss/channel/title"/> - rssume</title>
        <link rel="stylesheet" href="/feeds/assets/highlight.min.css"/>
        <style>
          @font-face {
            font-family: "JetBrains Mono";
            src: url("/feeds/assets/jetbrains-mono-regular.woff2") format("woff2");
            font-weight: 400;
            font-style: normal;
            font-display: swap;
          }

          @font-face {
            font-family: "JetBrains Mono";
            src: url("/feeds/assets/jetbrains-mono-bold.woff2") format("woff2");
            font-weight: 700;
            font-style: normal;
            font-display: swap;
          }

          :root {
            --canvas: #ffffff;
            --canvas-soft: #fafafa;
            --canvas-soft-2: #f5f5f5;
            --ink: #171717;
            --body: #4d4d4d;
            --mute: #888888;
            --hairline: #ebebeb;
            --hairline-strong: #a1a1a1;
            --primary: #171717;
            --on-primary: #ffffff;
            --link: #0070f3;
            --link-deep: #0761d1;
            --success: #0070f3;
            --error: #ee0000;
            --warning: #f5a623;
            --font-sans: Geist, Inter, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            --font-mono: "JetBrains Mono", "Fira Code", ui-monospace, monospace;
            --rounded-sm: 4px;
            --rounded-md: 6px;
            --rounded-lg: 8px;
            --rounded-xl: 12px;
          }

          *, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }

          body {
            font-family: var(--font-sans);
            font-size: 16px;
            line-height: 1.5;
            color: var(--ink);
            background: var(--canvas);
            -webkit-font-smoothing: antialiased;
          }

          header {
            background: var(--canvas);
            border-bottom: 1px solid var(--hairline);
            padding: 0 24px;
            height: 56px;
            display: flex;
            align-items: center;
            justify-content: space-between;
            position: sticky;
            top: 0;
            z-index: 100;
          }

          header .logo {
            font-size: 20px;
            font-weight: 600;
            color: var(--ink);
            text-decoration: none;
            letter-spacing: -0.5px;
          }

          header nav { display: flex; gap: 24px; align-items: center; }
          header nav a {
            color: var(--body);
            text-decoration: none;
            font-size: 14px;
            font-weight: 400;
            transition: color 0.15s;
          }
          header nav a:hover { color: var(--ink); }

          main {
            max-width: 800px;
            margin: 0 auto;
            padding: 32px 24px;
          }

          h1 {
            font-size: 32px;
            font-weight: 600;
            line-height: 1.25;
            letter-spacing: -0.96px;
            margin-bottom: 8px;
          }

          h2 {
            font-size: 24px;
            font-weight: 600;
            line-height: 1.3;
            letter-spacing: -0.5px;
            margin-bottom: 16px;
          }

          h3 {
            font-size: 18px;
            font-weight: 500;
            line-height: 1.4;
            margin-bottom: 8px;
          }

          .subtitle { color: var(--mute); font-size: 16px; margin-bottom: 32px; }

          .card {
            background: var(--canvas);
            border: 1px solid var(--hairline);
            border-radius: var(--rounded-lg);
            padding: 24px;
            margin-bottom: 16px;
            transition: border-color 0.15s;
          }
          .card:hover { border-color: var(--hairline-strong); }

          .article-meta {
            display: flex;
            gap: 8px;
            align-items: center;
            flex-wrap: wrap;
            margin: 8px 0;
            font-size: 14px;
            color: var(--mute);
          }

          .badge {
            display: inline-flex;
            padding: 2px 8px;
            font-size: 12px;
            font-weight: 400;
            border-radius: 9999px;
            background: var(--canvas-soft-2);
            color: var(--mute);
          }
          .badge-success { background: #d3e5ff; color: var(--link-deep); }

          .summary {
            background: var(--canvas-soft);
            border-left: 3px solid var(--link);
            padding: 12px 16px;
            border-radius: var(--rounded-sm);
            margin: 12px 0;
            font-size: 14px;
            color: var(--body);
          }

          .content {
            margin-top: 16px;
            font-size: 15px;
            line-height: 1.7;
            color: var(--body);
          }

          .content img {
            max-width: 100%;
            height: auto;
            border-radius: var(--rounded-md);
          }

          .content a {
            color: var(--link);
            text-decoration: none;
          }
          .content a:hover { text-decoration: underline; }

          .content pre {
            background: var(--canvas-soft);
            padding: 16px;
            border-radius: var(--rounded-md);
            overflow-x: auto;
            font-family: var(--font-mono);
            font-size: 14px;
            line-height: 1.6;
          }

          .content pre code {
            background: transparent;
            padding: 0;
            border-radius: 0;
          }

          .content code {
            font-family: var(--font-mono);
            font-size: 14px;
            background: var(--canvas-soft);
            padding: 2px 6px;
            border-radius: var(--rounded-sm);
          }

          /* Code block enhancements */
          .code-block {
            position: relative;
            margin: 16px 0;
            border-radius: var(--rounded-md);
            overflow: hidden;
            background: var(--canvas-soft);
          }

          .code-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 8px 16px;
            background: var(--canvas-soft-2);
            border-bottom: 1px solid var(--hairline);
            font-size: 12px;
            color: var(--mute);
          }

          .code-lang {
            font-family: var(--font-mono);
            text-transform: lowercase;
          }

          .code-copy {
            background: none;
            border: 1px solid var(--hairline);
            border-radius: var(--rounded-sm);
            padding: 4px 12px;
            font-size: 12px;
            font-family: var(--font-sans);
            color: var(--mute);
            cursor: pointer;
            transition: all 0.15s;
          }

          .code-copy:hover {
            color: var(--ink);
            border-color: var(--hairline-strong);
            background: var(--canvas);
          }

          .code-copy.copied {
            color: var(--success);
            border-color: var(--success);
          }

          .code-body {
            display: flex;
            overflow-x: auto;
          }

          .line-numbers {
            flex-shrink: 0;
            padding: 16px 0;
            padding-left: 16px;
            padding-right: 12px;
            text-align: right;
            font-family: var(--font-mono);
            font-size: 13px;
            line-height: 1.6;
            white-space: pre;
            color: var(--hairline-strong);
            user-select: none;
            border-right: 1px solid var(--hairline);
            background: var(--canvas-soft-2);
          }

          .code-body pre {
            margin: 0;
            padding: 16px;
            background: transparent;
            white-space: pre;
          }

          /* highlight.js style overrides for Vercel design */
          .hljs {
            background: transparent !important;
            padding: 0 !important;
          }

          .translation-info {
            margin-top: 12px;
            font-size: 13px;
            color: var(--mute);
            font-style: italic;
          }

          details {
            margin-top: 12px;
          }

          details summary {
            cursor: pointer;
            font-size: 13px;
            color: var(--mute);
            user-select: none;
          }
          details summary:hover { color: var(--ink); }

          .original-content {
            margin-top: 8px;
            padding: 16px;
            background: var(--canvas-soft);
            border-radius: var(--rounded-md);
            font-size: 14px;
            color: var(--mute);
            line-height: 1.6;
          }

          .empty-state {
            text-align: center;
            padding: 64px 24px;
            color: var(--mute);
          }
          .empty-state h3 { color: var(--ink); margin-bottom: 8px; }

          @media (max-width: 768px) {
            main { padding: 16px; }
            h1 { font-size: 24px; }
            .card { padding: 16px; }
          }
        </style>
      </head>
      <body>
        <header>
          <a href="/panel" class="logo">rssume</a>
          <nav>
            <a href="/panel">Dashboard</a>
            <a href="/panel/monitor">Monitor</a>
            <a href="/panel/settings">Settings</a>
          </nav>
        </header>
        <main>
          <h1><xsl:value-of select="rss/channel/title"/></h1>
          <p class="subtitle">
            <xsl:value-of select="count(rss/channel/item)"/> articles
            <xsl:if test="rss/channel/description">
              · <xsl:value-of select="rss/channel/description"/>
            </xsl:if>
          </p>

          <xsl:choose>
            <xsl:when test="rss/channel/item">
              <xsl:for-each select="rss/channel/item">
                <div class="card">
                  <h3>
                    <a href="{link}" target="_blank" rel="noopener" style="color: var(--ink); text-decoration: none;">
                      <xsl:value-of select="title"/>
                    </a>
                  </h3>

                  <div class="article-meta">
                    <xsl:if test="pubDate">
                      <span><xsl:value-of select="pubDate"/></span>
                    </xsl:if>
                    <xsl:if test="dc:creator">
                      <span>· <xsl:value-of select="dc:creator"/></span>
                    </xsl:if>
                    <xsl:if test="category">
                      <xsl:for-each select="category">
                        <span class="badge"><xsl:value-of select="."/></span>
                      </xsl:for-each>
                    </xsl:if>
                  </div>

                  <xsl:if test="description[contains(., '[AI 摘要]')]">
                    <div class="summary">
                      <strong>[AI 摘要]</strong>
                      <xsl:value-of select="substring-after(description, '[AI 摘要] ')"/>
                    </div>
                  </xsl:if>

                  <xsl:if test="content:encoded">
                    <div class="content">
                      <xsl:value-of select="content:encoded" disable-output-escaping="yes"/>
                    </div>
                  </xsl:if>

                  <xsl:if test="description[contains(., '由') and contains(., '模型翻译')]">
                    <div class="translation-info">
                      <xsl:value-of select="description[contains(., '由') and contains(., '模型翻译')]"/>
                    </div>
                  </xsl:if>
                </div>
              </xsl:for-each>
            </xsl:when>
            <xsl:otherwise>
              <div class="empty-state">
                <h3>No articles yet</h3>
                <p>Articles will appear here once the feed is polled.</p>
              </div>
            </xsl:otherwise>
          </xsl:choose>
        </main>
        <script src="/feeds/assets/highlight.min.js"></script>
        <script>
          document.addEventListener('DOMContentLoaded', function() {
            document.querySelectorAll('.content pre code').forEach(function(block) {
              hljs.highlightElement(block);

              var pre = block.parentElement;
              var lang = (block.className.match(/language-(\w+)/) || block.className.match(/hljs (\w+)/) || [])[1] || 'code';
              var lines = block.textContent.split('\n');
              if (lines[lines.length - 1] === '') lines.pop();

              var wrapper = document.createElement('div');
              wrapper.className = 'code-block';

              var header = document.createElement('div');
              header.className = 'code-header';

              var langSpan = document.createElement('span');
              langSpan.className = 'code-lang';
              langSpan.textContent = lang;

              var copyBtn = document.createElement('button');
              copyBtn.className = 'code-copy';
              copyBtn.textContent = 'Copy';
              copyBtn.onclick = function() {
                navigator.clipboard.writeText(block.textContent).then(function() {
                  copyBtn.textContent = 'Copied!';
                  copyBtn.classList.add('copied');
                  setTimeout(function() {
                    copyBtn.textContent = 'Copy';
                    copyBtn.classList.remove('copied');
                  }, 2000);
                });
              };

              header.appendChild(langSpan);
              header.appendChild(copyBtn);

              var body = document.createElement('div');
              body.className = 'code-body';

              var lineNums = document.createElement('div');
              lineNums.className = 'line-numbers';
              var numHtml = '';
              for (var i = 1; i <= lines.length; i++) {
                numHtml += i + '\n';
              }
              lineNums.textContent = numHtml;

              body.appendChild(lineNums);
              body.appendChild(pre.cloneNode(true));

              wrapper.appendChild(header);
              wrapper.appendChild(body);

              pre.parentNode.replaceChild(wrapper, pre);
            });
          });
        </script>
      </body>
    </html>
  </xsl:template>
</xsl:stylesheet>
