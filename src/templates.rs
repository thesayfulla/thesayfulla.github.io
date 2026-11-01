//! Jinja templates (minijinja), loaded from templates/ and reloaded when they change.
//!
//! Extras on top of minijinja's built-ins:
//!   static("css/site.css")      -> URL of a static file (content-hashed in the static export)
//!   now("F j, Y")               -> today's date
//!   value|date("M j, Y")        -> Django-style date formatting
//!   value|json_script("id")     -> <script id="id" type="application/json">…</script>

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use chrono::NaiveDateTime;
use minijinja::value::Value;
use minijinja::{AutoEscape, Environment, Error, ErrorKind, escape_formatter, path_loader, syntax::SyntaxConfig};

use crate::config::STATIC_URL;
use crate::text::{escape, format_date};

/// `static_manifest` maps "css/site.css" -> "css/site.b3954303e4cb.css"; without one,
/// files are served under their own names.
pub fn environment(templates_dir: &Path, static_manifest: Option<HashMap<String, String>>) -> Environment<'static> {
    let mut env = Environment::new();
    env.set_loader(path_loader(templates_dir));
    // Keep output byte-for-byte like the Django templates it came from.
    env.set_syntax(SyntaxConfig::builder().keep_trailing_newline(true).build().unwrap());
    env.set_formatter(|out, state, value| {
        if !matches!(state.auto_escape(), AutoEscape::Html) || value.is_safe() {
            return escape_formatter(out, state, value);
        }
        if !(value.is_none() || value.is_undefined()) {
            out.write_str(&escape(&value.to_string()))?;
        }
        Ok(())
    });

    let manifest = Arc::new(static_manifest);
    env.add_function("static", move |path: &str| {
        let name = manifest.as_ref().as_ref().and_then(|m| m.get(path)).map_or(path, String::as_str);
        format!("{STATIC_URL}{name}")
    });
    env.add_function("now", |format: &str| format_date(&crate::db::now(), format));
    env.add_filter("date", |value: &str, format: &str| -> Result<String, Error> {
        let dt = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S")
            .map_err(|e| Error::new(ErrorKind::InvalidOperation, format!("not a date: {value:?}")).with_source(e))?;
        Ok(format_date(&dt, format))
    });
    env.add_filter("json_script", |value: Value, id: &str| -> Result<Value, Error> {
        let json = serde_json::to_string(&value)
            .map_err(|e| Error::new(ErrorKind::BadSerialization, "can't serialize").with_source(e))?
            .replace('<', "\\u003C")
            .replace('>', "\\u003E")
            .replace('&', "\\u0026");
        Ok(Value::from_safe_string(format!(
            r#"<script id="{}" type="application/json">{json}</script>"#,
            escape(id)
        )))
    });
    env
}

#[cfg(test)]
mod tests {
    use super::*;
    use minijinja::context;

    #[test]
    fn escaping_matches_django() {
        let env = environment(Path::new("templates"), None);
        let out = env
            .render_named_str("t.html", "{{ a }}|{{ b|safe }}|{{ n }}", context! { a => "<a href='/x'>&\"", b => "<b>", n => () })
            .unwrap();
        assert_eq!(out, "&lt;a href=&#x27;/x&#x27;&gt;&amp;&quot;|<b>|");
    }

    #[test]
    fn json_script() {
        let env = environment(Path::new("templates"), None);
        let out = env.render_named_str("t.html", "{{ v|json_script('x') }}", context! { v => "</script>" }).unwrap();
        let expected = format!(r#"<script id="x" type="application/json">"{lt}/script{gt}"</script>"#, lt = r"\u003C", gt = r"\u003E");
        assert_eq!(out, expected);
    }
}
