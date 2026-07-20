//! The notebook's `.memo/config.json`.
//!
//! Holds the preferences that belong to the *notebook* and therefore travel
//! with it when it syncs. Machine preferences (last window, last notebook
//! opened) live in the OS config folder instead, and never here.
//!
//! Reading is deliberately forgiving (spec 3.4): a missing key takes the
//! default, a malformed value takes the default, and an unreadable file is
//! recreated. What is never forgiven is *losing* data — an unknown key
//! written by another version of the app survives a rewrite untouched.

use std::path::Path;

use serde_json::{Map, Value};

use crate::clock::{TurnOffset, WeekStart};
use crate::error::{Error, IoContext, Result};

/// Schema version this build understands. A notebook declaring more than this
/// was written by a newer app and opens read-only.
pub const SUPPORTED_SCHEMA_VERSION: u64 = 1;

/// What happens to unfinished tasks when the period turns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RolloverMode {
    /// Empty the state; unfinished tasks go back to being suggestions.
    ///
    /// The default, because the day and the week are an active choice of what
    /// to do in that period — not a queue that piles up on its own.
    #[default]
    Reset,
    /// Keep the pulled references, so they show up already pulled.
    Carry,
}

impl RolloverMode {
    pub fn parse_or_default(text: &str) -> Self {
        match text.trim().to_ascii_lowercase().as_str() {
            "carry" => Self::Carry,
            _ => Self::Reset,
        }
    }

    pub fn render(self) -> &'static str {
        match self {
            Self::Reset => "reset",
            Self::Carry => "carry",
        }
    }
}

/// Rollover preferences for the day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DailyRollover {
    pub mode: RolloverMode,
    pub at: TurnOffset,
}

/// Rollover preferences for the week. Independent from the day, on purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WeeklyRollover {
    pub mode: RolloverMode,
    pub at: TurnOffset,
    pub starts_on: WeekStart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rollover {
    pub daily: DailyRollover,
    pub weekly: WeeklyRollover,
}

/// A notebook's config file, in memory.
#[derive(Debug, Clone)]
pub struct Config {
    schema_version: u64,
    pub rollover: Rollover,
    /// The document exactly as it was read, so keys this build does not know
    /// about are written back instead of being silently dropped. This is what
    /// protects a notebook opened by two different app versions.
    raw: Map<String, Value>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            rollover: Rollover::default(),
            raw: Map::new(),
        }
    }
}

impl Config {
    pub fn schema_version(&self) -> u64 {
        self.schema_version
    }

    /// True when the file came from a newer app than this one.
    ///
    /// Spec 3.4: open read-only rather than risk corrupting a file written by
    /// a version that knows fields we do not.
    pub fn is_read_only(&self) -> bool {
        self.schema_version > SUPPORTED_SCHEMA_VERSION
    }

    /// Reads the config. A missing or unreadable file yields the defaults —
    /// same treatment `Inbox.md` gets, and for the same reason: a broken
    /// preference file must never stop someone from opening their notebook.
    pub fn load(path: impl AsRef<Path>) -> Self {
        let text = match std::fs::read_to_string(path.as_ref()) {
            Ok(text) => text,
            Err(_) => return Self::default(),
        };
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Self {
        let Ok(Value::Object(raw)) = serde_json::from_str::<Value>(text) else {
            return Self::default();
        };

        let schema_version = raw
            .get("schemaVersion")
            .and_then(Value::as_u64)
            .unwrap_or(SUPPORTED_SCHEMA_VERSION);

        let rollover = raw
            .get("rollover")
            .and_then(Value::as_object)
            .map(parse_rollover)
            .unwrap_or_default();

        Self {
            schema_version,
            rollover,
            raw,
        }
    }

    /// Renders the document: the file as it was read, with the keys this
    /// build owns written over it.
    pub fn render(&self) -> String {
        let mut doc = Value::Object(self.raw.clone());
        merge(
            &mut doc,
            Value::Object(Map::from_iter([
                (
                    "schemaVersion".to_string(),
                    Value::from(self.schema_version),
                ),
                ("rollover".to_string(), render_rollover(&self.rollover)),
            ])),
        );

        let mut text = serde_json::to_string_pretty(&doc)
            .unwrap_or_else(|_| "{\n  \"schemaVersion\": 1\n}".to_string());
        text.push('\n');
        text
    }

    /// Writes the config atomically. Refuses when the notebook is read-only.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if self.is_read_only() {
            return Err(Error::ReadOnlyNotebook {
                found: self.schema_version,
                supported: SUPPORTED_SCHEMA_VERSION,
            });
        }
        write_atomically(path, self.render().as_bytes())
    }
}

