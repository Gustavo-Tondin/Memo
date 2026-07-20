//! The task: model, parser and writer.
//!
//! Format spec: `docs/project-strategy.md`, section 3.2. The rule that
//! organises everything:
//!
//! > What the user writes stays visible. What the app controls goes in the
//! > hidden comment.
//!
//! A task can span several lines:
//!
//! ```text
//! - [ ] Comprar material da obra <!--id:g7h8i9-->
//!   @2026-07-25 #casa #urgent !2
//!   Falar com o Jorge antes, ele tem desconto.
//!   repeat: every-week
//!   - [ ] Cimento
//! ```
//!
//! Everything indented under a task belongs to it. Which kind of line it is
//! gets decided by shape, never by position, because the file is written by
//! humans in whatever order they like.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

const COMMENT_OPEN: &str = "<!--";
const COMMENT_CLOSE: &str = "-->";

/// Indentation of one level. Everything under a task uses exactly one level.
pub const INDENT: &str = "  ";

/// Named fields the app understands. Anything else stays description, so a
/// line like `lembrar: ligar pro Jorge` is never mistaken for a field.
const KNOWN_FIELDS: [&str; 1] = ["repeat"];

/// Tags with meaning to the app. Always stored in English; translated only
/// when displayed.
pub const TAG_URGENT: &str = "urgent";
pub const TAG_PINNED: &str = "pinned";

/// How often a task comes back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repeat {
    pub every: u32,
    pub unit: RepeatUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepeatUnit {
    Day,
    Week,
    Month,
}

impl RepeatUnit {
    fn as_str(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        // Both singular and plural, since `every-3-days` reads better than
        // `every-3-day` and people will write it that way.
        match text {
            "day" | "days" => Some(Self::Day),
            "week" | "weeks" => Some(Self::Week),
            "month" | "months" => Some(Self::Month),
            _ => None,
        }
    }
}

impl Repeat {
    /// Parses `every-week` or `every-3-days`.
    pub fn parse(text: &str) -> Option<Self> {
        let rest = text.trim().strip_prefix("every-")?;
        match rest.split_once('-') {
            Some((count, unit)) => Some(Self {
                every: count.parse().ok().filter(|n| *n > 0)?,
                unit: RepeatUnit::parse(unit)?,
            }),
            None => Some(Self {
                every: 1,
                unit: RepeatUnit::parse(rest)?,
            }),
        }
    }

    pub fn render(self) -> String {
        if self.every == 1 {
            format!("every-{}", self.unit.as_str())
        } else {
            // Plural reads naturally: every-3-days.
            format!("every-{}-{}s", self.every, self.unit.as_str())
        }
    }
}

/// A checkbox nested under a task. Text and state only — giving subtasks their
/// own dates and tags would turn the model into a recursive tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Subtask {
    pub text: String,
    pub done: bool,
}

/// A single task.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Task {
    /// Stable id, assigned only when the task needs to be addressed — pulled
    /// into a period or completed. A plain checklist never grows comments.
    pub id: Option<String>,
    pub text: String,
    pub done: bool,
    /// List the task came from. Only meaningful in the completed list, where
    /// it powers the undo.
    pub origin: Option<String>,
    /// Only written when a repeating task has no due date to anchor on.
    pub created: Option<NaiveDate>,
    pub due: Option<NaiveDate>,
    /// 1 (highest) to 3 (lowest).
    pub priority: Option<u8>,
    pub tags: Vec<String>,
    /// Free text under the task, kept line by line as written.
    pub description: Vec<String>,
    pub subtasks: Vec<Subtask>,
    pub repeat: Option<Repeat>,
    /// Leading whitespace of the task line, so a task nested inside someone
    /// else's markdown structure survives a rewrite.
    pub indent: String,
    /// Metadata written by an older version of the app. Preserved verbatim so
    /// upgrading and downgrading does not destroy data.
    pub meta: Option<serde_json::Value>,
}

