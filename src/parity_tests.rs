//! Checks against output captured from the old Python/Django version
//! (tests/fixtures/python_reference.json).

use chrono::NaiveDateTime;
use serde_json::Value;

use crate::db::Post;
use crate::rendering::{render_article, sanitize, split_text};
use crate::text::slugify;

fn fixtures() -> Value {
    serde_json::from_str(include_str!("../tests/fixtures/python_reference.json")).unwrap()
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap()
}

#[test]
fn render_article_matches_python() {
    for case in fixtures()["render"].as_array().unwrap() {
        let (input, expected) = (s(&case["input"]), s(&case["output"]));
        // Highlighted code is produced by a different highlighter; plain text blocks must match.
        if input.contains("data-language=\"python\"") {
            continue;
        }
        assert_eq!(render_article(input), expected, "input: {input}");
    }
}

#[test]
fn split_text_matches_python() {
    for case in fixtures()["split"].as_array().unwrap() {
        let (prose, code) = split_text(s(&case["input"]));
        assert_eq!(prose, s(&case["prose"]), "input: {}", s(&case["input"]));
        assert_eq!(code, s(&case["code"]), "input: {}", s(&case["input"]));
    }
}

#[test]
fn excerpt_and_reading_time_match_python() {
    for case in fixtures()["posts"].as_array().unwrap() {
        let post = Post {
            id: 1,
            title: String::new(),
            slug: String::new(),
            body_delta: "{}".into(),
            body_html: s(&case["body_html"]).into(),
            created_at: NaiveDateTime::default(),
            updated_at: NaiveDateTime::default(),
        };
        assert_eq!(post.excerpt(), s(&case["excerpt"]));
        assert_eq!(post.reading_minutes(), case["reading_minutes"].as_u64().unwrap());
    }
}

#[test]
fn slugify_matches_python() {
    for case in fixtures()["slugify"].as_array().unwrap() {
        assert_eq!(slugify(s(&case["input"])), s(&case["output"]), "input: {:?}", s(&case["input"]));
    }
}

#[test]
fn sanitize_matches_python() {
    for case in fixtures()["sanitize"].as_array().unwrap() {
        assert_eq!(sanitize(s(&case["input"])).replace("&nbsp;", " "), s(&case["output"]));
    }
}