fn parse_rollover(block: &Map<String, Value>) -> Rollover {
    let daily = block.get("daily").and_then(Value::as_object);
    let weekly = block.get("weekly").and_then(Value::as_object);

    Rollover {
        daily: DailyRollover {
            mode: read_mode(daily),
            at: read_at(daily),
        },
        weekly: WeeklyRollover {
            mode: read_mode(weekly),
            at: read_at(weekly),
            starts_on: weekly
                .and_then(|w| w.get("startsOn"))
                .and_then(Value::as_str)
                .map(WeekStart::parse_or_default)
                .unwrap_or_default(),
        },
    }
}

fn read_mode(block: Option<&Map<String, Value>>) -> RolloverMode {
    block
        .and_then(|b| b.get("mode"))
        .and_then(Value::as_str)
        .map(RolloverMode::parse_or_default)
        .unwrap_or_default()
}

fn read_at(block: Option<&Map<String, Value>>) -> TurnOffset {
    block
        .and_then(|b| b.get("at"))
        .and_then(Value::as_str)
        .map(TurnOffset::parse_or_default)
        .unwrap_or_default()
}

fn render_rollover(rollover: &Rollover) -> Value {
    let daily = Map::from_iter([
        ("mode".to_string(), Value::from(rollover.daily.mode.render())),
        ("at".to_string(), Value::from(rollover.daily.at.render())),
    ]);
    let weekly = Map::from_iter([
        (
            "mode".to_string(),
            Value::from(rollover.weekly.mode.render()),
        ),
        ("at".to_string(), Value::from(rollover.weekly.at.render())),
        (
            "startsOn".to_string(),
            Value::from(rollover.weekly.starts_on.render()),
        ),
    ]);

    Value::Object(Map::from_iter([
        ("daily".to_string(), Value::Object(daily)),
        ("weekly".to_string(), Value::Object(weekly)),
    ]))
}

/// Deep merge, so writing `rollover.daily.mode` does not wipe an unknown
/// sibling key sitting next to it.
fn merge(target: &mut Value, patch: Value) {
    match (target, patch) {
        (Value::Object(target), Value::Object(patch)) => {
            for (key, value) in patch {
                match target.get_mut(&key) {
                    Some(existing) => merge(existing, value),
                    None => {
                        target.insert(key, value);
                    }
                }
            }
        }
        (target, patch) => *target = patch,
    }
}

/// Same tmp-then-rename dance as the task lists: sync tools may read the file
/// at any moment, and a half-written config is a broken notebook.
pub(crate) fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ctx(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes).ctx(&tmp)?;
    std::fs::rename(&tmp, path).ctx(path)?;
    Ok(())
}

