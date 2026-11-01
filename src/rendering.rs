//! Turn the editor's HTML into the published article HTML.
//!
//! The editor sends simple semantic HTML. We sanitize it, then expand the two
//! special pieces: code blocks (syntax highlighted) and embeds
//! (YouTube/Vimeo iframes, GitHub gists, posts on X; other links become plain links).

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::{Captures, Regex};
use url::Url;

use crate::highlight;
use crate::text::{escape, unescape};

const ALLOWED_TAGS: &[&str] = &[
    "p", "br", "strong", "em", "u", "s", "code", "a",
    "h3", "h4", "blockquote", "pre", "ol", "ul", "li",
    "figure", "img", "figcaption", "div",
];
const ALLOWED_ATTRS: &[(&str, &[&str])] = &[
    ("a", &["href"]),
    ("pre", &["data-language"]),
    ("img", &["src", "alt"]),
    ("div", &["data-embed"]),
];

/// Editor language keys -> highlighter language names.
fn language_alias(key: &str) -> &str {
    match key {
        "plain" | "" => "text",
        "xml" => "html",
        other => other,
    }
}

macro_rules! regex {
    ($re:literal) => {{
        static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new($re).unwrap());
        &*RE
    }};
}

static CLEANER: LazyLock<ammonia::Builder<'static>> = LazyLock::new(|| {
    let mut b = ammonia::Builder::default();
    b.tags(ALLOWED_TAGS.iter().copied().collect())
        .tag_attributes(
            ALLOWED_ATTRS
                .iter()
                .map(|(tag, attrs)| (*tag, attrs.iter().copied().collect::<HashSet<_>>()))
                .collect::<HashMap<_, _>>(),
        )
        .url_schemes(["http", "https", "mailto"].into_iter().collect())
        .link_rel(Some("noopener noreferrer"));
    b
});

pub fn sanitize(raw: &str) -> String {
    CLEANER.clean(raw).to_string()
}

fn render_code(caps: &Captures) -> String {
    let mut language = language_alias(&caps[1]).to_string();
    let code = unescape(&caps[2]);
    let code = code.trim_matches('\n');
    if code.trim().is_empty() {
        return String::new();
    }
    let highlighted = match highlight::highlight(code, &language) {
        Some(html) => html,
        None => {
            language = "text".into();
            highlight::plain(code)
        }
    };
    let label = if language == "text" { "" } else { &language };
    format!(
        "<div class=\"codeblock\"><div class=\"codeblock-bar\"><span>{}</span>\
         <button type=\"button\" class=\"codeblock-copy\">copy</button></div>{highlighted}</div>",
        escape(label)
    )
}

fn non_empty(s: &str) -> Option<String> {
    (!s.is_empty()).then(|| s.to_string())
}

fn host(u: &Url) -> &str {
    u.host_str().unwrap_or("")
}

fn youtube_id(url: &str) -> Option<String> {
    let u = Url::parse(url).ok()?;
    let host = host(&u);
    let host = host.strip_prefix("www.").unwrap_or(host);
    let host = host.strip_prefix("m.").unwrap_or(host);
    let path = u.path();
    if host == "youtu.be" {
        return non_empty(path.trim_start_matches('/').split('/').next().unwrap_or(""));
    }
    if host == "youtube.com" {
        if path == "/watch" {
            return u.query_pairs().find(|(k, v)| k == "v" && !v.is_empty()).map(|(_, v)| v.into_owned());
        }
        for prefix in ["/shorts/", "/embed/", "/live/"] {
            if let Some(rest) = path.strip_prefix(prefix) {
                return non_empty(rest.split('/').next().unwrap_or(""));
            }
        }
    }
    None
}

fn vimeo_id(url: &str) -> Option<String> {
    let u = Url::parse(url).ok()?;
    let host = host(&u);
    if matches!(host.strip_prefix("www.").unwrap_or(host), "vimeo.com" | "player.vimeo.com") {
        return regex!(r"/(\d+)").captures(u.path()).map(|c| c[1].to_string());
    }
    None
}

