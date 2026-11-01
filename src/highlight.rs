//! Syntax highlighting for code blocks.
//!
//! Tokens are classified with Sublime Text grammars (syntect, plus bat's extra
//! grammars for TypeScript, Kotlin, Swift, …) and written out with Pygments'
//! short class names (`k`, `s2`, `c1`, …), so `static/css/code.css` styles new
//! posts and the ones published before exactly the same way.

use std::sync::LazyLock;

use syntect::easy::ScopeRangeIterator;
use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(two_face::syntax::extra_newlines);

/// Scope prefix -> Pygments token class. Checked in order, innermost scope first.
const RULES: &[(&str, &str)] = &[
    ("comment.block.documentation", "sd"),
    ("comment.line", "c1"),
    ("comment.block", "cm"),
    ("comment", "c"),
    ("string.regexp", "sr"),
    ("constant.character.escape", "se"),
    ("constant.other.placeholder", "si"),
    ("string.quoted.docstring", "sd"),
    ("string.quoted.single", "s1"),
    ("string.quoted.double", "s2"),
    ("string", "s"),
    ("constant.numeric.integer.hexadecimal", "mh"),
    ("constant.numeric.hex", "mh"),
    ("constant.numeric.integer.binary", "mb"),
    ("constant.numeric.integer.octal", "mo"),
    ("constant.numeric.float", "mf"),
    ("constant.numeric.integer", "mi"),
    ("constant.numeric", "m"),
    ("constant.language", "kc"),
    ("constant.character", "sc"),
    ("constant", "no"),
    ("keyword.operator.word", "ow"),
    ("keyword.operator", "o"),
    ("keyword.control.import", "kn"),
    ("keyword.control.from", "kn"),
    ("keyword.other.import", "kn"),
    ("keyword.declaration", "kd"),
    ("keyword", "k"),
    ("storage.type.function", "k"),
    ("storage.type.class", "k"),
    ("storage.modifier", "kd"),
    ("storage.type", "kt"),
    ("storage", "k"),
    ("support.type.property-name", "nt"),
    ("support.function", "nb"),
    ("support.type", "nb"),
    ("support.class", "nc"),
    ("support.constant", "kc"),
    ("support.variable", "nv"),
    ("entity.name.function.decorator", "nd"),
    ("entity.name.function", "nf"),
    ("entity.name.class", "nc"),
    ("entity.name.struct", "nc"),
    ("entity.name.enum", "nc"),
    ("entity.name.type", "nc"),
    ("entity.other.inherited-class", "nc"),
    ("entity.name.namespace", "nn"),
    ("entity.name.tag", "nt"),
    ("entity.other.attribute-name", "na"),
    ("entity.name.label", "nl"),
    ("entity.name.constant", "no"),
    ("entity.name", "n"),
    ("variable.annotation", "nd"),
    ("variable.language", "bp"),
    ("variable.other.constant", "no"),
    ("variable", "n"),
    ("markup.inserted", "gi"),
    ("markup.deleted", "gd"),
    ("markup.changed", "gu"),
    ("meta.diff.range", "gu"),
    ("meta.diff.header", "gh"),
    ("markup.heading", "gh"),
    ("markup.bold", "gs"),
    ("markup.italic", "ge"),
    ("invalid", "err"),
    ("punctuation", "p"),
];

static COMPILED_RULES: LazyLock<Vec<(Scope, &'static str)>> =
    LazyLock::new(|| RULES.iter().map(|(s, c)| (Scope::new(s).unwrap(), *c)).collect());
static PUNCTUATION_DEFINITION: LazyLock<Scope> = LazyLock::new(|| Scope::new("punctuation.definition").unwrap());

fn token_class(stack: &ScopeStack) -> Option<&'static str> {
    let mut saw_punctuation = false;
    for scope in stack.as_slice().iter().rev() {
        // Quote marks, comment markers etc. take the class of what they delimit.
        if PUNCTUATION_DEFINITION.is_prefix_of(*scope) {
            saw_punctuation = true;
            continue;
        }
        if let Some((_, class)) = COMPILED_RULES.iter().find(|(prefix, _)| prefix.is_prefix_of(*scope)) {
            return Some(class);
        }
    }
    saw_punctuation.then_some("p")
}

