//! Workspaces: the first-level folders of the notebook, and the
//! `.workspace.json` that says what lives inside each one.
//!
//! Model (spec 3.5): **notebook → workspace → widget → file**. A first-level
//! folder *with* a `.workspace.json` is a workspace; every other folder is
//! ignored — a stray folder dropped into the notebook must never turn into
//! interface on its own. Each widget owns a folder (or none, for pure views)
//! and has a type; the type comes from the config, never from the folder
//! name, so two task widgets can be called `Backlog/` and `Bugs/`.
//!
//! The config file follows the same covenant as `.memo/config.json`:
//!
//! - a missing or malformed value falls back to a default, never an error;
//! - an **unknown key survives the rewrite** — including unknown keys inside
//!   a widget entry, and entire widgets of unknown type. A template written
//!   for a future version must open as "not supported yet", never be
//!   destroyed;
//! - a `schemaVersion` above what this build knows opens the workspace
//!   read-only, and saving is refused.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::error::{Error, Result};

/// The marker file that makes a folder a workspace.
pub const WORKSPACE_CONFIG_FILE: &str = ".workspace.json";

/// Schema version this build understands.
pub const SUPPORTED_WORKSPACE_SCHEMA: u64 = 1;

/// Widget types this build ships. Anything else is *kept and shown as
/// unsupported*, never dropped — see [`WidgetSpec::is_known`].
pub const KNOWN_WIDGET_KINDS: [&str; 2] = ["tasks", "notes"];

/// One widget entry of a workspace config.
#[derive(Debug, Clone, PartialEq)]
pub struct WidgetSpec {
    /// The widget type (`tasks`, `notes`, or something this build has never
    /// heard of). Empty when the entry has no usable `type` — still kept, so
    /// nothing the user wrote is lost.
    pub kind: String,
    /// Folder of the widget, relative to the workspace root. `"."` means the
    /// workspace root itself; `None` means a pure view with no files of its
    /// own (the Home widgets).
    pub folder: Option<String>,
    /// Options the widget understands. Opaque to the core.
    pub options: Value,
    /// The entry exactly as read, so unknown keys survive the rewrite.
    raw: Map<String, Value>,
}

impl WidgetSpec {
    /// Builds a spec for a widget the app itself creates.
    pub fn new(kind: impl Into<String>, folder: Option<&str>) -> Self {
        Self {
            kind: kind.into(),
            folder: folder.map(str::to_string),
            options: Value::Null,
            raw: Map::new(),
        }
    }

    /// Whether this build knows how to render the widget. An unknown one is
    /// shown as an "unsupported" card with its folder left untouched.
    pub fn is_known(&self) -> bool {
        KNOWN_WIDGET_KINDS.contains(&self.kind.as_str())
    }

    fn parse(value: &Value) -> Self {
        let raw = value.as_object().cloned().unwrap_or_default();
        Self {
            kind: raw
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            folder: raw.get("folder").and_then(Value::as_str).map(str::to_string),
            options: raw.get("options").cloned().unwrap_or(Value::Null),
            raw,
        }
    }

    /// The entry as written back: the original object with the fields this
    /// build owns written over it.
    fn render(&self) -> Value {
        let mut out = self.raw.clone();
        if !self.kind.is_empty() {
            out.insert("type".into(), Value::from(self.kind.clone()));
        }
        if let Some(folder) = &self.folder {
            out.insert("folder".into(), Value::from(folder.clone()));
        }
        if !self.options.is_null() {
            out.insert("options".into(), self.options.clone());
        }
        Value::Object(out)
    }
}

/// A workspace's `.workspace.json`, in memory.
#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    schema_version: u64,
    /// Display name. Falls back to the folder name when absent.
    pub name: Option<String>,
    pub widgets: Vec<WidgetSpec>,
    /// The document as read, for the unknown-key promise.
    raw: Map<String, Value>,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            schema_version: SUPPORTED_WORKSPACE_SCHEMA,
            name: None,
            widgets: Vec::new(),
            raw: Map::new(),
        }
    }
}

impl WorkspaceConfig {
    pub fn schema_version(&self) -> u64 {
        self.schema_version
    }

    /// True when the file came from a newer app than this one. Same rule as
    /// the notebook config: read, never rewrite.
    pub fn is_read_only(&self) -> bool {
        self.schema_version > SUPPORTED_WORKSPACE_SCHEMA
    }

    /// Reads a config file. Missing or unreadable yields the defaults.
    pub fn load(path: impl AsRef<Path>) -> Self {
        match std::fs::read_to_string(path.as_ref()) {
            Ok(text) => Self::parse(&text),
            Err(_) => Self::default(),
        }
    }

