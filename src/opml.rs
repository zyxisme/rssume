use crate::config::FeedConfig;
use crate::rss::generate::esc;

pub fn generate_opml(feeds: &[FeedConfig]) -> String {
    let outlines: String = feeds
        .iter()
        .filter(|f| f.enabled)
        .map(|f| {
            format!(
                r#"      <outline type="rss" text="{}" title="{}" xmlUrl="{}"/>"#,
                esc(&f.name),
                esc(&f.name),
                esc(&f.url)
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
