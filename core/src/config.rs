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
use crate::error::{Error, Result};

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

/// How a date is shown. The file always stores ISO; this is display only.
///
/// A closed set rather than a free pattern, for the same reason the repeat
/// field is a select: a value the app cannot parse would have to fall back
/// silently, and a date shown wrong is worse than a date shown plainly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DateFormat {
    /// `25-07-2026`
    #[default]
    DayMonthYear,
    /// `25/07/2026`
    DayMonthYearSlash,
    /// `07/25/2026`
    MonthDayYear,
    /// `2026-07-25` — the same form the file uses.
    Iso,
}

impl DateFormat {
    pub fn parse_or_default(text: &str) -> Self {
        match text.trim().to_ascii_lowercase().as_str() {
            "dd/mm/yyyy" => Self::DayMonthYearSlash,
            "mm/dd/yyyy" => Self::MonthDayYear,
            "yyyy-mm-dd" => Self::Iso,
            _ => Self::DayMonthYear,
        }
    }

    pub fn render(self) -> &'static str {
        match self {
            Self::DayMonthYear => "dd-mm-yyyy",
            Self::DayMonthYearSlash => "dd/mm/yyyy",
            Self::MonthDayYear => "mm/dd/yyyy",
            Self::Iso => "yyyy-mm-dd",
        }
    }

    /// Formats a date for display.
    pub fn format(self, date: chrono::NaiveDate) -> String {
        use chrono::Datelike;
        let (d, m, y) = (date.day(), date.month(), date.year());
        match self {
            Self::DayMonthYear => format!("{d:02}-{m:02}-{y}"),
            Self::DayMonthYearSlash => format!("{d:02}/{m:02}/{y}"),
            Self::MonthDayYear => format!("{m:02}/{d:02}/{y}"),
            Self::Iso => format!("{y}-{m:02}-{d:02}"),
        }
    }
}

/// A notebook's config file, in memory.
#[derive(Debug, Clone)]
pub struct Config {
    schema_version: u64,
    pub rollover: Rollover,
    /// Reopen on the screen the user left, instead of always landing on Today.
    ///
    /// Off by default: landing somewhere temporally relevant is the more
    /// predictable behaviour, and this is the kind of thing people want only
    /// once they have a habit. The *preference* lives here so it applies on
    /// every machine; the screen it points at is machine-specific and lives in
    /// the OS config folder.
    pub restore_last_screen: bool,
    /// Show how many open tasks each list has, in the navigation.
    pub show_list_counts: bool,
    /// Treat a task due today or overdue as urgent, without being told.
    ///
    /// On by default, but switchable: some people find an interface that
    /// paints deadlines red on its own more stressful than useful. The
    /// `#urgent` tag written by hand always counts, either way.
    pub auto_urgent_by_date: bool,
    /// How dates are shown. The file always stores ISO.
    pub date_display_format: DateFormat,
    /// Close the task panel when clicking outside it.
    ///
    /// Off by default, and that default is a decision: it shipped on, fired
    /// too easily, and losing a half-typed task cost more than the shortcut
    /// was worth (2026-07-21). Kept as an option because the gesture is
    /// muscle memory for some people.
    pub close_inspector_on_click_away: bool,
    /// Where the Home's quick capture writes, relative to the notes widget.
    pub quick_note_folder: String,
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
            restore_last_screen: false,
            show_list_counts: true,
            auto_urgent_by_date: true,
            date_display_format: DateFormat::default(),
            close_inspector_on_click_away: false,
            quick_note_folder: crate::notefolder::NOTES_INBOX.to_string(),
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