    pub fn parse(text: &str) -> Self {
        let Ok(Value::Object(raw)) = serde_json::from_str::<Value>(text) else {
            return Self::default();
        };

        Self {
            schema_version: raw
                .get("schemaVersion")
                .and_then(Value::as_u64)
                .unwrap_or(SUPPORTED_WORKSPACE_SCHEMA),
            name: raw.get("name").and_then(Value::as_str).map(str::to_string),
            widgets: raw
                .get("widgets")
                .and_then(Value::as_array)
                .map(|entries| entries.iter().map(WidgetSpec::parse).collect())
                .unwrap_or_default(),
            raw,
        }
    }

    /// Renders the document: the file as read, with the owned keys written
    /// over it — same deep merge the notebook config uses.
    pub fn render(&self) -> String {
        let mut doc = Value::Object(self.raw.clone());

        let mut owned = Map::from_iter([(
            "schemaVersion".to_string(),
            Value::from(self.schema_version),
        )]);
        if let Some(name) = &self.name {
            owned.insert("name".into(), Value::from(name.clone()));
        }
        crate::config::merge(&mut doc, Value::Object(owned));

        // Widgets are replaced wholesale — order and membership are exactly
        // what this struct says — but each entry re-renders over its own raw
        // object, so unknown keys inside an entry still survive.
        if let Value::Object(doc) = &mut doc {
            doc.insert(
                "widgets".into(),
                Value::Array(self.widgets.iter().map(WidgetSpec::render).collect()),
            );
        }

        let mut text = serde_json::to_string_pretty(&doc)
            .unwrap_or_else(|_| "{\n  \"schemaVersion\": 1\n}".to_string());
        text.push('\n');
        text
    }

    /// Writes the config atomically. Refuses when it came from a newer app.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        if self.is_read_only() {
            return Err(Error::ReadOnlyNotebook {
                found: self.schema_version,
                supported: SUPPORTED_WORKSPACE_SCHEMA,
            });
        }
        crate::fsio::write_atomically(path.as_ref(), self.render().as_bytes())
    }
}

/// An open workspace: a first-level folder plus its config.
#[derive(Debug, Clone)]
pub struct Workspace {
    root: PathBuf,
    folder_name: String,
    pub config: WorkspaceConfig,
}

impl Workspace {
    /// True when the folder carries the marker file.
    pub fn is_workspace(path: impl AsRef<Path>) -> bool {
        path.as_ref().join(WORKSPACE_CONFIG_FILE).is_file()
    }

