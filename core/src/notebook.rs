//! The notebook: the folder the user picked, and everything inside it.
//!
//! Layout:
//!
//! ```text
//! MeuCaderno/
//! ├── .memo/
//! ├── Tarefas/
//! │   ├── Inbox.md
//! │   └── Completas.md
//! └── Notas/
//! ```

use std::path::{Path, PathBuf};

use crate::error::{Error, IoContext, Result};
use crate::list::TaskList;
use crate::task::Task;
use crate::{INBOX_LIST, COMPLETED_LIST, NOTEBOOK_CONFIG_DIR, NOTES_DIR, TASKS_DIR};

/// What to do with a task's `origin` field when moving it between lists.
///
/// The writer stays mechanical on purpose — deciding *when* to record an
/// origin is business logic, and it lives in the caller (phase 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OriginAction {
    /// Record the source list, so the move can be undone later.
    Record,
    /// Drop the origin — the task is going back where it came from.
    Clear,
    /// Leave whatever was there.
    Keep,
}

/// An open notebook.
#[derive(Debug, Clone)]
pub struct Notebook {
    root: PathBuf,
}

impl Notebook {
    /// True when the folder looks like a notebook, i.e. it has a `.memo/`.
    pub fn is_notebook(path: impl AsRef<Path>) -> bool {
        path.as_ref().join(NOTEBOOK_CONFIG_DIR).is_dir()
    }

    /// Opens an existing notebook, recreating the default lists if the user
    /// deleted them outside the app.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let root = path.as_ref().to_path_buf();
        if !Self::is_notebook(&root) {
            return Err(Error::NotANotebook(root));
        }
        let notebook = Self { root };
        notebook.ensure_default_lists()?;
        Ok(notebook)
    }

    /// Creates a notebook in an empty or existing folder.
    pub fn init(path: impl AsRef<Path>) -> Result<Self> {
        let root = path.as_ref().to_path_buf();
        if Self::is_notebook(&root) {
            return Err(Error::AlreadyANotebook(root));
        }

        for dir in [
            root.join(NOTEBOOK_CONFIG_DIR),
            root.join(TASKS_DIR),
            root.join(NOTES_DIR),
        ] {
            std::fs::create_dir_all(&dir).ctx(&dir)?;
        }

        // Only the container. Preferences are added by the features that own
        // them; the version field ships from day one because adding it after
        // notebooks exist on disk is far more expensive.
        let config = root.join(NOTEBOOK_CONFIG_DIR).join("config.json");
        std::fs::write(&config, "{\n  \"schemaVersion\": 1\n}\n").ctx(&config)?;

        let notebook = Self { root };
        notebook.ensure_default_lists()?;
        Ok(notebook)
    }

    /// Opens the notebook, creating it if the folder is not one yet.
    pub fn open_or_init(path: impl AsRef<Path>) -> Result<Self> {
        if Self::is_notebook(&path) {
            Self::open(path)
        } else {
            Self::init(path)
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn tasks_dir(&self) -> PathBuf {
        self.root.join(TASKS_DIR)
    }

    pub fn notes_dir(&self) -> PathBuf {
        self.root.join(NOTES_DIR)
    }

    pub fn config_dir(&self) -> PathBuf {
        self.root.join(NOTEBOOK_CONFIG_DIR)
    }

    /// Path of a list by name. Rejects anything that could escape the tasks
    /// folder — list names reach this from user input.
    pub fn list_path(&self, name: &str) -> Result<PathBuf> {
        let invalid = name.trim().is_empty()
            || name.starts_with('.')
            || name.contains(['/', '\\', '\0'])
            || name.contains("..");
        if invalid {
            return Err(Error::InvalidListName(name.to_string()));
        }
        Ok(self.tasks_dir().join(format!("{name}.md")))
    }

    /// Every list in the notebook, alphabetically.
    pub fn list_names(&self) -> Result<Vec<String>> {
        let dir = self.tasks_dir();
        let mut names = Vec::new();
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(names),
            Err(e) => return Err(Error::Io { path: dir, source: e }),
        };

        for entry in entries {
            let entry = entry.ctx(&dir)?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md") {
                if let Some(stem) = path.file_stem() {
                    names.push(stem.to_string_lossy().to_string());
                }
            }
        }
        names.sort();
        Ok(names)
    }

    pub fn open_list(&self, name: &str) -> Result<TaskList> {
        TaskList::load(self.list_path(name)?)
    }

    pub fn inbox(&self) -> Result<TaskList> {
        self.open_list(INBOX_LIST)
    }

    pub fn completed(&self) -> Result<TaskList> {
        self.open_list(COMPLETED_LIST)
    }

    /// Recreates `Inbox.md` and `Completas.md` when missing. Called on every
    /// open: the user may have deleted them, and the app must not break.
    pub fn ensure_default_lists(&self) -> Result<()> {
        let dir = self.tasks_dir();
        std::fs::create_dir_all(&dir).ctx(&dir)?;
        for name in [INBOX_LIST, COMPLETED_LIST] {
            let path = self.list_path(name)?;
            if !path.exists() {
                std::fs::write(&path, "").ctx(&path)?;
            }
        }
        Ok(())
    }

    /// Creates a new list. Fails if one with that name already exists.
    pub fn create_list(&self, name: &str) -> Result<TaskList> {
        let path = self.list_path(name)?;
        if path.exists() {
            return Err(Error::InvalidListName(format!("{name} already exists")));
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ctx(parent)?;
        }
        std::fs::write(&path, "").ctx(&path)?;
        TaskList::load(path)
    }

    /// Moves a task between lists, preserving its id.
    ///
    /// Both files are saved; the task is returned as it landed. This is the
    /// primitive behind completing and un-completing a task.
    pub fn move_task(
        &self,
        id: &str,
        from: &str,
        to: &str,
        origin: OriginAction,
    ) -> Result<Task> {
        let mut source = self.open_list(from)?;
        let mut target = self.open_list(to)?;

        let mut task = source.remove(id)?;
        match origin {
            OriginAction::Record => task.origin = Some(from.to_string()),
            OriginAction::Clear => task.origin = None,
            OriginAction::Keep => {}
        }

        let moved = task.clone();
        target.add(task);

        // Target first: a crash between the two writes duplicates the task
        // instead of losing it, and a duplicate is recoverable by hand.
        target.save()?;
        source.save()?;
        Ok(moved)
    }
}
