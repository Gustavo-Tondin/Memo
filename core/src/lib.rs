//! Memo core — all business logic lives here.
//!
//! Hard rule: this crate must NEVER depend on Tauri. If a function only works
//! by calling into Tauri, it belongs in `src-tauri` instead. Keeping the rule
//! is what would let a different frontend reuse all of this untouched.
//!
//! Reading and writing the notebook is done. Day/week rules come next.

pub mod error;
pub mod id;
pub mod list;
pub mod notebook;
pub mod task;

pub use error::{Error, Result};
pub use list::{Line, TaskList};
pub use notebook::{Notebook, OriginAction};
pub use task::Task;

/// Name of the hidden config directory inside a notebook (the "caderno").
/// Equivalent to Obsidian's `.obsidian`.
pub const NOTEBOOK_CONFIG_DIR: &str = ".memo";

/// Directory holding the task lists, inside a notebook.
pub const TASKS_DIR: &str = "Tarefas";

/// Directory holding notes, inside a notebook. Product phase 2.
pub const NOTES_DIR: &str = "Notas";

/// Default list, recreated whenever the notebook is opened.
pub const INBOX_LIST: &str = "Inbox";

/// List holding completed tasks, recreated whenever the notebook is opened.
pub const COMPLETED_LIST: &str = "Completas";

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
        // renaming any of these breaks every existing notebook.
        assert_eq!(NOTEBOOK_CONFIG_DIR, ".memo");
        assert_eq!(TASKS_DIR, "Tarefas");
        assert_eq!(NOTES_DIR, "Notas");
        assert_eq!(INBOX_LIST, "Inbox");
        assert_eq!(COMPLETED_LIST, "Completas");
    }

    #[test]
    fn version_is_not_empty() {
        assert!(!version().is_empty());
    }
}