fn render_embed(url: &str) -> String {
    let url = unescape(url);
    let url = url.trim();
    if let Some(vid) = youtube_id(url) {
        let src = format!("https://www.youtube-nocookie.com/embed/{}", escape(&vid));
        return format!(
            r#"<div class="embed video"><iframe src="{src}" allowfullscreen loading="lazy" referrerpolicy="strict-origin-when-cross-origin"></iframe></div>"#
        );
    }
    if let Some(vid) = vimeo_id(url) {
        return format!(
            r#"<div class="embed video"><iframe src="https://player.vimeo.com/video/{vid}" allowfullscreen loading="lazy" referrerpolicy="strict-origin-when-cross-origin"></iframe></div>"#
        );
    }
    if regex!(r"^https://gist\.github\.com/[\w-]+/[0-9a-f]+/?$").is_match(url) {
        let src = escape(url.trim_end_matches('/'));
        return format!(r#"<div class="gist-embed"><script src="{src}.js"></script></div>"#);
    }
    let safe = escape(url);
    if regex!(r"^https://(?:www\.)?(?:x|twitter)\.com/\w+/status/\d+").is_match(url) {
        let tweet = safe.replace("://x.com/", "://twitter.com/").replace("://www.x.com/", "://twitter.com/");
        return format!(
            "<div class=\"embed tweet\"><blockquote class=\"twitter-tweet\" data-dnt=\"true\">\
             <a href=\"{tweet}\">{safe}</a></blockquote>\
             <script async src=\"https://platform.twitter.com/widgets.js\"></script></div>"
        );
    }
    format!(r#"<p class="embed-link"><a href="{safe}" target="_blank" rel="noopener noreferrer">{safe}</a></p>"#)
}

/// Editor HTML -> final article HTML (safe to output unescaped).
pub fn render_article(raw_html: &str) -> String {
    let clean = sanitize(&raw_html.replace("&nbsp;", " "));
    let clean = regex!(r#"(?s)<pre data-language="([\w+#-]*)">(.*?)</pre>"#).replace_all(&clean, render_code);
    let clean = regex!(r#"<div data-embed="([^"]+)"></div>"#).replace_all(&clean, |c: &Captures| render_embed(&c[1]));
    let clean = regex!(r"<img ").replace_all(&clean, r#"<img loading="lazy" "#);
    regex!(r"^(?:<p>(?:\s|<br>)*</p>)+|(?:<p>(?:\s|<br>)*</p>)+$")
        .replace_all(&clean, "")
        .into_owned()
}

// ---------- Plain text (excerpts, reading time) ----------

const PROSE_TAGS: &[&str] = &["p", "h3", "h4", "blockquote", "li"];
const SKIP_TAGS: &[&str] = &["div", "figure", "pre"]; // code blocks, embeds, images + captions
const VOID_TAGS: &[&str] = &["br", "img", "hr", "input"];

/// Separates an article's readable prose from the code inside it.
#[derive(Default)]
struct TextSplitter {
    stack: Vec<(String, bool)>,
    prose: Vec<String>,
    code: Vec<String>,
}

impl TextSplitter {
    fn start_tag(&mut self, tag: String, attrs: &str) {
        if VOID_TAGS.contains(&tag.as_str()) {
            return;
        }
        let skip = SKIP_TAGS.contains(&tag.as_str()) || (tag == "p" && class_attr(attrs).contains("embed-link"));
        self.stack.push((tag, skip));
    }

    fn end_tag(&mut self, tag: &str) {
        // Pop back to the matching open tag (tolerates sloppy nesting).
        if let Some(i) = self.stack.iter().rposition(|(t, _)| t == tag) {
            self.stack.truncate(i);
        }
    }

    fn data(&mut self, data: String) {
        let in_tag = |name: &str| self.stack.iter().any(|(t, _)| t == name);
        if in_tag("pre") {
            self.code.push(data);
        } else if !self.stack.iter().any(|(_, skip)| *skip) && PROSE_TAGS.iter().any(|t| in_tag(t)) {
            self.prose.push(data);
        }
    }

    fn feed(&mut self, html: &str) {
        let mut rest = html;
        while !rest.is_empty() {
            let Some(lt) = rest.find('<') else {
                self.data(unescape(rest));
                break;
            };
            if lt > 0 {
                self.data(unescape(&rest[..lt]));
            }
            rest = &rest[lt..];
            if let Some(after) = rest.strip_prefix("<!--") {
                rest = after.find("-->").map_or("", |end| &after[end + 3..]);
            } else if let Some(c) = regex!(r"^</([a-zA-Z][^\s/>]*)[^>]*>").captures(rest) {
                self.end_tag(&c[1].to_ascii_lowercase());
                rest = &rest[c[0].len()..];
            } else if let Some(c) = regex!(r#"^<([a-zA-Z][^\s/>]*)((?:[^>"']|"[^"]*"|'[^']*')*)>"#).captures(rest) {
                let tag = c[1].to_ascii_lowercase();
                let attrs = c.get(2).map_or("", |m| m.as_str());
                let self_closing = attrs.trim_end().ends_with('/');
                self.start_tag(tag.clone(), attrs);
                if self_closing {
                    self.end_tag(&tag);
                }
                rest = &rest[c[0].len()..];
            } else {
                self.data("<".into());
                rest = &rest[1..];
            }
        }
    }
}

fn class_attr(attrs: &str) -> String {
    regex!(r#"(?i)(?:^|\s)class\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))"#)
        .captures(attrs)
        .and_then(|c| c.get(1).or(c.get(2)).or(c.get(3)))
        .map(|m| unescape(m.as_str()))
        .unwrap_or_default()
}

fn squash(parts: &[String]) -> String {
    parts.join(" ").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// (prose, code) plain text of a rendered article.
pub fn split_text(rendered_html: &str) -> (String, String) {
    let mut parser = TextSplitter::default();
    parser.feed(rendered_html);
    (squash(&parser.prose), squash(&parser.code))
}

/// Readable text only — no code, image captions or embed links.
pub fn prose_text(rendered_html: &str) -> String {
    split_text(rendered_html).0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeds() {
        assert_eq!(youtube_id("https://www.youtube.com/watch?v=abc&t=1").as_deref(), Some("abc"));
        assert_eq!(youtube_id("https://youtu.be/abc?si=x").as_deref(), Some("abc"));
        assert_eq!(youtube_id("https://youtube.com/watch").as_deref(), None);
        assert_eq!(vimeo_id("https://vimeo.com/123").as_deref(), Some("123"));
        assert!(render_embed("https://example.com/").starts_with("<p class=\"embed-link\">"));
    }

    #[test]
    fn code_blocks() {
        let html = render_article("<pre data-language=\"python\">def f():\n    return \"x\" # hi</pre>");
        assert!(html.starts_with("<div class=\"codeblock\"><div class=\"codeblock-bar\"><span>python</span>"));
        assert!(html.contains("<div class=\"highlight\"><pre><span></span>"));
        assert!(html.contains("<span class=\"kd\">def</span>") || html.contains("<span class=\"k\">def</span>"), "{html}");
        assert!(html.contains("<span class=\"c1\"># hi</span>"), "{html}");
        assert!(html.ends_with("</pre></div>\n</div>"));
        assert_eq!(split_text(&html).1, "def f (): return \"x\" # hi"); // same as the Python version
    }
}