        let defaults = Self::default();
        Self {
            schema_version,
            rollover,
            restore_last_screen: read_bool(
                &raw,
                "restoreLastScreen",
                defaults.restore_last_screen,
            ),
            show_list_counts: read_bool(&raw, "showListCounts", defaults.show_list_counts),
            auto_urgent_by_date: read_bool(
                &raw,
                "autoUrgentByDate",
                defaults.auto_urgent_by_date,
            ),
            date_display_format: raw
                .get("dateDisplayFormat")
                .and_then(Value::as_str)
                .map(DateFormat::parse_or_default)
                .unwrap_or_default(),
            close_inspector_on_click_away: read_bool(
                &raw,
                "closeInspectorOnClickAway",
                defaults.close_inspector_on_click_away,
            ),
            quick_note_folder: raw
                .get("quickNoteFolder")
                .and_then(Value::as_str)
                .filter(|folder| !folder.trim().is_empty())
                .unwrap_or(&defaults.quick_note_folder)
                .to_string(),
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
                (
                    "restoreLastScreen".to_string(),
                    Value::from(self.restore_last_screen),
                ),
                (
                    "showListCounts".to_string(),
                    Value::from(self.show_list_counts),
                ),
                (
                    "autoUrgentByDate".to_string(),
                    Value::from(self.auto_urgent_by_date),
                ),
                (
                    "dateDisplayFormat".to_string(),
                    Value::from(self.date_display_format.render()),
                ),
                (
                    "closeInspectorOnClickAway".to_string(),
                    Value::from(self.close_inspector_on_click_away),
                ),
                (
                    "quickNoteFolder".to_string(),
                    Value::from(self.quick_note_folder.clone()),
                ),
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

/// Reads a boolean, falling back to the default when absent or the wrong type.
/// A malformed preference never blocks the notebook from opening (spec 3.4).
fn read_bool(raw: &Map<String, Value>, key: &str, default: bool) -> bool {
    raw.get(key).and_then(Value::as_bool).unwrap_or(default)
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
///
/// Shared with the workspace config, which makes the same promise: a key
/// this build does not know about survives the rewrite.
pub(crate) fn merge(target: &mut Value, patch: Value) {
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

/// The atomic write moved to [`crate::fsio`], where every kind of file shares
/// it. Kept as a thin alias so existing callers read naturally.
pub(crate) use crate::fsio::write_atomically;

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
    fn the_new_toggles_round_trip_and_tolerate_garbage() {
        let mut config = Config::default();
        assert!(!config.restore_last_screen, "off by default");
        assert!(config.show_list_counts, "on by default");

        config.restore_last_screen = true;
        config.show_list_counts = false;
        let reparsed = Config::parse(&config.render());
        assert!(reparsed.restore_last_screen);
        assert!(!reparsed.show_list_counts);

        // Wrong type falls back to the default instead of failing to open.
        let broken = Config::parse(
            r#"{ "schemaVersion": 1, "restoreLastScreen": "yes", "showListCounts": 3 }"#,
        );
        assert!(!broken.restore_last_screen);
        assert!(broken.show_list_counts);
    }

    #[test]
    fn defaults_match_the_spec() {
        let config = Config::default();
        assert_eq!(config.schema_version(), 1);
        assert!(!config.restore_last_screen);
        assert!(config.show_list_counts);
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
    fn the_phase_nine_keys_round_trip_and_tolerate_garbage() {
        let mut config = Config::default();
        assert_eq!(config.date_display_format, DateFormat::DayMonthYear);
        assert!(!config.close_inspector_on_click_away, "off by default");
        assert_eq!(config.quick_note_folder, "Inbox");

        config.date_display_format = DateFormat::Iso;
        config.close_inspector_on_click_away = true;
        config.quick_note_folder = "Clientes".into();

        let reparsed = Config::parse(&config.render());
        assert_eq!(reparsed.date_display_format, DateFormat::Iso);
        assert!(reparsed.close_inspector_on_click_away);
        assert_eq!(reparsed.quick_note_folder, "Clientes");

        // A pattern the app cannot render falls back rather than showing a
        // date wrong, and an empty folder is not a folder.
        let broken = Config::parse(
            r#"{ "schemaVersion": 1, "dateDisplayFormat": "banana",
                 "quickNoteFolder": "  ", "closeInspectorOnClickAway": 7 }"#,
        );
        assert_eq!(broken.date_display_format, DateFormat::DayMonthYear);
        assert_eq!(broken.quick_note_folder, "Inbox");
        assert!(!broken.close_inspector_on_click_away);
    }

    #[test]
    fn dates_render_in_every_offered_shape() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 7, 5).unwrap();
        assert_eq!(DateFormat::DayMonthYear.format(date), "05-07-2026");
        assert_eq!(DateFormat::DayMonthYearSlash.format(date), "05/07/2026");
        assert_eq!(DateFormat::MonthDayYear.format(date), "07/05/2026");
        assert_eq!(DateFormat::Iso.format(date), "2026-07-05");

        // Every offered value survives the config round trip.
        for shape in [
            DateFormat::DayMonthYear,
            DateFormat::DayMonthYearSlash,
            DateFormat::MonthDayYear,
            DateFormat::Iso,
        ] {
            assert_eq!(DateFormat::parse_or_default(shape.render()), shape);
        }
    }

    #[test]
    fn a_missing_file_loads_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::load(dir.path().join("absent.json"));
        assert_eq!(config.rollover, Rollover::default());
    }
}
