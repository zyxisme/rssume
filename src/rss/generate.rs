use crate::storage::Article;
use chrono::DateTime;

pub fn generate_rss(feed_name: &str, articles: &[Article]) -> String {
    let now = chrono::Utc::now().to_rfc2822();
    let mut xml = String::new();
    xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    xml.push('\n');
    xml.push_str(concat!(
        r#"<rss version="2.0""#,
        r#" xmlns:atom="http://www.w3.org/2005/Atom""#,
        r#" xmlns:content="http://purl.org/rss/1.0/modules/content/""#,
        r#" xmlns:dc="http://purl.org/dc/elements/1.1/""#,
        r#">"#,
    ));
    xml.push('\n');
    xml.push_str("  <channel>\n");
    xml.push_str(&format!("    <title>{}</title>\n", esc(feed_name)));
    xml.push_str(&format!(
        "    <description>rssume processed feed for {}</description>\n",
        esc(feed_name)
    ));
    xml.push_str(&format!("    <link>/feeds/{}</link>\n", esc(feed_name)));
    xml.push_str(&format!(
        "    <atom:link href=\"/feeds/{}\" rel=\"self\" type=\"application/rss+xml\"/>\n",
        esc(feed_name)
    ));
    xml.push_str(&format!("    <lastBuildDate>{}</lastBuildDate>\n", now));
    xml.push_str("    <generator>rssume</generator>\n");

    for article in articles.iter().rev().take(50) {
        xml.push_str("    <item>\n");
        xml.push_str(&format!("      <title>{}</title>\n", esc(&article.title)));

        if let Some(ref a) = article.author {
            xml.push_str(&format!("      <dc:creator>{}</dc:creator>\n", esc(a)));
        }
        for c in &article.categories {
            xml.push_str(&format!("      <category>{}</category>\n", esc(c)));
        }

        let desc = if let Some(ref s) = article.summary {
            format!("[AI 摘要] {}", esc(s))
        } else {
            esc(&article.content.chars().take(200).collect::<String>())
        };
        xml.push_str(&format!("      <description>{}</description>\n", desc));

        xml.push_str("      <content:encoded><![CDATA[");
        if let Some(ref s) = article.summary {
            xml.push_str(&format!(
                "<p><strong>[AI 摘要]</strong> {}</p><hr/>",
                esc(s)
            ));
        }
        xml.push_str(&article.content);
        if article.translated {
            xml.push_str(&format!(
                "<p><em>(Translated from {})</em></p>",
                article
                    .source_lang
                    .as_ref()
                    .map(|l| crate::lang::lang_name(l))
                    .unwrap_or_else(|| "unknown".to_string())
            ));
        }
        xml.push_str("]]></content:encoded>\n");

        xml.push_str(&format!("      <link>{}</link>\n", esc(&article.link)));
        xml.push_str(&format!(
            "      <guid isPermaLink=\"false\">{}</guid>\n",
            esc(&article.id)
        ));

        if let Some(ref d) = article.published_at_rfc2822 {
            xml.push_str(&format!("      <pubDate>{}</pubDate>\n", d));
        } else if let Ok(dt) = DateTime::parse_from_rfc2822(&article.published_at) {
            xml.push_str(&format!("      <pubDate>{}</pubDate>\n", dt.to_rfc2822()));
        }

        if let Some(ref enc) = article.enclosure {
            xml.push_str(&format!(
                "      <enclosure url=\"{}\" length=\"{}\" type=\"{}\"/>\n",
                esc(&enc.url),
                enc.length.unwrap_or(0),
                esc(enc.content_type.as_deref().unwrap_or("image/jpeg"))
            ));
        }

        xml.push_str("    </item>\n");
    }
    xml.push_str("  </channel>\n</rss>\n");
    xml
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
