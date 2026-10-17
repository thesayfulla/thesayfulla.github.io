"""Turn the editor's HTML into the published article HTML.

The editor sends simple semantic HTML. We sanitize it, then expand the two
special pieces: code blocks (highlighted with Pygments) and embeds
(YouTube/Vimeo iframes, GitHub gists, posts on X; other links become plain links).
"""

import html
import re
from html.parser import HTMLParser
from urllib.parse import parse_qs, urlparse

import nh3
from pygments import highlight
from pygments.formatters import HtmlFormatter
from pygments.lexers import get_lexer_by_name
from pygments.util import ClassNotFound

ALLOWED_TAGS = {
    "p", "br", "strong", "em", "u", "s", "code", "a",
    "h3", "h4", "blockquote", "pre", "ol", "ul", "li",
    "figure", "img", "figcaption", "div",
}
ALLOWED_ATTRS = {
    "a": {"href"},
    "pre": {"data-language"},
    "img": {"src", "alt"},
    "div": {"data-embed"},
}

# Quill / highlight.js language keys -> Pygments lexer names
LANGUAGE_ALIASES = {"plain": "text", "xml": "html", "": "text"}

PRE_RE = re.compile(r'<pre data-language="([\w+#-]*)">(.*?)</pre>', re.S)
EMBED_RE = re.compile(r'<div data-embed="([^"]+)"></div>')
GIST_RE = re.compile(r"^https://gist\.github\.com/[\w-]+/[0-9a-f]+/?$")
TWEET_RE = re.compile(r"^https://(?:www\.)?(?:x|twitter)\.com/\w+/status/\d+")
FIGURE_IMG_RE = re.compile(r"<img ")
EMPTY_TAIL_RE = re.compile(r"(?:<p>(?:\s|<br>)*</p>)+$")


def sanitize(raw):
    return nh3.clean(
        raw,
        tags=ALLOWED_TAGS,
        attributes=ALLOWED_ATTRS,
        url_schemes={"http", "https", "mailto"},
        link_rel="noopener noreferrer",
    )


def render_code(match):
    language = LANGUAGE_ALIASES.get(match.group(1), match.group(1))
    code = html.unescape(match.group(2)).strip("\n")
    if not code.strip():
        return ""
    try:
        lexer = get_lexer_by_name(language)
    except ClassNotFound:
        lexer, language = get_lexer_by_name("text"), "text"
    highlighted = highlight(code, lexer, HtmlFormatter(cssclass="highlight"))
    label = "" if language == "text" else language
    return (
        '<div class="codeblock"><div class="codeblock-bar">'
        f"<span>{html.escape(label)}</span>"
        '<button type="button" class="codeblock-copy">copy</button></div>'
        f"{highlighted}</div>"
    )


def youtube_id(url):
    u = urlparse(url)
    host = u.netloc.removeprefix("www.").removeprefix("m.")
    if host == "youtu.be":
        return u.path.lstrip("/").split("/")[0]
    if host == "youtube.com":
        if u.path == "/watch":
            return parse_qs(u.query).get("v", [None])[0]
        for prefix in ("/shorts/", "/embed/", "/live/"):
            if u.path.startswith(prefix):
                return u.path[len(prefix):].split("/")[0]
    return None


def vimeo_id(url):
    u = urlparse(url)
    if u.netloc.removeprefix("www.") in ("vimeo.com", "player.vimeo.com"):
        m = re.search(r"/(\d+)", u.path)
        return m.group(1) if m else None
    return None


def render_embed(url):
    url = html.unescape(url).strip()
    if vid := youtube_id(url):
        src = "https://www.youtube-nocookie.com/embed/" + html.escape(vid)
        return f'<div class="embed video"><iframe src="{src}" allowfullscreen loading="lazy" referrerpolicy="strict-origin-when-cross-origin"></iframe></div>'
    if vid := vimeo_id(url):
        return f'<div class="embed video"><iframe src="https://player.vimeo.com/video/{vid}" allowfullscreen loading="lazy" referrerpolicy="strict-origin-when-cross-origin"></iframe></div>'
    if GIST_RE.match(url):
        return f'<div class="gist-embed"><script src="{html.escape(url.rstrip("/"))}.js"></script></div>'
    safe = html.escape(url)
    if TWEET_RE.match(url):
        tweet = safe.replace("://x.com/", "://twitter.com/").replace("://www.x.com/", "://twitter.com/")
        return (
            f'<div class="embed tweet"><blockquote class="twitter-tweet" data-dnt="true">'
            f'<a href="{tweet}">{safe}</a></blockquote>'
            '<script async src="https://platform.twitter.com/widgets.js"></script></div>'
        )
    return f'<p class="embed-link"><a href="{safe}" target="_blank" rel="noopener noreferrer">{safe}</a></p>'


def render_article(raw_html):
    """Editor HTML -> final article HTML (safe to output with |safe)."""
    clean = sanitize(raw_html.replace("&nbsp;", " "))
    clean = PRE_RE.sub(render_code, clean)
    clean = EMBED_RE.sub(lambda m: render_embed(m.group(1)), clean)
    clean = FIGURE_IMG_RE.sub('<img loading="lazy" ', clean)
    return EMPTY_TAIL_RE.sub("", clean)


PROSE_TAGS = {"p", "h3", "h4", "blockquote", "li"}
SKIP_TAGS = {"div", "figure", "pre"}  # code blocks, embeds, images + captions
VOID_TAGS = {"br", "img", "hr", "input"}


class _TextSplitter(HTMLParser):
    """Separates an article's readable prose from the code inside it."""

    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.stack = []
        self.prose, self.code = [], []

    def handle_starttag(self, tag, attrs):
        if tag in VOID_TAGS:
            return
        skip = tag in SKIP_TAGS or (tag == "p" and "embed-link" in (dict(attrs).get("class") or ""))
        self.stack.append((tag, skip))

    def handle_endtag(self, tag):
        # Pop back to the matching open tag (tolerates sloppy nesting).
        for i in range(len(self.stack) - 1, -1, -1):
            if self.stack[i][0] == tag:
                del self.stack[i:]
                break

    def handle_data(self, data):
        tags = [t for t, _ in self.stack]
        if "pre" in tags:
            self.code.append(data)
        elif not any(skip for _, skip in self.stack) and PROSE_TAGS.intersection(tags):
            self.prose.append(data)


def split_text(rendered_html):
    """(prose, code) plain text of a rendered article."""
    parser = _TextSplitter()
    parser.feed(rendered_html)
    parser.close()
    squash = lambda parts: re.sub(r"\s+", " ", " ".join(parts)).strip()
    return squash(parser.prose), squash(parser.code)


def prose_text(rendered_html):
    """Readable text only — no code, image captions or embed links."""
    return split_text(rendered_html)[0]
