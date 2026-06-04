# RSS Preview Page TOC Sidebar

## Problem

The RSS preview page (`/feeds/:name`) renders all articles in a long scrollable list. For feeds with many articles, users have no way to quickly navigate between articles — they must scroll manually.

## Solution

Add a collapsible left sidebar with a table of contents (TOC) that lists all article titles. Clicking a title scrolls to that article. The sidebar is collapsed by default and does not interfere with normal reading.

## Scope

Single file change: `templates/rss_style.xsl`. No Rust code changes required.

## Design

### Layout

- Fixed sidebar on the left side of the viewport (`position: fixed; left: 0; top: 56px`)
- Default state: collapsed (hidden off-screen)
- Toggle button: fixed at bottom-left corner (`☰` icon), always visible
- Expanded width: 240px
- Sidebar overlays the page — main content does not shift
- Mobile (`max-width: 768px`): sidebar hidden entirely, toggle button also hidden

### TOC Content

- Feed title as sidebar header
- List of article titles, each as an `<a>` link
- No metadata (date/author) — titles only
- Each link uses `href="#article-{index}"` anchor

### Article Anchors

- Each `.card` div gets `id="article-{position()}"` via XSL
- Index is 1-based (`position()` in XSL `for-each`)

### JavaScript Behavior

1. **Toggle**: click toggle button → slide sidebar in/out from left
2. **Smooth scroll**: click TOC link → `el.scrollIntoView({ behavior: 'smooth', block: 'start' })`
3. **Active highlight**: `IntersectionObserver` on all `.card` elements, highlights the currently visible article's TOC entry with `var(--link)` color
4. **Close on mobile**: sidebar auto-hides on small screens via CSS media query

### Styling

- Sidebar background: `var(--canvas)` with `border-right: 1px solid var(--hairline)`
- TOC links: `var(--body)` color, `14px` font, hover → `var(--ink)`
- Active link: `var(--link)` color, `font-weight: 500`
- Toggle button: `position: fixed; bottom: 24px; left: 24px`, `40px` circle, `var(--primary)` background, `var(--on-primary)` text
- Sidebar z-index: 90 (below header at 100)
- Transition: `transform 0.2s ease` for slide animation

### Edge Cases

- Empty feed: no TOC rendered, toggle button hidden
- Single article: TOC still shows, but not very useful (acceptable)
- Very long article list (50+): TOC scrollable with `overflow-y: auto; max-height: calc(100vh - 100px)`

## Files Modified

| File | Change |
|------|--------|
| `templates/rss_style.xsl` | Add sidebar HTML, TOC XSL generation, JS for toggle/scroll/observe, CSS for sidebar |

## Verification

1. `cargo build --release` passes
2. Visit `/feeds/:name` in browser
3. See toggle button at bottom-left
4. Click toggle → sidebar slides in with article list
5. Click article title → page scrolls to that article, title highlighted
6. Scroll manually → highlight updates
7. Click toggle again → sidebar slides out
8. Mobile viewport: sidebar and toggle not visible