impl Task {
    /// Builds a new, unsaved task.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Default::default()
        }
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// Marked urgent by hand. Being urgent *because of a date* is a decision
    /// that needs the clock and a preference, so it lives in the notebook.
    pub fn is_marked_urgent(&self) -> bool {
        self.has_tag(TAG_URGENT)
    }

    pub fn is_pinned(&self) -> bool {
        self.has_tag(TAG_PINNED)
    }

    /// Parses the first line of a task. Returns `None` for anything that is
    /// not a checkbox — headings, prose and blank lines are the normal case in
    /// a document a human writes.
    pub fn parse(line: &str) -> Option<Self> {
        let indent_len = line.len() - line.trim_start().len();
        let indent = line[..indent_len].to_string();
        let (done, body) = parse_checkbox(&line[indent_len..])?;
        let (text, comment) = split_trailing_comment(body);

        let mut task = Self {
            text: text.trim().to_string(),
            done,
            indent,
            ..Default::default()
        };
        if let Some(comment) = comment {
            task.apply_comment(&comment);
        }
        Some(task)
    }

    /// Absorbs a line indented under this task. Returns whether it was taken —
    /// `false` means the line is not ours and belongs to the document.
    pub(crate) fn absorb(&mut self, line: &str) -> bool {
        let body = line.trim_start();
        if body.is_empty() {
            return false;
        }

        if let Some((done, text)) = parse_checkbox(body) {
            let (text, _) = split_trailing_comment(text);
            self.subtasks.push(Subtask {
                text: text.trim().to_string(),
                done,
            });
            return true;
        }

        if let Some((key, value)) = parse_field(body) {
            if key == "repeat" {
                // An unparseable value would be silently dropped on rewrite,
                // so keep it as description instead.
                if let Some(repeat) = Repeat::parse(value) {
                    self.repeat = Some(repeat);
                    return true;
                }
            }
        }

        if let Some(metadata) = Metadata::parse(body) {
            self.due = metadata.due.or(self.due);
            self.priority = metadata.priority.or(self.priority);
            for tag in metadata.tags {
                if !self.has_tag(&tag) {
                    self.tags.push(tag);
                }
            }
            return true;
        }

        self.description.push(body.to_string());
        true
    }

    fn apply_comment(&mut self, comment: &str) {
        self.id = read_field(comment, "id:").map(str::to_string);
        self.origin = read_field(comment, "origin:").map(str::to_string);
        self.created = read_field(comment, "created:").and_then(parse_date);
        self.meta = read_meta(comment);
    }

    /// Renders the task, first line first. A task with nothing extra is a
    /// single plain checkbox line.
    pub fn render(&self) -> Vec<String> {
        let checkbox = if self.done { "[x]" } else { "[ ]" };
        let mut first = format!("{}- {} {}", self.indent, checkbox, self.text);
        if let Some(comment) = self.render_comment() {
            first.push(' ');
            first.push_str(&comment);
        }

        let mut lines = vec![first];
        let child_indent = format!("{}{INDENT}", self.indent);

        // Order on write is fixed; order on read is not. Metadata first
        // because it is what the eye looks for.
        if let Some(metadata) = self.render_metadata() {
            lines.push(format!("{child_indent}{metadata}"));
        }
        for line in &self.description {
            lines.push(format!("{child_indent}{line}"));
        }
        if let Some(repeat) = self.repeat {
            lines.push(format!("{child_indent}repeat: {}", repeat.render()));
        }
        for subtask in &self.subtasks {
            let checkbox = if subtask.done { "[x]" } else { "[ ]" };
            lines.push(format!("{child_indent}- {checkbox} {}", subtask.text));
        }
        lines
    }

    /// The whole task as text, lines joined. Convenience for tests and for
    /// anything that wants the block as one string.
    pub fn render_block(&self) -> String {
        self.render().join("\n")
    }

    fn render_metadata(&self) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(due) = self.due {
            parts.push(format!("@{due}"));
        }
        for tag in &self.tags {
            parts.push(format!("#{tag}"));
        }
        if let Some(priority) = self.priority {
            parts.push(format!("!{priority}"));
        }
        (!parts.is_empty()).then(|| parts.join(" "))
    }

    fn render_comment(&self) -> Option<String> {
        let mut fields: Vec<String> = Vec::new();
        if let Some(id) = &self.id {
            fields.push(format!("id:{id}"));
        }
        if let Some(origin) = &self.origin {
            fields.push(format!("origin:{origin}"));
        }
        if let Some(created) = self.created {
            fields.push(format!("created:{created}"));
        }
        // meta goes last: its JSON may contain spaces, so keeping it at the
        // end lets the parser read it by brace matching to the end.
        if let Some(meta) = &self.meta {
            fields.push(format!("meta:{meta}"));
        }
        (!fields.is_empty())
            .then(|| format!("{COMMENT_OPEN}{}{COMMENT_CLOSE}", fields.join(" ")))
    }
}

