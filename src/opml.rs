use crate::config::FeedConfig;

pub fn generate_opml(feeds: &[FeedConfig]) -> String {
    let outlines: String = feeds
        .iter()
        .filter(|f| f.enabled)
        .map(|f| {
            format!(
                r#"      <outline type="rss" text="{}" title="{}" xmlUrl="{}"/>"#,
                escape_xml(&f.name),
                escape_xml(&f.name),
                escape_xml(&f.url)
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

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
