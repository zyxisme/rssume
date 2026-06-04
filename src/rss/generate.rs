use crate::storage::Article;
use ammonia::Builder;
use chrono::DateTime;

pub fn generate_rss(feed_name: &str, articles: &[Article]) -> String {
    let now = chrono::Utc::now().to_rfc2822();
    let mut xml = String::new();
    xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    xml.push('\n');
    xml.push_str(r#"<?xml-stylesheet type="text/xsl" href="/feeds/style.xsl"?>"#);
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

    for article in articles.iter() {
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
                "<div style=\"background:#f0f4f8;border-left:3px solid #3b82f6;padding:12px 16px;border-radius:6px;margin:12px 0;font-size:14px;color:#555\">\
                <strong>[AI 摘要]</strong> {}</div>",
                s
            ));
        }
        // Clean HTML to ensure proper tag closing and prevent nesting issues
        // Allow id attributes for in-article anchor links (e.g., footnotes)
        let cleaned_content = Builder::default()
            .add_tag_attributes("div", &["id"])
            .add_tag_attributes("span", &["id"])
            .add_tag_attributes("a", &["id"])
            .add_tag_attributes("h1", &["id"])
            .add_tag_attributes("h2", &["id"])
            .add_tag_attributes("h3", &["id"])
            .add_tag_attributes("h4", &["id"])
            .add_tag_attributes("h5", &["id"])
            .add_tag_attributes("h6", &["id"])
            .add_tag_attributes("p", &["id"])
            .add_tag_attributes("li", &["id"])
            .add_tag_attributes("td", &["id"])
            .add_tag_attributes("th", &["id"])
            .add_tag_attributes("section", &["id"])
            .add_tag_attributes("blockquote", &["id"])
            .add_tag_attributes("pre", &["id"])
            .add_tag_attributes("code", &["id"])
            .add_tag_attributes("table", &["id"])
            .add_tag_attributes("tr", &["id"])
            .add_tag_attributes("ul", &["id"])
            .add_tag_attributes("ol", &["id"])
            .add_tag_attributes("dl", &["id"])
            .add_tag_attributes("dt", &["id"])
            .add_tag_attributes("dd", &["id"])
            .add_tag_attributes("em", &["id"])
            .add_tag_attributes("strong", &["id"])
            .add_tag_attributes("sup", &["id"])
            .add_tag_attributes("sub", &["id"])
            .add_tag_attributes("details", &["id"])
            .add_tag_attributes("summary", &["id"])
            .add_tag_attributes("figure", &["id"])
            .add_tag_attributes("figcaption", &["id"])
            .clean(&article.content)
            .to_string();
        xml.push_str(&cleaned_content);
        if article.translated {
            let model = article.translation_model.as_deref().unwrap_or("unknown");
            let tokens = article
                .translation_tokens
                .map(|t| t.to_string())
                .unwrap_or_else(|| "?".to_string());
            xml.push_str(&format!(
                "<p><em>由 {} 模型翻译，花费 {} tokens</em></p>",
                esc(model),
                tokens
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

pub(crate) fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
