//! A task list file (`Tarefas/Inbox.md` and friends).
//!
//! A list is read as a sequence of lines, not as a bag of tasks: anything that
//! is not a task — headings, notes, blank lines — is kept verbatim and written
//! back untouched. The file belongs to the user, and the app is only one of
//! the tools that edit it.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::error::{Error, IoContext, Result};
use crate::id;
use crate::task::Task;

/// One line of a list file.
#[derive(Debug, Clone, PartialEq)]
pub enum Line {
    Task(Task),
    /// Any line that is not a task, preserved exactly as found.
    Raw(String),
}

/// An in-memory list file. Changes only reach the disk on [`TaskList::save`].
#[derive(Debug, Clone)]
pub struct TaskList {
    path: PathBuf,
    lines: Vec<Line>,
    /// Whether the file ended with a newline, so saving does not silently
    /// change a byte the user did not ask us to change.
    trailing_newline: bool,
}

impl TaskList {
    /// Reads a list from disk. A missing file is an empty list, not an error:
    /// lists are recreated on demand.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(Error::Io { path, source: e }),
        };
        Ok(Self::from_content(path, &content))
    }

    pub(crate) fn from_content(path: PathBuf, content: &str) -> Self {
        let trailing_newline = content.is_empty() || content.ends_with('\n');
        let lines = content
            .lines()
            .map(|line| match Task::parse(line) {
                Some(task) => Line::Task(task),
                None => Line::Raw(line.to_string()),
            })
            .collect();
        Self {
            path,
            lines,
            trailing_newline,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The list name as shown in the app: the file stem (`Compras.md` →
    /// `Compras`).
    pub fn name(&self) -> String {
        self.path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    pub fn tasks(&self) -> impl Iterator<Item = &Task> {
        self.lines.iter().filter_map(|line| match line {
            Line::Task(task) => Some(task),
            Line::Raw(_) => None,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.tasks().next().is_none()
    }

    pub fn find(&self, id: &str) -> Option<&Task> {
        self.tasks().find(|t| t.id.as_deref() == Some(id))
    }

    fn position_of(&self, id: &str) -> Option<usize> {
        self.lines.iter().position(|line| {
            matches!(line, Line::Task(task) if task.id.as_deref() == Some(id))
        })
    }

    fn taken_ids(&self) -> HashSet<String> {
        self.tasks().filter_map(|t| t.id.clone()).collect()
    }

    /// Appends a task, assigning an id if it has none. Returns the id.
    pub fn add(&mut self, mut task: Task) -> String {
        let id = match task.id.take() {
            Some(id) => id,
            None => id::generate_unique(&self.taken_ids()),
        };
        task.id = Some(id.clone());
        self.lines.push(Line::Task(task));
        id
    }

    /// Adds a task from its text alone — the common case.
    pub fn add_text(&mut self, text: impl Into<String>) -> String {
        self.add(Task::new(text))
    }

    /// Replaces the text of an existing task, leaving everything else alone.
    pub fn edit_text(&mut self, id: &str, text: impl Into<String>) -> Result<()> {
        let at = self
            .position_of(id)
            .ok_or_else(|| Error::TaskNotFound(id.to_string()))?;
        if let Line::Task(task) = &mut self.lines[at] {
            task.text = text.into();
        }
        Ok(())
    }

    /// Marks a task done or undone in place, without moving it between files.
    pub fn set_done(&mut self, id: &str, done: bool) -> Result<()> {
        let at = self
            .position_of(id)
            .ok_or_else(|| Error::TaskNotFound(id.to_string()))?;
        if let Line::Task(task) = &mut self.lines[at] {
            task.done = done;
        }
        Ok(())
    }

    /// Removes a task and hands it back, so the caller can put it elsewhere.
    pub fn remove(&mut self, id: &str) -> Result<Task> {
        let at = self
            .position_of(id)
            .ok_or_else(|| Error::TaskNotFound(id.to_string()))?;
        match self.lines.remove(at) {
            Line::Task(task) => Ok(task),
            Line::Raw(_) => unreachable!("position_of only matches task lines"),
        }
    }

    /// Repoints the `origin` of every task that came from `from` to `to`.
    /// Returns how many changed, so the caller can skip a pointless write.
    ///
    /// Used when a list is renamed: without this, undoing a completed task
    /// would try to send it back to a list that no longer exists.
    pub fn repoint_origin(&mut self, from: &str, to: &str) -> usize {
        let mut changed = 0;
        for line in &mut self.lines {
            if let Line::Task(task) = line {
                if task.origin.as_deref() == Some(from) {
                    task.origin = Some(to.to_string());
                    changed += 1;
                }
            }
        }
        changed
    }

    /// Gives an id to every task typed by hand outside the app. Returns how
    /// many were adopted, so the caller can skip saving when nothing changed.
    pub fn ensure_ids(&mut self) -> usize {
        let mut taken = self.taken_ids();
        let mut adopted = 0;
        for line in &mut self.lines {
            if let Line::Task(task) = line {
                if task.id.is_none() {
                    let new_id = id::generate_unique(&taken);
                    taken.insert(new_id.clone());
                    task.id = Some(new_id);
                    adopted += 1;
                }
            }
        }
        adopted
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        for (i, line) in self.lines.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            match line {
                Line::Task(task) => out.push_str(&task.render()),
                Line::Raw(raw) => out.push_str(raw),
            }
        }
        if self.trailing_newline && !out.is_empty() {
            out.push('\n');
        }
        out
    }

    /// Writes the list to disk atomically: a half-written list would be a
    /// corrupted notebook, and sync tools may read the file at any moment.
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).ctx(parent)?;
        }
        let tmp = self.path.with_extension("md.tmp");
        std::fs::write(&tmp, self.render()).ctx(&tmp)?;
        std::fs::rename(&tmp, &self.path).ctx(&self.path)?;
        Ok(())
    }
}
