//! The notebook: the folder the user picked, and everything inside it.
//!
//! Layout:
//!
//! ```text
//! MyNotebook/
//! ├── .memo/
//! │   ├── config.json
//! │   ├── daily-state.json
//! │   └── weekly-state.json
//! ├── Tasks/
//! │   ├── _FORMAT.txt
//! │   ├── Inbox.md
//! │   └── Completed.md
//! └── Notes/
//! ```
//!
//! Since phase 7 every list is addressed by its **root-relative path**
//! (`Tasks/Compras.md`), never by a bare name — two folders of tasks mean two
//! lists called `Inbox`, and a name stops identifying anything. A notebook in
//! the pre-phase-7 layout is refused on open with a clear message (no
//! migrations before v1 — decision 2026-07-21).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;

use crate::clock;
use crate::conflict::Conflict;
use crate::config::{Config, RolloverMode};
use crate::error::{Error, IoContext, Result};
use crate::list::TaskList;
use crate::rollover;
use crate::state::{Period, StateFile};
use crate::task::Task;
use crate::{COMPLETED_LIST, INBOX_LIST, NOTEBOOK_CONFIG_DIR, NOTES_DIR, TASKS_DIR};

/// What to do with a task's `origin` field when moving it between lists.
///
/// The writer stays mechanical on purpose — deciding *when* to record an
/// origin is business logic, and it lives in the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OriginAction {
    /// Record the source list, so the move can be undone later.
    Record,
    /// Drop the origin — the task is going back where it came from.
    Clear,
    /// Leave whatever was there.
    Keep,
}

/// A task together with the list it lives in.
///
/// Day and Week show tasks from several lists at once, so the list's address
/// has to travel with the task — without it the UI could not tell the core
/// which file to act on. `path` is relative to the notebook root
/// (`Tasks/Compras.md`); the display name is the file stem, derived by
/// whoever shows it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ListedTask {
    pub path: String,
    pub task: Task,
}

/// Why a suggestion is where it is.
///
/// The order of the variants **is** the display order, so a group cannot be
/// reordered by accident somewhere else in the code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SuggestionGroup {
    /// Overdue, due today, or tagged `#urgent`.
    Urgent,
    /// Due in the next few days.
    Soon,
    /// Already chosen for this week (only offered to the day).
    ThisWeek,
    /// Everything else, in the order the lists have it.
    Lists,
}

/// How many days ahead still counts as "soon".
const SOON_WINDOW_DAYS: i64 = 3;

/// A task offered for a period, and the reason it is being offered.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub path: String,
    pub task: Task,
    pub group: SuggestionGroup,
}

/// A list as the navigation shows it: address plus display name.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListEntry {
    /// Root-relative address (`Tasks/Compras.md`) — what every command takes.
    pub path: String,
    /// The file stem (`Compras`) — what the user reads.
    pub name: String,
}

/// Splits a root-relative list address into folder part and list name:
/// `Tasks/Compras.md` → (`Tasks`, `Compras`).
///
/// Rejects everything that could escape the notebook — the address arrives
/// from user input and config files. Note the inversion from the old
/// name-based rule: `/` stopped being forbidden and became the separator;
/// what is forbidden now is any component that climbs (`..`), hides (leading
/// `.`) or breaks the comment format (`"`).
fn split_list_path(path: &str) -> Result<(&str, &str)> {
    let invalid = || Error::InvalidListName(path.to_string());

    let stem = path.strip_suffix(".md").ok_or_else(invalid)?;
    // A list always lives inside a workspace folder, never at the root.
    let (dir, name) = stem.rsplit_once('/').ok_or_else(invalid)?;

    let bad_component = |part: &str| part.trim().is_empty() || part.starts_with('.');
    if path.starts_with('/')
        || path.contains(['\\', '\0', '"'])
        || path.contains("..")
        || dir.split('/').any(bad_component)
        || bad_component(name)
    {
        return Err(invalid());
    }
    Ok((dir, name))
}

/// An open notebook.
#[derive(Debug, Clone)]
pub struct Notebook {
    root: PathBuf,
    config: Config,
}