    /// Opens the workspace living in `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let root = path.as_ref().to_path_buf();
        if !Self::is_workspace(&root) {
            return Err(Error::NotAWorkspace(root));
        }
        let folder_name = root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let config = WorkspaceConfig::load(root.join(WORKSPACE_CONFIG_FILE));
        Ok(Self {
            root,
            folder_name,
            config,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The folder name, which is how states and origins will address the
    /// workspace — renaming the folder is renaming the workspace.
    pub fn folder_name(&self) -> &str {
        &self.folder_name
    }

    /// What the UI shows: the configured name, or the folder's.
    pub fn display_name(&self) -> &str {
        self.config
            .name
            .as_deref()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(&self.folder_name)
    }

    pub fn config_path(&self) -> PathBuf {
        self.root.join(WORKSPACE_CONFIG_FILE)
    }

    /// Resolves a widget's folder inside this workspace.
    ///
    /// `None` for a pure view. `"."` is the workspace root. Anything else is
    /// a relative folder, validated against escaping the workspace — the
    /// config file is user input, same as a list name.
    pub fn widget_dir(&self, spec: &WidgetSpec) -> Result<Option<PathBuf>> {
        let Some(folder) = spec.folder.as_deref() else {
            return Ok(None);
        };
        if folder == "." {
            return Ok(Some(self.root.clone()));
        }

        crate::relpath::safe_join(&self.root, folder)
            .map(Some)
            .ok_or_else(|| Error::InvalidWidgetFolder(folder.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_documented_config_parses() {
        let config = WorkspaceConfig::parse(
            r#"{
              "schemaVersion": 1,
              "name": "Project A",
              "widgets": [
                { "type": "tasks", "folder": "Backlog", "options": { "showCompleted": false } },
                { "type": "notes", "folder": "Notas" }
              ]
            }"#,
        );

        assert_eq!(config.name.as_deref(), Some("Project A"));
        assert_eq!(config.widgets.len(), 2);
        assert_eq!(config.widgets[0].kind, "tasks");
        assert_eq!(config.widgets[0].folder.as_deref(), Some("Backlog"));
        assert_eq!(
            config.widgets[0].options["showCompleted"],
            serde_json::json!(false)
        );
        assert!(config.widgets[0].is_known());
    }

    #[test]
    fn an_unknown_widget_type_is_kept_not_dropped() {
        // The most important promise of spec 3.5: a template from a future
        // version renders as "unsupported", and nothing the user has is lost.
        let config = WorkspaceConfig::parse(
            r#"{ "schemaVersion": 1, "widgets": [
                { "type": "kanban", "folder": "Board", "columns": ["todo", "done"] }
            ] }"#,
        );

        let widget = &config.widgets[0];
        assert!(!widget.is_known());
        assert_eq!(widget.kind, "kanban");

        // And the rewrite keeps the key this build has never heard of.
        let rendered = config.render();
        assert!(rendered.contains("columns"), "{rendered}");
        assert!(rendered.contains("kanban"));
    }

    #[test]
    fn unknown_top_level_keys_survive_the_rewrite() {
        let config = WorkspaceConfig::parse(
            r#"{ "schemaVersion": 1, "futureFeature": { "deep": [1] }, "widgets": [] }"#,
        );
        let reparsed = WorkspaceConfig::parse(&config.render());
        assert_eq!(reparsed.raw["futureFeature"], serde_json::json!({ "deep": [1] }));
    }

    #[test]
    fn garbage_or_missing_falls_back_to_defaults() {
        for text in ["", "not json", "[]", "null"] {
            let config = WorkspaceConfig::parse(text);
            assert_eq!(config.schema_version(), SUPPORTED_WORKSPACE_SCHEMA);
            assert!(config.widgets.is_empty(), "{text:?}");
        }
    }

    #[test]
    fn a_newer_schema_opens_read_only_and_refuses_to_save() {
        let config = WorkspaceConfig::parse(r#"{ "schemaVersion": 99 }"#);
        assert!(config.is_read_only());

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(WORKSPACE_CONFIG_FILE);
        assert!(config.save(&path).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn a_widget_entry_without_a_type_is_kept_as_unknown() {
        let config =
            WorkspaceConfig::parse(r#"{ "schemaVersion": 1, "widgets": [{ "folder": "X" }] }"#);
        assert_eq!(config.widgets.len(), 1);
        assert!(!config.widgets[0].is_known());
        assert_eq!(config.widgets[0].kind, "");
    }

    fn workspace_at(dir: &Path, config: &str) -> Workspace {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(WORKSPACE_CONFIG_FILE), config).unwrap();
        Workspace::open(dir).unwrap()
    }

    #[test]
    fn the_display_name_falls_back_to_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let ws = workspace_at(&dir.path().join("Trabalho"), r#"{ "schemaVersion": 1 }"#);
        assert_eq!(ws.display_name(), "Trabalho");
        assert_eq!(ws.folder_name(), "Trabalho");

        let named = workspace_at(
            &dir.path().join("pasta-feia"),
            r#"{ "schemaVersion": 1, "name": "Project A" }"#,
        );
        assert_eq!(named.display_name(), "Project A");
        assert_eq!(named.folder_name(), "pasta-feia");
    }

    #[test]
    fn a_folder_without_the_marker_is_not_a_workspace() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            Workspace::open(dir.path()),
            Err(Error::NotAWorkspace(_))
        ));
    }

    #[test]
    fn widget_dirs_resolve_inside_the_workspace_only() {
        let dir = tempfile::tempdir().unwrap();
        let ws = workspace_at(&dir.path().join("W"), r#"{ "schemaVersion": 1 }"#);

        // `.` is the workspace root — how the fixed workspaces work.
        let root = WidgetSpec::new("tasks", Some("."));
        assert_eq!(ws.widget_dir(&root).unwrap().unwrap(), ws.root());

        // A nested folder is allowed; the widget owns its subtree.
        let nested = WidgetSpec::new("notes", Some("Notas/Arquivo"));
        assert_eq!(
            ws.widget_dir(&nested).unwrap().unwrap(),
            ws.root().join("Notas/Arquivo")
        );

        // No folder at all: a pure view (the Home widgets).
        let view = WidgetSpec::new("today", None);
        assert_eq!(ws.widget_dir(&view).unwrap(), None);

        // Escapes and tricks are refused — the config file is user input.
        for bad in ["..", "a/../b", "/etc", "a\\b", ".hidden", "a/.b", " ", "a//b"] {
            let spec = WidgetSpec::new("tasks", Some(bad));
            assert!(
                matches!(ws.widget_dir(&spec), Err(Error::InvalidWidgetFolder(_))),
                "{bad:?} should be refused"
            );
        }
    }
}
