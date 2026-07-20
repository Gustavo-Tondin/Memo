//! Memo core — all business logic lives here.
//!
//! Hard rule: this crate must NEVER depend on Tauri. If a function only works
//! by calling into Tauri, it belongs in `src-tauri` instead. Keeping the rule
//! is what would let a different frontend reuse all of this untouched.
//!
//! Reading and writing the notebook is done. Day/week rules come next.

pub mod clock;
pub mod conflict;
pub mod config;
pub mod error;
pub mod id;
pub mod list;
pub mod notebook;
pub mod recurrence;
pub mod rollover;
pub mod state;
pub mod task;
pub mod watcher;

pub use clock::{TurnOffset, WeekStart};
pub use config::{Config, Rollover, RolloverMode};
pub use conflict::Conflict;
pub use error::{Error, Result};
pub use list::{Line, TaskList};
pub use notebook::{ListedTask, Notebook, OriginAction};
pub use state::{Period, PeriodState, StateFile, TaskRef};
pub use watcher::{Change, NotebookWatcher};
pub use task::Task;

/// Name of the hidden config directory inside a notebook.
/// Equivalent to Obsidian's `.obsidian`.
pub const NOTEBOOK_CONFIG_DIR: &str = ".memo";

/// Directory holding the task lists, inside a notebook.
pub const TASKS_DIR: &str = "Tasks";

/// Directory holding notes, inside a notebook.
pub const NOTES_DIR: &str = "Notes";

/// Default list, recreated whenever the notebook is opened.
pub const INBOX_LIST: &str = "Inbox";

/// List holding completed tasks, recreated whenever the notebook is opened.
pub const COMPLETED_LIST: &str = "Completed";

/// The names these used to have, before the app settled on English in
/// 2026-07-20. Kept so notebooks created by earlier versions can be migrated
/// instead of breaking — see [`notebook::Notebook::open`].
pub mod legacy {
    pub const TASKS_DIR: &str = "Tarefas";
    pub const NOTES_DIR: &str = "Notas";
    pub const COMPLETED_LIST: &str = "Completas";
}

/// Version of this crate, exposed so the shell can report it.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notebook_layout_constants_match_the_spec() {
        // The notebook layout is part of the file format users depend on:
        // renaming any of these breaks every existing notebook, so changing
        // this test has to be a deliberate act with a migration attached.
        // Last changed 2026-07-20, when the app settled on English names.
        assert_eq!(NOTEBOOK_CONFIG_DIR, ".memo");
        assert_eq!(TASKS_DIR, "Tasks");
        assert_eq!(NOTES_DIR, "Notes");
        assert_eq!(INBOX_LIST, "Inbox");
        assert_eq!(COMPLETED_LIST, "Completed");
    }

    #[test]
    fn legacy_names_are_the_ones_we_migrate_from() {
        // These must never change: they describe notebooks already on disk.
        assert_eq!(legacy::TASKS_DIR, "Tarefas");
        assert_eq!(legacy::NOTES_DIR, "Notas");
        assert_eq!(legacy::COMPLETED_LIST, "Completas");
    }

    #[test]
    fn version_is_not_empty() {
        assert!(!version().is_empty());
    }
}