/// The `@date #tag !priority` line.
#[derive(Debug, Default, PartialEq)]
struct Metadata {
    due: Option<NaiveDate>,
    priority: Option<u8>,
    tags: Vec<String>,
}

impl Metadata {
    /// Parses a line made **only** of metadata tokens. Any loose word makes it
    /// description instead — that is what lets a description start with `#`.
    fn parse(line: &str) -> Option<Self> {
        let mut metadata = Self::default();
        let mut found = false;

        for token in line.split_whitespace() {
            found = true;
            if let Some(rest) = token.strip_prefix('@') {
                metadata.due = Some(parse_date(rest)?);
            } else if let Some(rest) = token.strip_prefix('#') {
                if rest.is_empty() {
                    return None;
                }
                metadata.tags.push(rest.to_string());
            } else if let Some(rest) = token.strip_prefix('!') {
                let priority: u8 = rest.parse().ok()?;
                if !(1..=3).contains(&priority) {
                    return None;
                }
                metadata.priority = Some(priority);
            } else {
                return None;
            }
        }
        found.then_some(metadata)
    }
}

/// `- [ ] text`, `* [x] text`, `+ [X] text`.
fn parse_checkbox(body: &str) -> Option<(bool, &str)> {
    let rest = body
        .strip_prefix("- ")
        .or_else(|| body.strip_prefix("* "))
        .or_else(|| body.strip_prefix("+ "))?;

    let (done, rest) = match rest.strip_prefix("[ ]") {
        Some(r) => (false, r),
        None => (
            true,
            rest.strip_prefix("[x]").or_else(|| rest.strip_prefix("[X]"))?,
        ),
    };
    Some((done, rest.strip_prefix(' ').unwrap_or(rest)))
}

/// `key: value`, but only for keys the app owns.
fn parse_field(body: &str) -> Option<(&str, &str)> {
    let (key, value) = body.split_once(':')?;
    let key = key.trim();
    KNOWN_FIELDS
        .contains(&key)
        .then(|| (key, value.trim()))
}

/// Accepts the canonical ISO form and the two shapes people type by hand.
/// Written back as ISO, so a hand-typed date is normalised on the next save.
pub fn parse_date(text: &str) -> Option<NaiveDate> {
    let text = text.trim();
    for format in ["%Y-%m-%d", "%d-%m-%Y", "%d/%m/%Y"] {
        if let Ok(date) = NaiveDate::parse_from_str(text, format) {
            return Some(date);
        }
    }
    None
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
    let value = comment[start..].split_whitespace().next().unwrap_or_default();
    (!value.is_empty()).then_some(value)
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

/// Unknown named fields are kept as description, so nothing is lost. Exposed
/// for tests that check that promise.
#[allow(dead_code)]
pub(crate) fn known_fields() -> BTreeMap<&'static str, ()> {
    KNOWN_FIELDS.iter().map(|k| (*k, ())).collect()
}