impl Notebook {
    /// True when the folder looks like a notebook, i.e. it has a `.memo/`.
    pub fn is_notebook(path: impl AsRef<Path>) -> bool {
        path.as_ref().join(NOTEBOOK_CONFIG_DIR).is_dir()
    }

    /// Opens an existing notebook, recreating the default lists if the user
    /// deleted them outside the app.
    ///
    /// A notebook in the pre-phase-7 layout is **refused with a clear
    /// message**, never converted in silence — decided on 2026-07-21: there
    /// is exactly one (test) notebook in the world, and carrying migration
    /// code for a format that still changes weekly is weight without a user.
    /// From v1 on this inverts, permanently: breaking an existing notebook
    /// stops being an option and every format change ships with a migration.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let root = path.as_ref().to_path_buf();
        if !Self::is_notebook(&root) {
            return Err(Error::NotANotebook(root));
        }
        if root.join(crate::legacy::TASKS_DIR).is_dir()
            || root.join(crate::legacy::NOTES_DIR).is_dir()
        {
            return Err(Error::LegacyNotebook(root));
        }

        let config = Config::load(root.join(NOTEBOOK_CONFIG_DIR).join("config.json"));
        let notebook = Self { root, config };

        // A notebook written by a newer app is opened for reading only, so
        // nothing here may touch the disk.
        if !notebook.is_read_only() {
            notebook.ensure_fixed_workspaces()?;
            notebook.ensure_default_lists()?;
            notebook.write_format_guide()?;
        }
        Ok(notebook)
    }

    /// Creates a notebook in an empty or existing folder.
    pub fn init(path: impl AsRef<Path>) -> Result<Self> {
        let root = path.as_ref().to_path_buf();
        if Self::is_notebook(&root) {
            return Err(Error::AlreadyANotebook(root));
        }

        let dir = root.join(NOTEBOOK_CONFIG_DIR);
        std::fs::create_dir_all(&dir).ctx(&dir)?;

        let notebook = Self {
            root,
            config: Config::default(),
        };
        notebook.config.save(notebook.config_path())?;
        notebook.ensure_fixed_workspaces()?;
        notebook.ensure_default_lists()?;
        notebook.write_format_guide()?;
        Ok(notebook)
    }

    /// Recreates the three fixed workspaces — Home, Tasks, Notes — when their
    /// folder or marker is missing. Called on init and on every open, same
    /// treatment the default lists get: the user may delete things outside
    /// the app, and the app must not break.
    ///
    /// Only the **markers** are recreated; the contents of the folders are
    /// never touched. A `.workspace.json` the user edited is left exactly as
    /// it is — recreating is not rewriting.
    fn ensure_fixed_workspaces(&self) -> Result<()> {
        const FIXED: [(&str, &str); 3] = [
            (
                "Home",
                "{\n  \"schemaVersion\": 1,\n  \"name\": \"Home\",\n  \"widgets\": []\n}\n",
            ),
            (
                TASKS_DIR,
                "{\n  \"schemaVersion\": 1,\n  \"widgets\": [{ \"type\": \"tasks\", \"folder\": \".\" }]\n}\n",
            ),
            (
                NOTES_DIR,
                "{\n  \"schemaVersion\": 1,\n  \"widgets\": [{ \"type\": \"notes\", \"folder\": \".\" }]\n}\n",
            ),
        ];

        for (name, config) in FIXED {
            let dir = self.root.join(name);
            std::fs::create_dir_all(&dir).ctx(&dir)?;
            let marker = dir.join(crate::workspace::WORKSPACE_CONFIG_FILE);
            if !marker.exists() {
                crate::fsio::write_atomically(&marker, config.as_bytes())?;
            }
        }
        Ok(())
    }

    fn write_format_guide(&self) -> Result<()> {
        self.tasks_folder().write_format_guide()
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

    /// The folder of lists a `tasks` widget owns — today, always `Tasks/`.
    ///
    /// Extracted in phase 7 (step B): everything that only needs "a folder
    /// of lists" lives on [`TaskFolder`], so the second tasks widget is a
    /// different folder, not a rewrite. The notebook keeps the rules that
    /// coordinate across files — states, completion, suggestions.
    pub fn tasks_folder(&self) -> crate::folder::TaskFolder {
        crate::folder::TaskFolder::new(self.tasks_dir())
    }

    pub fn notes_dir(&self) -> PathBuf {
        self.root.join(NOTES_DIR)
    }

    pub fn config_dir(&self) -> PathBuf {
        self.root.join(NOTEBOOK_CONFIG_DIR)
    }

    pub fn config_path(&self) -> PathBuf {
        self.config_dir().join("config.json")
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// True when the notebook was written by a newer version of the app.
    pub fn is_read_only(&self) -> bool {
        self.config.is_read_only()
    }

    /// Replaces the preferences and writes them to disk.
    pub fn set_config(&mut self, config: Config) -> Result<()> {
        self.ensure_writable()?;
        config.save(self.config_path())?;
        self.config = config;
        Ok(())
    }

    /// Guard for every operation that writes. Reading a future notebook is
    /// fine; rewriting one is how data written by a newer app gets destroyed.
    fn ensure_writable(&self) -> Result<()> {
        if self.is_read_only() {
            return Err(Error::ReadOnlyNotebook {
                found: self.config.schema_version(),
                supported: crate::config::SUPPORTED_SCHEMA_VERSION,
            });
        }
        Ok(())
    }

    /// The root-relative address of the fixed workspace's inbox — where
    /// quick-captured tasks land.
    pub fn inbox_path() -> String {
        format!("{TASKS_DIR}/{INBOX_LIST}.md")
    }

    /// The address of the Completed list that serves `list_path` — the one in
    /// the **same folder** (spec 3.5: one Completed per tasks widget, so a
    /// completed task never leaves the workspace it lived in).
    pub fn completed_path_of(list_path: &str) -> Result<String> {
        let (dir, _) = split_list_path(list_path)?;
        Ok(format!("{dir}/{COMPLETED_LIST}.md"))
    }

    /// Resolves a root-relative list address (`Tasks/Compras.md`) into the
    /// folder that owns it and the list name. Every operation that receives a
    /// list goes through here — the address is user input, exactly like a
    /// list name used to be.
    fn resolve_list(&self, path: &str) -> Result<(crate::folder::TaskFolder, String)> {
        let (dir, name) = split_list_path(path)?;
        Ok((
            crate::folder::TaskFolder::new(self.root.join(dir)),
            name.to_string(),
        ))
    }

    /// Every tasks-widget folder in the notebook, with its root-relative
    /// prefix. This is the walk behind lists, counts, conflicts and
    /// suggestions — one definition of "where tasks live", not four.
    fn task_folders(&self) -> Result<Vec<(String, crate::folder::TaskFolder)>> {
        let mut folders = Vec::new();
        for workspace in self.workspaces()? {
            for spec in &workspace.config.widgets {
                if spec.kind != "tasks" {
                    continue;
                }
                let Some(dir) = workspace.widget_dir(spec)? else {
                    continue;
                };
                let prefix = dir
                    .strip_prefix(&self.root)
                    .unwrap_or(&dir)
                    .to_string_lossy()
                    .replace('\\', "/");
                folders.push((prefix, crate::folder::TaskFolder::new(dir)));
            }
        }
        Ok(folders)
    }

    /// The lists of the notebook, across every workspace's tasks widgets.
    /// Sorted by name, which is what a sidebar shows.
    pub fn lists(&self) -> Result<Vec<ListEntry>> {
        let mut entries: Vec<ListEntry> = Vec::new();
        for (prefix, folder) in self.task_folders()? {
            for name in folder.list_names()? {
                entries.push(ListEntry {
                    path: format!("{prefix}/{name}.md"),
                    name,
                });
            }
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.path.cmp(&b.path)));
        Ok(entries)
    }

    /// How many open tasks each list has, keyed by address, across every
    /// workspace's tasks widgets.
    pub fn open_task_counts(&self) -> Result<BTreeMap<String, usize>> {
        let mut counts = BTreeMap::new();
        for (prefix, folder) in self.task_folders()? {
            for (name, count) in folder.open_task_counts()? {
                counts.insert(format!("{prefix}/{name}.md"), count);
            }
        }
        Ok(counts)
    }

    /// Conflicting copies sitting in the notebook right now.
    ///
    /// Scans the config folder and every tasks-widget folder, which is where
    /// sync tools leave them. Reporting is all this does — the user decides
    /// what to keep.
    pub fn conflicts(&self) -> Result<Vec<Conflict>> {
        let mut found = Vec::new();
        let mut dirs = vec![self.config_dir()];
        for (_, folder) in self.task_folders()? {
            dirs.push(folder.dir().to_path_buf());
        }
        for dir in dirs {
            let entries = match std::fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(Error::Io { path: dir, source: e }),
            };
            for entry in entries {
                let path = entry.ctx(&dir)?.path();
                if let Some(conflict) = crate::conflict::describe(&path) {
                    found.push(conflict);
                }
            }
        }
        found.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(found)
    }

    /// Opens a list by its root-relative address (`Tasks/Compras.md`).
    pub fn open_list(&self, path: &str) -> Result<TaskList> {
        let (folder, name) = self.resolve_list(path)?;
        folder.open_list(&name)
    }

    /// Tasks of a list, ready to show.
    ///
    /// Reading does **not** hand out ids: a task only gets one when something
    /// needs to address it (see [`Notebook::ensure_task_id`]). Opening a list
    /// therefore leaves a hand-written file exactly as it was.
    ///
    /// The one thing reading does fix is a *duplicated* id, because that makes
    /// two lines indistinguishable to every later operation. That is rare, so
    /// the file is only rewritten when it actually happens.
    pub fn tasks_in(&self, path: &str) -> Result<Vec<Task>> {
        let mut tasks = self.open_list(path)?;
        if !self.is_read_only() && tasks.dedupe_ids() > 0 {
            tasks.save()?;
        }
        Ok(tasks.tasks().cloned().collect())
    }

    /// Gives the task at `position` in the list at `path` an id, and returns
    /// it.
    ///
    /// The frontend shows tasks by position; the moment the user acts on one
    /// — pulls it into a period, completes it — it needs a stable name. This
    /// is where a task earns one.
    pub fn ensure_task_id(&self, path: &str, position: usize) -> Result<String> {
        self.ensure_writable()?;
        let mut tasks = self.open_list(path)?;

        let existing = tasks
            .tasks()
            .nth(position)
            .ok_or_else(|| Error::TaskNotFound(format!("{path}[{position}]")))?
            .id
            .clone();
        if let Some(id) = existing {
            return Ok(id);
        }

        let id = tasks
            .ensure_id_at(position)
            .ok_or_else(|| Error::TaskNotFound(format!("{path}[{position}]")))?;
        tasks.save()?;
        Ok(id)
    }

    /// The fixed workspace's inbox.
    pub fn inbox(&self) -> Result<TaskList> {
        self.open_list(&Self::inbox_path())
    }

    /// Whether a list is one the app recreates on every open.
    pub fn is_default_list(name: &str) -> bool {
        name == INBOX_LIST || name == COMPLETED_LIST
    }

    /// Recreates `Inbox.md` and `Completed.md` when missing. Called on every
    /// open: the user may have deleted them, and the app must not break.
    pub fn ensure_default_lists(&self) -> Result<()> {
        self.tasks_folder().ensure_default_lists()
    }

    /// Creates a new list inside `folder` (a root-relative workspace folder,
    /// e.g. `Tasks`). Fails if one with that name already exists.
    pub fn create_list(&self, folder: &str, name: &str) -> Result<TaskList> {
        self.ensure_writable()?;
        // A list name is a leaf: a `/` here would silently create a nested
        // folder instead of a list called "sub/lista".
        if name.contains('/') {
            return Err(Error::InvalidListName(name.to_string()));
        }
        // Validate folder and name in one go by resolving the would-be path.
        let address = format!("{folder}/{name}.md");
        let (task_folder, name) = self.resolve_list(&address)?;
        let path = task_folder.list_path(&name)?;
        if path.exists() {
            return Err(Error::InvalidListName(format!("{name} already exists")));
        }
        crate::fsio::write_atomically(&path, b"")?;
        TaskList::load(path)
    }

    /// Renames a user list (addressed by path) to a new **name**, in the same
    /// folder — a rename never moves a list between workspaces. Repoints
    /// everything that referred to it: the `origin` of completed tasks in the
    /// folder's own Completed (otherwise undo would send them to a list that
    /// no longer exists) and the day/week states.
    pub fn rename_list(&self, from: &str, to_name: &str) -> Result<()> {
        self.ensure_writable()?;
        let (folder, from_name) = self.resolve_list(from)?;
        if Self::is_default_list(&from_name) {
            return Err(Error::ProtectedList(from_name));
        }
        if Self::is_default_list(to_name) {
            return Err(Error::InvalidListName(to_name.to_string()));
        }

        let source = folder.list_path(&from_name)?;
        let target = folder.list_path(to_name)?;
        if !source.exists() {
            return Err(Error::InvalidListName(format!("{from} does not exist")));
        }
        if target.exists() {
            return Err(Error::InvalidListName(format!("{to_name} already exists")));
        }

        std::fs::rename(&source, &target).ctx(&target)?;

        // Origins live in the folder's own Completed and hold bare names —
        // relative to the widget, so the folder stays portable (spec 3.5).
        let mut completed = folder.open_list(COMPLETED_LIST)?;
        if completed.repoint_origin(&from_name, to_name) > 0 {
            completed.save()?;
        }

        let (dir, _) = split_list_path(from)?;
        let to_path = format!("{dir}/{to_name}.md");
        for period in [Period::Day, Period::Week] {
            let mut state = self.open_state(period)?;
            if state.state.rename_path(from, &to_path) {
                state.save()?;
            }
        }
        Ok(())
    }

    /// Deletes a user list, moving whatever was still in it to the **same
    /// folder's** Inbox.
    ///
    /// Deleting a list is a filing decision, not a decision to throw work
    /// away — principle 2, the data is the user's. An empty list disappears
    /// silently; one with tasks leaves them in the folder's Inbox.
    pub fn delete_list(&self, path: &str) -> Result<usize> {
        self.ensure_writable()?;
        let (folder, name) = self.resolve_list(path)?;
        if Self::is_default_list(&name) {
            return Err(Error::ProtectedList(name));
        }

        let file = folder.list_path(&name)?;
        if !file.exists() {
            return Err(Error::InvalidListName(format!("{path} does not exist")));
        }

        // Every task moves, not just the ones that happen to have an id —
        // most tasks never earn one, and losing them here would be silent.
        let list = TaskList::load(&file)?;
        let rescued: Vec<Task> = list.tasks().cloned().collect();

        let mut inbox = folder.open_list(INBOX_LIST)?;
        for task in &rescued {
            inbox.add(task.clone());
        }

        // Inbox first, then the file goes away: a crash in between leaves a
        // duplicate, never a hole.
        inbox.save()?;
        std::fs::remove_file(&file).ctx(&file)?;
        let rescued = rescued.len();

        let (dir, _) = split_list_path(path)?;
        let inbox_path = format!("{dir}/{INBOX_LIST}.md");
        for period in [Period::Day, Period::Week] {
            let mut state = self.open_state(period)?;
            // References now point at the Inbox copies, which carry the same
            // ids; repointing keeps a pulled task pulled.
            if state.state.rename_path(path, &inbox_path) {
                state.save()?;
            }
        }
        Ok(rescued)
    }

    /// Moves a task between lists (addressed by path), preserving its id.
    pub fn move_task(
        &self,
        id: &str,
        from: &str,
        to: &str,
        origin: OriginAction,
    ) -> Result<Task> {
        self.transfer(id, from, to, origin, None)
    }

    /// The move primitive. `done` optionally flips the checkbox in the same
    /// write, so completing a task is one pass over each file instead of two.
    ///
    /// A recorded origin is the source's **name**, not its path: origins are
    /// only ever resolved inside the same folder (undo goes back to a sibling
    /// list), and a bare name keeps the folder portable as a template.
    fn transfer(
        &self,
        id: &str,
        from: &str,
        to: &str,
        origin: OriginAction,
        done: Option<bool>,
    ) -> Result<Task> {
        self.ensure_writable()?;
        let (_, from_name) = self.resolve_list(from)?;
        let mut source = self.open_list(from)?;
        let mut target = self.open_list(to)?;

        let mut task = source.remove(id)?;
        match origin {
            OriginAction::Record => task.origin = Some(from_name),
            OriginAction::Clear => task.origin = None,
            OriginAction::Keep => {}
        }
        if let Some(done) = done {
            task.done = done;
        }

        let moved = task.clone();
        target.add(task);

        // Target first: a crash between the two writes duplicates the task
        // instead of losing it, and a duplicate is recoverable by hand.
        target.save()?;
        source.save()?;
        Ok(moved)
    }

    // ---------------------------------------------------------------- state

    pub fn state_path(&self, period: Period) -> PathBuf {
        self.config_dir().join(period.file_name())
    }

    /// The current logical day, honouring the configured turn.
    pub fn today(&self) -> NaiveDate {
        clock::today(self.config.rollover.daily.at)
    }

    /// First day of the current logical week.
    pub fn current_week(&self) -> NaiveDate {
        clock::this_week(
            self.config.rollover.weekly.at,
            self.config.rollover.weekly.starts_on,
        )
    }

    pub fn current_period_date(&self, period: Period) -> NaiveDate {
        match period {
            Period::Day => self.today(),
            Period::Week => self.current_week(),
        }
    }

    /// When the next turn of `period` happens.
    ///
    /// The rollover must also fire while the app is *open*, not only when the
    /// notebook is reopened. The core cannot own a timer without dragging in a
    /// runtime, so it answers "when" and the app schedules the wake-up.
    pub fn next_turn_at(&self, period: Period) -> chrono::DateTime<chrono::Local> {
        // The clock module reads the instant: this used to call Local::now()
        // here, which the invariant test now flags — the configured turn only
        // stays honest while clock.rs is the single reader.
        match period {
            Period::Day => clock::next_daily_turn(self.config.rollover.daily.at),
            Period::Week => clock::next_weekly_turn(
                self.config.rollover.weekly.at,
                self.config.rollover.weekly.starts_on,
            ),
        }
    }

    /// The workspaces of this notebook: every first-level folder carrying a
    /// `.workspace.json`, alphabetically by folder name.
    ///
    /// Folders without the marker are ignored on purpose — a stray folder
    /// dropped into the notebook (downloads, an attachments dir, whatever a
    /// sync tool leaves) must never turn into interface on its own.
    pub fn workspaces(&self) -> Result<Vec<crate::workspace::Workspace>> {
        let mut found = Vec::new();
        let entries = match std::fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(found),
            Err(e) => {
                return Err(Error::Io {
                    path: self.root.clone(),
                    source: e,
                })
            }
        };

        for entry in entries {
            let path = entry.ctx(&self.root)?.path();
            let hidden = path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('.'));
            if !path.is_dir() || hidden {
                continue;
            }
            if crate::workspace::Workspace::is_workspace(&path) {
                found.push(crate::workspace::Workspace::open(path)?);
            }
        }
        found.sort_by(|a, b| a.folder_name().cmp(b.folder_name()));
        Ok(found)
    }

    /// Starts watching this notebook for changes made outside the app.
    pub fn watch(&self) -> Result<crate::watcher::NotebookWatcher> {
        crate::watcher::NotebookWatcher::start(&self.root)
    }

    fn rollover_mode(&self, period: Period) -> RolloverMode {
        match period {
            Period::Day => self.config.rollover.daily.mode,
            Period::Week => self.config.rollover.weekly.mode,
        }
    }

    /// Opens a state file with the rollover already applied.
    ///
    /// Every read goes through here, so a notebook that sat closed for a week
    /// is up to date the moment anything looks at it — the app never has to
    /// remember to roll over first.
    pub fn open_state(&self, period: Period) -> Result<StateFile> {
        let current = self.current_period_date(period);
        let mut file = StateFile::load(self.state_path(period), current);

        let rolled = rollover::apply(&mut file.state, current, self.rollover_mode(period));
        if rolled.changed() && !self.is_read_only() {
            file.save()?;
        }
        Ok(file)
    }

    /// Pulls an existing task into Today or This Week.
    pub fn pull_into(&self, period: Period, path: &str, id: &str) -> Result<bool> {
        self.ensure_writable()?;
        // Fail before writing the state if the task is not really there —
        // a reference to a missing task shows up as a ghost row in the UI.
        let source = self.open_list(path)?;
        if source.find(id).is_none() {
            return Err(Error::TaskNotFound(id.to_string()));
        }

        let mut file = self.open_state(period)?;
        if !file.state.add(path, id) {
            return Ok(false);
        }
        file.save()?;
        Ok(true)
    }

    /// Removes a task from Today or This Week. The task itself is untouched.
    pub fn remove_from(&self, period: Period, path: &str, id: &str) -> Result<bool> {
        self.ensure_writable()?;
        let mut file = self.open_state(period)?;
        if !file.state.remove(path, id) {
            return Ok(false);
        }
        file.save()?;
        Ok(true)
    }

    /// Creates a task straight from Today or This Week.
    ///
    /// The task is written to the Inbox — Day and Week never store content of
    /// their own, they only point at tasks that live in a real list (spec 3).
    pub fn add_task_in_period(&self, period: Period, text: impl Into<String>) -> Result<String> {
        self.ensure_writable()?;
        let mut inbox = self.inbox()?;

        // This one earns an id immediately: the state is about to reference
        // it, and a reference needs something stable to point at.
        let id = inbox.add_text_with_id(text);
        inbox.save()?;

        let mut file = self.open_state(period)?;
        file.state.add(Self::inbox_path(), &id);
        file.save()?;
        Ok(id)
    }

    /// The tasks actually pulled into a period, in the order they were pulled.
    ///
    /// A reference whose task no longer exists (deleted in another editor) is
    /// skipped instead of failing: the notebook is shared with other tools, so
    /// a stale reference is a normal state, not corruption.
    pub fn period_tasks(&self, period: Period) -> Result<Vec<ListedTask>> {
        let state = self.open_state(period)?.state;
        let mut out = Vec::new();

        for reference in &state.items {
            let Ok(list) = self.open_list(&reference.path) else {
                continue;
            };
            if let Some(task) = list.find(&reference.id) {
                out.push(ListedTask {
                    path: reference.path.clone(),
                    task: task.clone(),
                });
            }
        }
        Ok(out)
    }

    /// Whether a task counts as urgent right now.
    ///
    /// Two sources with equal weight (spec 3.2): the `#urgent` tag the user
    /// wrote, and a date that is today or already past. The date half can be
    /// switched off for people who do not want the interface flagging
    /// deadlines on its own.
    pub fn is_urgent(&self, task: &Task) -> bool {
        if task.is_marked_urgent() {
            return true;
        }
        if !self.config.auto_urgent_by_date {
            return false;
        }
        task.due.is_some_and(|due| due <= self.today())
    }

    /// What to offer pulling into a period, grouped and in display order.
    ///
    /// Nothing here *selects* a task — the day stays a deliberate choice.
    /// Dates only change what is offered first.
    pub fn grouped_suggestions(&self, period: Period) -> Result<Vec<Suggestion>> {
        let today = self.today();
        let soon = today + chrono::Duration::days(SOON_WINDOW_DAYS);

        let in_week: std::collections::HashSet<(String, String)> = if period == Period::Day {
            self.open_state(Period::Week)?
                .state
                .items
                .iter()
                .map(|r| (r.path.clone(), r.id.clone()))
                .collect()
        } else {
            Default::default()
        };

        let mut suggestions: Vec<Suggestion> = self
            .suggestions_for(period)?
            .into_iter()
            .map(|entry| {
                let group = if self.is_urgent(&entry.task) {
                    SuggestionGroup::Urgent
                } else if entry.task.due.is_some_and(|due| due <= soon) {
                    SuggestionGroup::Soon
                } else if entry
                    .task
                    .id
                    .as_ref()
                    .is_some_and(|id| in_week.contains(&(entry.path.clone(), id.clone())))
                {
                    SuggestionGroup::ThisWeek
                } else {
                    SuggestionGroup::Lists
                };
                Suggestion {
                    path: entry.path,
                    task: entry.task,
                    group,
                }
            })
            .collect();

        // Stable sort: inside a group the original order is kept, which is the
        // order of the lists on disk — the order the user arranged.
        suggestions.sort_by_key(|s| s.group);
        Ok(suggestions)
    }

    /// What to offer pulling into a period, in the order the UI shows it.
    ///
    /// For the day, tasks already chosen for the week come first: they are
    /// what the user decided mattered this week, so they are the best
    /// candidates for today. Everything else in the lists follows.
    ///
    /// Anything already pulled into the period is left out, and so are
    /// completed tasks and the `Completas` list itself.
    pub fn suggestions_for(&self, period: Period) -> Result<Vec<ListedTask>> {
        let pulled = self.open_state(period)?.state;
        let mut out: Vec<ListedTask> = Vec::new();

        let push = |candidate: ListedTask, out: &mut Vec<ListedTask>| {
            if candidate.task.done {
                return;
            }
            // Most tasks have no id — one is handed out only when something
            // needs to address the task. A task without an id has never been
            // pulled anywhere, so it is always still a suggestion.
            if let Some(id) = candidate.task.id.as_deref() {
                if pulled.contains(&candidate.path, id) {
                    return;
                }
                let already = out
                    .iter()
                    .any(|t| t.path == candidate.path && t.task.id.as_deref() == Some(id));
                if already {
                    return;
                }
            }
            out.push(candidate);
        };

        // The week feeds the day, but nothing feeds the week except the lists.
        if period == Period::Day {
            for candidate in self.period_tasks(Period::Week)? {
                push(candidate, &mut out);
            }
        }

        for entry in self.lists()? {
            if entry.name == COMPLETED_LIST {
                continue;
            }
            for task in self.tasks_in(&entry.path)? {
                push(
                    ListedTask {
                        path: entry.path.clone(),
                        task,
                    },
                    &mut out,
                );
            }
        }
        Ok(out)
    }

    // ------------------------------------------------------- complete / undo

    /// Completes a task: it moves to the **same folder's** Completed with its
    /// origin recorded, and stops being pulled into Today and This Week.
    ///
    /// A repeating task leaves its next occurrence behind in the same list,
    /// so finishing it is also what schedules it — there is no scheduler.
    pub fn complete_task(&self, path: &str, id: &str) -> Result<Task> {
        self.ensure_writable()?;

        let respawned = self
            .open_list(path)?
            .find(id)
            .and_then(crate::recurrence::respawn);

        let completed = Self::completed_path_of(path)?;
        let task = self.transfer(id, path, &completed, OriginAction::Record, Some(true))?;

        if let Some(next) = respawned {
            let mut source = self.open_list(path)?;
            source.add(next);
            source.save()?;
        }

        // The task left the list, so any reference to it is now dangling.
        for period in [Period::Day, Period::Week] {
            let mut state = self.open_state(period)?;
            if state.state.remove(path, id) {
                state.save()?;
            }
        }
        Ok(task)
    }

    /// Un-completes a task, sending it back to the list it came from.
    ///
    /// `completed` is the address of the Completed list holding the task —
    /// with one Completed per widget (spec 3.5), the id alone cannot say
    /// which folder to undo in. The origin is a bare name resolved **inside
    /// that same folder**; a task with no usable origin — hand-written, or
    /// pointing at a name that is no longer valid — lands in the folder's
    /// Inbox rather than nowhere. The origin list is recreated when it no
    /// longer exists.
    pub fn uncomplete_task(&self, completed: &str, id: &str) -> Result<Task> {
        self.ensure_writable()?;
        let (folder, _) = self.resolve_list(completed)?;
        let list = self.open_list(completed)?;
        let task = list
            .find(id)
            .ok_or_else(|| Error::TaskNotFound(id.to_string()))?;

        let target_name = match task.origin.as_deref() {
            Some(origin) if folder.list_path(origin).is_ok() => origin.to_string(),
            _ => INBOX_LIST.to_string(),
        };
        let (dir, _) = split_list_path(completed)?;
        let target = format!("{dir}/{target_name}.md");

        self.transfer(id, completed, &target, OriginAction::Clear, Some(false))
    }
}