fn find_syntax(language: &str) -> Option<&'static SyntaxReference> {
    let token = match language {
        "csharp" | "c#" => "cs",
        "shell" | "sh" | "zsh" => "bash",
        "golang" => "go",
        "js" => "javascript",
        "ts" => "typescript",
        "py" | "python3" => "python",
        "yml" => "yaml",
        "c++" => "cpp",
        "kt" => "kotlin",
        "rb" => "ruby",
        "rs" => "rust",
        "patch" => "diff",
        other => other,
    };
    SYNTAXES.find_syntax_by_token(token)
}

/// Same escaping as Pygments' HTML formatter.
fn escape(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
}

fn wrap(body: &str) -> String {
    let newline = if body.ends_with('\n') { "" } else { "\n" };
    format!("<div class=\"highlight\"><pre><span></span>{body}{newline}</pre></div>\n")
}

/// Unhighlighted code in the same markup.
pub fn plain(code: &str) -> String {
    let mut body = String::new();
    escape(&code.replace("\r\n", "\n"), &mut body);
    wrap(&body)
}

/// Highlighted HTML for `code`, or None if the language is unknown.
pub fn highlight(code: &str, language: &str) -> Option<String> {
    if language == "text" {
        return Some(plain(code));
    }
    let syntax = find_syntax(language)?;
    let code = code.replace("\r\n", "\n");
    let mut state = ParseState::new(syntax);
    let mut stack = ScopeStack::new();
    let mut body = String::new();

    // Runs of text with the same class are merged; spans never cross a line break.
    let mut run = String::new();
    let mut run_class: Option<&str> = None;
    let flush = |body: &mut String, run: &mut String, class: Option<&str>| {
        if run.is_empty() {
            return;
        }
        match class {
            Some(class) => {
                body.push_str(&format!("<span class=\"{class}\">"));
                escape(run, body);
                body.push_str("</span>");
            }
            None => escape(run, body),
        }
        run.clear();
    };

    for line in LinesWithEndings::from(&code) {
        let Ok(ops) = state.parse_line(line, &SYNTAXES) else {
            return Some(plain(&code));
        };
        for (range, op) in ScopeRangeIterator::new(&ops, line) {
            if stack.apply(op).is_err() {
                return Some(plain(&code));
            }
            for (i, piece) in line[range].split('\n').enumerate() {
                if i > 0 {
                    flush(&mut body, &mut run, run_class);
                    body.push('\n');
                }
                if piece.is_empty() {
                    continue;
                }
                let class = token_class(&stack);
                if class != run_class {
                    flush(&mut body, &mut run, run_class);
                    run_class = class;
                }
                run.push_str(piece);
            }
        }
    }
    flush(&mut body, &mut run, run_class);
    Some(wrap(&body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_languages_are_known() {
        for lang in [
            "python", "javascript", "typescript", "html", "css", "bash", "json", "yaml", "sql", "go", "rust",
            "java", "kotlin", "swift", "c", "cpp", "csharp", "php", "ruby", "diff",
        ] {
            assert!(find_syntax(lang).is_some(), "{lang}");
        }
        assert!(highlight("x", "nosuchlang").is_none());
    }

    #[test]
    fn sql() {
        let html = highlight("CREATE INDEX idx ON users(email);", "sql").unwrap();
        assert!(html.contains("<span class=\"k\">CREATE</span>"), "{html}");
        assert!(html.ends_with(";</span>\n</pre></div>\n") || html.ends_with(";\n</pre></div>\n"), "{html}");
    }

    #[test]
    fn plain_text() {
        assert_eq!(plain("a < b"), "<div class=\"highlight\"><pre><span></span>a &lt; b\n</pre></div>\n");
    }
}
