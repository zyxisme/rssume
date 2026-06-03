# XSLT RSS Feed Readability Design

## Overview

Add human-readable HTML rendering to RSS feed endpoints using XSLT stylesheets. When users visit `/feeds/:name` in a browser, they'll see a beautifully formatted page instead of raw XML. RSS readers will continue to receive the standard XML feed.

## Goals

1. **Browser-friendly**: Automatic HTML rendering when accessed via browser
2. **RSS-compatible**: Standard XML for feed readers (XSLT ignored)
3. **Design consistency**: Follow existing Vercel-style design system
4. **Rich content**: Display full HTML content, AI summaries, translation info
5. **Security**: Safe rendering of HTML content

## Architecture

### File Structure

```
templates/
└── rss_style.xsl        # New: XSLT stylesheet (embedded at compile time)
src/
└── rss/
    └── generate.rs      # Modified: Add XSLT stylesheet reference
```

### Implementation Approach

1. **XSLT Stylesheet**: Create an XSLT file that transforms RSS XML into HTML
2. **XML Processing Instruction**: Add `<?xml-stylesheet?>` to generated RSS XML
3. **Static File Serving**: Serve the XSLT file via Axum static file handler
4. **Browser Rendering**: Browsers automatically apply XSLT when accessing RSS feed

### Data Flow

```
Browser Request → /feeds/:name → RSS XML + XSLT PI → Browser applies XSLT → HTML Page
RSS Reader → /feeds/:name → RSS XML (XSLT ignored) → Standard RSS parsing
```

## Design Details

### 1. XSLT Stylesheet Design

**Location**: `templates/rss_style.xsl`

**Features**:
- Transform RSS XML to HTML5
- Apply Vercel-style CSS variables and typography
- Responsive layout for mobile/desktop
- Secure HTML content rendering

**Key Elements**:
- Feed header with title and description
- Article list with card-based layout
- Each article shows: title, metadata, AI summary, full content
- Translation information display
- Original content toggle for translated articles

### 2. CSS Design System

**Variables** (matching existing Vercel style):
```css
:root {
  --canvas: #ffffff;
  --canvas-soft: #fafafa;
  --ink: #171717;
  --body: #4d4d4d;
  --mute: #888888;
  --hairline: #ebebeb;
  --primary: #171717;
  --link: #0070f3;
  --font-sans: Geist, Inter, -apple-system, sans-serif;
  --font-mono: "JetBrains Mono", ui-monospace, monospace;
}
```

**Typography**:
- Display: 32px, weight 600, letter-spacing -0.96px
- Body: 16px, weight 400, line-height 1.5
- Caption: 14px, weight 400, color var(--mute)

**Components**:
- Cards with border: 1px solid var(--hairline)
- Rounded corners: 8px (var(--rounded-lg))
- Hover effects: border-color change
- Badges for translation/summary status

### 3. Content Rendering

**AI Summary**:
- Special styling with left border accent
- Background: var(--canvas-soft)
- Border-left: 3px solid var(--link)

**Translation Info**:
- Display model name and token count
- Badge-style presentation

**HTML Content**:
- Render using `disable-output-escaping="yes"`
- Browser handles XSS protection automatically
- Support for images, links, formatting

**Original Content** (for translated articles):
- Collapsible section
- Muted styling to distinguish from translated content

### 4. Responsive Design

**Breakpoints**:
- Mobile: < 768px (single column)
- Tablet: 768px - 1024px (2 columns optional)
- Desktop: > 1024px (full layout)

**Mobile Optimizations**:
- Stack content vertically
- Reduce padding and margins
- Adjust font sizes for readability

## Implementation Steps

### Phase 1: XSLT File Creation
1. Create `templates/rss_style.xsl` with complete XSLT stylesheet
2. Include all CSS variables and responsive design
3. Handle all RSS elements (title, description, items, etc.)

### Phase 2: RSS Generation Modification
1. Modify `src/rss/generate.rs` to add XSLT processing instruction
2. Add `<?xml-stylesheet type="text/xsl" href="/feeds/style.xsl"?>` to RSS XML
3. Embed XSLT file using `include_str!()` at compile time

### Phase 3: XSLT Endpoint
1. Add new route `/feeds/style.xsl` in `src/web/rss_route.rs`
2. Serve embedded XSLT content with `application/xslt+xml` Content-Type
3. No need for static file serving - XSLT embedded in binary

### Phase 4: Testing & Refinement
1. Test in multiple browsers (Chrome, Firefox, Safari)
2. Verify RSS reader compatibility
3. Test responsive design on different screen sizes
4. Validate HTML content rendering

## Security Considerations

### HTML Content Safety
- XSLT runs in browser sandbox
- Browser automatically sanitizes HTML content
- No server-side HTML parsing needed
- XSS protection handled by browser

### File Serving
- XSLT file embedded in binary at compile time
- Served via dedicated endpoint `/feeds/style.xsl`
- Proper Content-Type headers (`application/xslt+xml`)
- No user input in XSLT file

## Success Criteria

1. ✅ Browser shows formatted HTML page when accessing `/feeds/:name`
2. ✅ RSS readers receive standard XML feed
3. ✅ Design matches existing Vercel style system
4. ✅ All article information displayed (summary, translation, content)
5. ✅ Responsive design works on mobile
6. ✅ HTML content renders safely
7. ✅ No performance impact on RSS generation

## Future Enhancements

1. **Dark Mode**: Add CSS media query for dark mode support
2. **Print Styles**: Optimize for printing articles
3. **Accessibility**: ARIA labels and keyboard navigation
4. **Custom Themes**: Allow users to customize feed appearance
5. **Feed Discovery**: Add HTML version link in RSS feed
