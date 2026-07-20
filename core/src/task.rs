//! The task line: model, parser and writer.
//!
//! The on-disk format must stay a plain Markdown checklist, so any editor can
//! read and write it:
//!
//! ```text
//! - [ ] Comprar leite <!--id:a1b2c3-->
//! - [x] Pagar internet <!--id:d4e5f6 origin:Compras-->
//! ```

use serde::{Deserialize, Serialize};

const COMMENT_OPEN: &str = "<!--";
const COMMENT_CLOSE: &str = "-->";

/// A single task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    /// Stable id. `None` for a checkbox typed by hand outside the app, which
    /// the app adopts by assigning an id on the next write.
    pub id: Option<String>,
    pub text: String,
    pub done: bool,
    /// List the task came from. Only meaningful inside `Completas.md`, where
    /// it powers the undo.
    pub origin: Option<String>,
    /// Free-form metadata, kept verbatim so a future version of the app (or
    /// another tool) can add fields without this one destroying them.
    pub meta: Option<serde_json::Value>,
    /// Leading whitespace, preserved so nested checklists survive a rewrite.
    pub indent: String,
}

impl Task {
    /// Builds a new, unsaved task. The id is assigned when it reaches a file.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            id: None,
            text: text.into(),
            done: false,
            origin: None,
            meta: None,
            indent: String::new(),
        }
    }

    /// Parses one line. Returns `None` when the line is not a task, which is
    /// the normal case for headings, blank lines and free prose — a notebook
    /// is a human document, not an app-owned database.
    pub fn parse(line: &str) -> Option<Self> {
        let indent_len = line.len() - line.trim_start().len();
        let indent = line[..indent_len].to_string();
        let rest = &line[indent_len..];

        // Bullet marker: "- ", "* " or "+ ", as Markdown allows.
        let rest = rest
            .strip_prefix("- ")
            .or_else(|| rest.strip_prefix("* "))
            .or_else(|| rest.strip_prefix("+ "))?;

        // Checkbox: "[ ]" or "[x]" / "[X]".
        let (done, rest) = match rest.strip_prefix("[ ]") {
            Some(r) => (false, r),
            None => (
                true,
                rest.strip_prefix("[x]").or_else(|| rest.strip_prefix("[X]"))?,
            ),
        };

        let body = rest.strip_prefix(' ').unwrap_or(rest);
        let (text, comment) = split_trailing_comment(body);

        let mut task = Self {
            id: None,
            text: text.trim().to_string(),
            done,
            origin: None,
            meta: None,
            indent,
        };

        if let Some(comment) = comment {
            task.apply_comment(&comment);
        }

        Some(task)
    }

    fn apply_comment(&mut self, comment: &str) {
        self.id = read_field(comment, "id:").map(|v| v.to_string());
        self.origin = read_field(comment, "origin:").map(|v| v.to_string());
        self.meta = read_meta(comment);
    }

    /// Renders the task back to a Markdown line.
    pub fn render(&self) -> String {
        let checkbox = if self.done { "[x]" } else { "[ ]" };
        let mut line = format!("{}- {} {}", self.indent, checkbox, self.text);

        let mut fields: Vec<String> = Vec::new();
        if let Some(id) = &self.id {
            fields.push(format!("id:{id}"));
        }
        if let Some(origin) = &self.origin {
            fields.push(format!("origin:{origin}"));
        }
        // meta goes last: its JSON may contain spaces, so keeping it at the
        // end lets the parser read it by brace matching to the end.
        if let Some(meta) = &self.meta {
            fields.push(format!("meta:{meta}"));
        }

        if !fields.is_empty() {
            line.push_str(&format!(" {COMMENT_OPEN}{}{COMMENT_CLOSE}", fields.join(" ")));
        }
        line
    }
}

/// Splits the trailing `<!--...-->` off a task body, if present.
fn split_trailing_comment(body: &str) -> (&str, Option<String>) {
    let trimmed = body.trim_end();
    let Some(stripped) = trimmed.strip_suffix(COMMENT_CLOSE) else {
        return (body, None);
    };
    let Some(open_at) = stripped.rfind(COMMENT_OPEN) else {
        return (body, None);
    };
    let inner = stripped[open_at + COMMENT_OPEN.len()..].to_string();
    (&trimmed[..open_at], Some(inner))
}

/// Reads a `key:value` field, where the value runs to the next whitespace.
fn read_field<'a>(comment: &'a str, key: &str) -> Option<&'a str> {
    let start = find_field(comment, key)? + key.len();
    let value = comment[start..]
        .split_whitespace()
        .next()
        .unwrap_or_default();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

/// Reads `meta:{...}` by brace matching, so JSON containing spaces survives.
fn read_meta(comment: &str) -> Option<serde_json::Value> {
    let start = find_field(comment, "meta:")? + "meta:".len();
    let json = &comment[start..];
    if !json.starts_with('{') {
        return None;
    }

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut end = None;

    for (i, c) in json.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    end = Some(i + 1);
                    break;
                }
            }
            _ => {}
        }
    }

    // Malformed metadata is dropped rather than failing the whole parse: one
    // bad line must never make a notebook unreadable.
    serde_json::from_str(&json[..end?]).ok()
}

/// Finds a field key at a token boundary, so `origin:` does not match inside
/// another value.
fn find_field(comment: &str, key: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(found) = comment[from..].find(key) {
        let at = from + found;
        let at_boundary = at == 0
            || comment[..at]
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace);
        if at_boundary {
            return Some(at);
        }
        from = at + key.len();
    }
    None
}
