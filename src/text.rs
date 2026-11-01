//! Small text helpers that behave like the Django ones the site used to rely on.

use std::sync::LazyLock;

use chrono::{Datelike, NaiveDateTime};
use rand::RngExt;
use regex::Regex;
use unicode_normalization::UnicodeNormalization;

/// Escape for HTML text and attribute values: `& < > " '` (like Django's `escape`).
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
    out
}

/// Decode HTML entities (`&amp;`, `&#39;`, `&rsquo;`, …).
pub fn unescape(s: &str) -> String {
    html_escape::decode_html_entities(s).into_owned()
}

/// "Hello, World!" -> "hello-world". Non-ASCII letters are transliterated when
/// possible ("é" -> "e") and dropped otherwise.
pub fn slugify(value: &str) -> String {
    static NOT_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^\w\s-]").unwrap());
    static DASHES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[-\s]+").unwrap());
    let ascii: String = value.nfkd().filter(char::is_ascii).collect();
    let lower = ascii.to_lowercase();
    let cleaned = NOT_WORD.replace_all(&lower, "");
    DASHES.replace_all(&cleaned, "-").trim_matches(['-', '_']).to_string()
}

/// A file name that's safe on disk and in URLs: "my photo (1).png" -> "my_photo_1.png".
pub fn valid_filename(name: &str) -> String {
    static UNSAFE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^-\w.]").unwrap());
    let name = name.trim().replace(' ', "_");
    UNSAFE.replace_all(&name, "").into_owned()
}

/// First `n` characters of `s`.
pub fn truncate_chars(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

pub fn random_string(len: usize) -> String {
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::rng();
    (0..len).map(|_| CHARS[rng.random_range(0..CHARS.len())] as char).collect()
}

const MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June",
    "July", "August", "September", "October", "November", "December",
];

/// Format a date with Django's `date` filter codes: `Y-m-d` -> "2026-10-18",
/// `M j, Y` -> "Oct 18, 2026", `F j, Y` -> "October 18, 2026".
/// Supported: d j m n M F b Y y; `\x` prints x literally; anything else is copied.
pub fn format_date(dt: &NaiveDateTime, format: &str) -> String {
    let month = MONTHS[dt.month0() as usize];
    let mut out = String::new();
    let mut chars = format.chars();
    while let Some(c) = chars.next() {
        match c {
            'd' => out.push_str(&format!("{:02}", dt.day())),
            'j' => out.push_str(&dt.day().to_string()),
            'm' => out.push_str(&format!("{:02}", dt.month())),
            'n' => out.push_str(&dt.month().to_string()),
            'M' => out.push_str(&month[..3]),
            'b' => out.push_str(&month[..3].to_lowercase()),
            'F' => out.push_str(month),
            'Y' => out.push_str(&dt.year().to_string()),
            'y' => out.push_str(&format!("{:02}", dt.year() % 100)),
            '\\' => out.extend(chars.next()),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        let dt = NaiveDateTime::parse_from_str("2026-10-08 05:21:10", "%Y-%m-%d %H:%M:%S").unwrap();
        assert_eq!(format_date(&dt, "Y-m-d"), "2026-10-08");
        assert_eq!(format_date(&dt, "M j, Y"), "Oct 8, 2026");
        assert_eq!(format_date(&dt, "F j, Y"), "October 8, 2026");
    }

    #[test]
    fn filenames() {
        assert_eq!(valid_filename(" my photo (1).png"), "my_photo_1.png");
        assert_eq!(truncate_chars("héllo", 2), "hé");
    }
}
