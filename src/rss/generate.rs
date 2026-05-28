use crate::storage::Article;
use chrono::DateTime;

pub fn generate_rss(feed_name: &str, articles: &[Article]) -> String {
    let now = chrono::Utc::now().to_rfc2822();

    let mut xml = String::new();
    xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    xml.push('\n');
    xml.push_str(r#"<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">"#);
    xml.push('\n');
    xml.push_str("  <channel>\n");
    xml.push_str(&format!("    <title>{}</title>\n", escape_xml(feed_name)));
    xml.push_str(&format!(
        "    <description>rssume processed feed for {}</description>\n",
        escape_xml(feed_name)
    ));
    xml.push_str(&format!(
        "    <atom:link href=\"/feeds/{}\" rel=\"self\" type=\"application/rss+xml\"/>\n",
        escape_xml(feed_name)
    ));
    xml.push_str(&format!("    <lastBuildDate>{}</lastBuildDate>\n", now));
    xml.push_str("    <generator>rssume</generator>\n");

    for article in articles.iter().rev().take(50) {
        xml.push_str("    <item>\n");
        xml.push_str(&format!(
            "      <title>{}</title>\n",
            escape_xml(&article.title)
        ));

        let description = build_description(article);
        xml.push_str(&format!(
            "      <description>{}</description>\n",
            escape_xml(&description)
        ));

        xml.push_str(&format!(
            "      <link>{}</link>\n",
            escape_xml(&article.link)
        ));
        xml.push_str(&format!("      <guid>{}</guid>\n", escape_xml(&article.id)));

        if let Ok(dt) = DateTime::parse_from_rfc2822(&article.published_at) {
            xml.push_str(&format!("      <pubDate>{}</pubDate>\n", dt.to_rfc2822()));
        }
        xml.push_str("    </item>\n");
    }

    xml.push_str("  </channel>\n");
    xml.push_str("</rss>\n");
    xml
}

fn build_description(article: &Article) -> String {
    let mut desc = String::new();

    if let Some(ref summary) = article.summary {
        desc.push_str(&format!(
            "<p><strong>[AI 摘要]</strong> {}</p>\n<hr/>\n",
            escape_xml(summary)
        ));
    }

    desc.push_str(&article.content);

    if article.translated {
        desc.push_str(&format!(
            "\n<p><em>(Translated from {})</em></p>",
            article
                .source_lang
                .as_ref()
                .map(|l| crate::lang::lang_name(l))
                .unwrap_or_else(|| "unknown".to_string())
        ));
    }

    desc
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