/// Sorted view of a JSON object, for stable assertions in tests.
#[cfg(test)]
fn keys_of(text: &str) -> std::collections::BTreeMap<String, Value> {
    let Ok(Value::Object(map)) = serde_json::from_str::<Value>(text) else {
        panic!("not a json object: {text}");
    };
    map.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_spec() {
        let config = Config::default();
        assert_eq!(config.schema_version(), 1);
        assert_eq!(config.rollover.daily.mode, RolloverMode::Reset);
        assert_eq!(config.rollover.daily.at, TurnOffset::MIDNIGHT);
        assert_eq!(config.rollover.weekly.mode, RolloverMode::Reset);
        assert_eq!(config.rollover.weekly.starts_on, WeekStart::Monday);
        assert!(!config.is_read_only());
    }

    #[test]
    fn reads_the_documented_example() {
        let config = Config::parse(
            r#"{
              "schemaVersion": 1,
              "rollover": {
                "daily":  { "mode": "carry", "at": "-02:00" },
                "weekly": { "mode": "reset", "at": "02:00", "startsOn": "sunday" }
              }
            }"#,
        );

        assert_eq!(config.rollover.daily.mode, RolloverMode::Carry);
        assert_eq!(config.rollover.daily.at, TurnOffset::from_minutes(-120));
        assert_eq!(config.rollover.weekly.at, TurnOffset::from_minutes(120));
        assert_eq!(config.rollover.weekly.starts_on, WeekStart::Sunday);
    }

    #[test]
    fn missing_keys_take_the_defaults() {
        let config = Config::parse(r#"{ "schemaVersion": 1 }"#);
        assert_eq!(config.rollover, Rollover::default());
    }

    #[test]
    fn malformed_values_take_the_defaults_without_erroring() {
        let config = Config::parse(
            r#"{
              "schemaVersion": 1,
              "rollover": {
                "daily": { "mode": "banana", "at": "25:99" },
                "weekly": { "startsOn": 42 }
              }
            }"#,
        );
        assert_eq!(config.rollover, Rollover::default());
    }

    #[test]
    fn garbage_file_falls_back_to_defaults() {
        for text in ["", "not json", "[]", "null", "{"] {
            let config = Config::parse(text);
            assert_eq!(config.rollover, Rollover::default(), "{text:?}");
        }
    }

    #[test]
    fn unknown_top_level_keys_survive_a_rewrite() {
        // The scenario this protects: the notebook is synced between two app
        // versions, and the older one must not delete the newer one's data.
        let config = Config::parse(
            r#"{ "schemaVersion": 1, "futureFeature": { "deep": [1, 2] } }"#,
        );
        let written = keys_of(&config.render());

        assert_eq!(
            written.get("futureFeature").unwrap(),
            &serde_json::json!({ "deep": [1, 2] })
        );
    }

    #[test]
    fn unknown_keys_nested_inside_rollover_also_survive() {
        let config = Config::parse(
            r#"{
              "schemaVersion": 1,
              "rollover": {
                "daily": { "mode": "carry", "unknownKnob": true },
                "monthly": { "mode": "reset" }
              }
            }"#,
        );
        let written = keys_of(&config.render());
        let rollover = written.get("rollover").unwrap();

        assert_eq!(rollover["daily"]["unknownKnob"], serde_json::json!(true));
        assert_eq!(rollover["daily"]["mode"], serde_json::json!("carry"));
        assert_eq!(rollover["monthly"]["mode"], serde_json::json!("reset"));
    }

    #[test]
    fn render_round_trips() {
        let mut config = Config::default();
        config.rollover.daily.mode = RolloverMode::Carry;
        config.rollover.daily.at = TurnOffset::from_minutes(-120);
        config.rollover.weekly.starts_on = WeekStart::Sunday;

        let reparsed = Config::parse(&config.render());
        assert_eq!(reparsed.rollover, config.rollover);
        assert_eq!(reparsed.schema_version(), config.schema_version());
    }

    #[test]
    fn a_newer_schema_version_opens_read_only() {
        let config = Config::parse(r#"{ "schemaVersion": 99 }"#);
        assert!(config.is_read_only());

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let err = config.save(&path).unwrap_err();

        assert!(matches!(err, Error::ReadOnlyNotebook { found: 99, .. }));
        assert!(!path.exists(), "read-only config must not be written");
    }

    #[test]
    fn saves_and_loads_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("config.json");

        let mut config = Config::default();
        config.rollover.weekly.at = TurnOffset::from_minutes(90);
        config.save(&path).unwrap();

        let loaded = Config::load(&path);
        assert_eq!(loaded.rollover.weekly.at, TurnOffset::from_minutes(90));
    }

    #[test]
    fn a_missing_file_loads_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::load(dir.path().join("absent.json"));
        assert_eq!(config.rollover, Rollover::default());
    }
}
