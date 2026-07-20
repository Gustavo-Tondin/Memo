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
//! A notebook created before 2026-07-20 uses the Portuguese names and is
//! migrated on open — see [`Notebook::migrate_legacy_names`].

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

/// Written into every new notebook, for whoever opens the folder without the
/// app. Deliberately short: someone reading this is looking at a text file,
/// not at documentation.
const FORMAT_GUIDE: &str = "\
These are plain Markdown checklists. Edit them in any text editor —
Memo reads whatever you write.

  - [ ] Buy milk
  - [x] Pay the bill

You can add details on indented lines below a task:

  - [ ] Buy building material
    @2026-07-25 #home #urgent !2
    Talk to Jorge first, he gives a discount.
    repeat: every-week
    - [ ] Cement
    - [ ] Sand

  @2026-07-25   a date, always year-month-day
  #home         a tag
  !1 to !3      priority, 1 is highest
  repeat:       every-day, every-week, every-month, every-3-days...
  - [ ] ...     a subtask
  anything else on an indented line is a description

Two tags mean something to the app: #urgent and #pinned.

The <!--id:...--> comments are Memo's. It adds one to a task only when it
needs to keep track of it — when you pull it into your day or week, or
complete it. You never have to write those yourself, and you can leave them
alone.

Delete a list file and Memo forgets that list. Inbox.md and Completed.md
come back automatically.
";

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
/// Day and Week show tasks from several lists at once, so the list name has
/// to travel with the task — without it the UI could not tell the core which
/// file to act on.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ListedTask {
    pub list: String,
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
    pub list: String,
    pub task: Task,
    pub group: SuggestionGroup,
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
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let root = path.as_ref().to_path_buf();
        if !Self::is_notebook(&root) {
            return Err(Error::NotANotebook(root));
        }

        let config = Config::load(root.join(NOTEBOOK_CONFIG_DIR).join("config.json"));
        let notebook = Self { root, config };

        // A notebook written by a newer app is opened for reading only, so
        // nothing here may touch the disk.
        if !notebook.is_read_only() {
            notebook.migrate_legacy_names()?;
            notebook.ensure_default_lists()?;
            notebook.write_format_guide()?;
        }
        Ok(notebook)
    }

    /// Renames the Portuguese folders and list of older notebooks to the
    /// English names the app settled on.
    ///
    /// Breaking a notebook that already exists is never an option: this same
    /// rename will run on other people's notebooks when they update, so it
    /// has to be boring and safe.
    ///
    /// Refuses to act whenever the destination already exists — two `Tasks/`
    /// folders would be a worse problem than one `Tarefas/`.
    fn migrate_legacy_names(&self) -> Result<()> {
        let renamed_dirs = [
            (crate::legacy::TASKS_DIR, TASKS_DIR),
            (crate::legacy::NOTES_DIR, NOTES_DIR),
        ];
        for (from, to) in renamed_dirs {
            let old = self.root.join(from);
            let new = self.root.join(to);
            if old.is_dir() && !new.exists() {
                std::fs::rename(&old, &new).ctx(&new)?;
            }
        }

        let old_completed = self.tasks_dir().join(format!("{}.md", crate::legacy::COMPLETED_LIST));
        let new_completed = self.tasks_dir().join(format!("{COMPLETED_LIST}.md"));
        if old_completed.is_file() && !new_completed.exists() {
            std::fs::rename(&old_completed, &new_completed).ctx(&new_completed)?;
        }

        // Folder names never appear in `origin` or in the states — both store
        // *list* names. Only the completed list was renamed, and it is a
        // destination, not an origin, so this repoint matters just for a
        // notebook someone edited by hand. Cheap enough to do anyway.
        self.repoint_legacy_completed()
    }

    fn repoint_legacy_completed(&self) -> Result<()> {
        let legacy = crate::legacy::COMPLETED_LIST;

        for period in [Period::Day, Period::Week] {
            let path = self.state_path(period);
            if !path.exists() {
                continue;
            }
            let mut file = StateFile::load(&path, self.current_period_date(period));
            if file.state.rename_list(legacy, COMPLETED_LIST) {
                file.save()?;
            }
        }

        for name in self.list_names()? {
            let mut list = self.open_list(&name)?;
            if list.repoint_origin(legacy, COMPLETED_LIST) > 0 {
                list.save()?;
            }
        }
        Ok(())
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

        let notebook = Self {
            root,
            config: Config::default(),
        };
        notebook.config.save(notebook.config_path())?;
        notebook.ensure_default_lists()?;
        notebook.write_format_guide()?;
        Ok(notebook)
    }

    /// Drops a plain-text guide next to the lists, for whoever opens the
    /// folder without the app.
    ///
    /// `.txt` on purpose: the app only reads `.md`, so the guide never shows
    /// up as a list.
    fn write_format_guide(&self) -> Result<()> {
        let path = self.tasks_dir().join("_FORMAT.txt");
        if path.exists() {
            return Ok(());
        }
        std::fs::write(&path, FORMAT_GUIDE).ctx(&path)
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
            // A copy left behind by a sync tool is not a list the user made.
            if crate::conflict::is_conflict_file(&path) {
                continue;
            }
            if path.extension().is_some_and(|ext| ext == "md") {
                if let Some(stem) = path.file_stem() {
                    names.push(stem.to_string_lossy().to_string());
                }
            }
        }
        names.sort();
        Ok(names)
    }

    /// How many open tasks each list has, for the navigation.
    ///
    /// One pass over the whole notebook instead of one read per list: the
    /// sidebar shows every count at once, so asking list by list would re-read
    /// the same folder N times on every render.
    ///
    /// Counts **open** tasks — a list of finished things reads as empty, which
    /// is what "3 left to do" means to someone looking at a sidebar. The
    /// completed list is skipped entirely, since everything in it is done.
    pub fn open_task_counts(&self) -> Result<BTreeMap<String, usize>> {
        let mut counts = BTreeMap::new();
        for name in self.list_names()? {
            if name == COMPLETED_LIST {
                continue;
            }
            // Reading without adopting ids: counting is not a reason to write
            // to every file in the notebook.
            let open = self
                .open_list(&name)?
                .tasks()
                .filter(|task| !task.done)
                .count();
            counts.insert(name, open);
        }
        Ok(counts)
    }

    /// Conflicting copies sitting in the notebook right now.
    ///
    /// Scans the tasks folder and the config folder, which is where sync tools
    /// leave them. Reporting is all this does — the user decides what to keep.
    pub fn conflicts(&self) -> Result<Vec<Conflict>> {
        let mut found = Vec::new();
        for dir in [self.tasks_dir(), self.config_dir()] {
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

    pub fn open_list(&self, name: &str) -> Result<TaskList> {
        TaskList::load(self.list_path(name)?)
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
    pub fn tasks_in(&self, list: &str) -> Result<Vec<Task>> {
        let mut tasks = self.open_list(list)?;
        if !self.is_read_only() && tasks.dedupe_ids() > 0 {
            tasks.save()?;
        }
        Ok(tasks.tasks().cloned().collect())
    }

    /// Gives the task at `position` in `list` an id, and returns it.
    ///
    /// The frontend shows tasks by position; the moment the user acts on one
    /// — pulls it into a period, completes it — it needs a stable name. This
    /// is where a task earns one.
    pub fn ensure_task_id(&self, list: &str, position: usize) -> Result<String> {
        self.ensure_writable()?;
        let mut tasks = self.open_list(list)?;

        let existing = tasks
            .tasks()
            .nth(position)
            .ok_or_else(|| Error::TaskNotFound(format!("{list}[{position}]")))?
            .id
            .clone();
        if let Some(id) = existing {
            return Ok(id);
        }

        let id = tasks
            .ensure_id_at(position)
            .ok_or_else(|| Error::TaskNotFound(format!("{list}[{position}]")))?;
        tasks.save()?;
        Ok(id)
    }

    pub fn inbox(&self) -> Result<TaskList> {
        self.open_list(INBOX_LIST)
    }

    pub fn completed(&self) -> Result<TaskList> {
        self.open_list(COMPLETED_LIST)
    }

    /// Whether a list is one the app recreates on every open.
    pub fn is_default_list(name: &str) -> bool {
        name == INBOX_LIST || name == COMPLETED_LIST
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
        self.ensure_writable()?;
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

    /// Renames a user list, repointing everything that referred to it: the
    /// `origin` of completed tasks (otherwise undo would send them to a list
    /// that no longer exists) and the day/week states.
    pub fn rename_list(&self, from: &str, to: &str) -> Result<()> {
        self.ensure_writable()?;
        if Self::is_default_list(from) {
            return Err(Error::ProtectedList(from.to_string()));
        }
        if Self::is_default_list(to) {
            return Err(Error::InvalidListName(to.to_string()));
        }

        let source = self.list_path(from)?;
        let target = self.list_path(to)?;
        if !source.exists() {
            return Err(Error::InvalidListName(format!("{from} does not exist")));
        }
        if target.exists() {
            return Err(Error::InvalidListName(format!("{to} already exists")));
        }

        std::fs::rename(&source, &target).ctx(&target)?;

        let mut completed = self.completed()?;
        if completed.repoint_origin(from, to) > 0 {
            completed.save()?;
        }

        for period in [Period::Day, Period::Week] {
            let mut state = self.open_state(period)?;
            if state.state.rename_list(from, to) {
                state.save()?;
            }
        }
        Ok(())
    }

    /// Deletes a user list, moving whatever was still in it to the Inbox.
    ///
    /// Deleting a list is a filing decision, not a decision to throw work
    /// away — principle 2, the data is the user's. An empty list disappears
    /// silently; one with tasks leaves them in the Inbox.
    pub fn delete_list(&self, name: &str) -> Result<usize> {
        self.ensure_writable()?;
        if Self::is_default_list(name) {
            return Err(Error::ProtectedList(name.to_string()));
        }

        let path = self.list_path(name)?;
        if !path.exists() {
            return Err(Error::InvalidListName(format!("{name} does not exist")));
        }

        // Every task moves, not just the ones that happen to have an id —
        // most tasks never earn one, and losing them here would be silent.
        let list = TaskList::load(&path)?;
        let rescued: Vec<Task> = list.tasks().cloned().collect();

        let mut inbox = self.inbox()?;
        for task in &rescued {
            inbox.add(task.clone());
        }

        // Inbox first, then the file goes away: a crash in between leaves a
        // duplicate, never a hole.
        inbox.save()?;
        std::fs::remove_file(&path).ctx(&path)?;
        let rescued = rescued.len();

        for period in [Period::Day, Period::Week] {
            let mut state = self.open_state(period)?;
            // References now point at the Inbox copies, which carry the same
            // ids; repointing keeps a pulled task pulled.
            if state.state.rename_list(name, INBOX_LIST) {
                state.save()?;
            }
        }
        Ok(rescued)
    }

    /// Moves a task between lists, preserving its id.
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
    fn transfer(
        &self,
        id: &str,
        from: &str,
        to: &str,
        origin: OriginAction,
        done: Option<bool>,
    ) -> Result<Task> {
        self.ensure_writable()?;
        let mut source = self.open_list(from)?;
        let mut target = self.open_list(to)?;

        let mut task = source.remove(id)?;
        match origin {
            OriginAction::Record => task.origin = Some(from.to_string()),
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
        let now = chrono::Local::now();
        match period {
            Period::Day => clock::next_daily_turn_at(now, self.config.rollover.daily.at),
            Period::Week => clock::next_weekly_turn_at(
                now,
                self.config.rollover.weekly.at,
                self.config.rollover.weekly.starts_on,
            ),
        }
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
    pub fn pull_into(&self, period: Period, list: &str, id: &str) -> Result<bool> {
        self.ensure_writable()?;
        // Fail before writing the state if the task is not really there —
        // a reference to a missing task shows up as a ghost row in the UI.
        let source = self.open_list(list)?;
        if source.find(id).is_none() {
            return Err(Error::TaskNotFound(id.to_string()));
        }

        let mut file = self.open_state(period)?;
        if !file.state.add(list, id) {
            return Ok(false);
        }
        file.save()?;
        Ok(true)
    }

    /// Removes a task from Today or This Week. The task itself is untouched.
    pub fn remove_from(&self, period: Period, list: &str, id: &str) -> Result<bool> {
        self.ensure_writable()?;
        let mut file = self.open_state(period)?;
        if !file.state.remove(list, id) {
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
        file.state.add(INBOX_LIST, &id);
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
            let Ok(list) = self.open_list(&reference.list) else {
                continue;
            };
            if let Some(task) = list.find(&reference.id) {
                out.push(ListedTask {
                    list: reference.list.clone(),
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
                .map(|r| (r.list.clone(), r.id.clone()))
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
                    .is_some_and(|id| in_week.contains(&(entry.list.clone(), id.clone())))
                {
                    SuggestionGroup::ThisWeek
                } else {
                    SuggestionGroup::Lists
                };
                Suggestion {
                    list: entry.list,
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
                if pulled.contains(&candidate.list, id) {
                    return;
                }
                let already = out
                    .iter()
                    .any(|t| t.list == candidate.list && t.task.id.as_deref() == Some(id));
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

        for name in self.list_names()? {
            if name == COMPLETED_LIST {
                continue;
            }
            for task in self.tasks_in(&name)? {
                push(
                    ListedTask {
                        list: name.clone(),
                        task,
                    },
                    &mut out,
                );
            }
        }
        Ok(out)
    }

    // ------------------------------------------------------- complete / undo

    /// Completes a task: it moves to the completed list with its origin
    /// recorded, and stops being pulled into Today and This Week.
    ///
    /// A repeating task leaves its next occurrence behind in the same list,
    /// so finishing it is also what schedules it — there is no scheduler.
    pub fn complete_task(&self, list: &str, id: &str) -> Result<Task> {
        self.ensure_writable()?;

        let respawned = self
            .open_list(list)?
            .find(id)
            .and_then(crate::recurrence::respawn);

        let task = self.transfer(id, list, COMPLETED_LIST, OriginAction::Record, Some(true))?;

        if let Some(next) = respawned {
            let mut source = self.open_list(list)?;
            source.add(next);
            source.save()?;
        }

        // The task left the list, so any reference to it is now dangling.
        for period in [Period::Day, Period::Week] {
            let mut state = self.open_state(period)?;
            if state.state.remove(list, id) {
                state.save()?;
            }
        }
        Ok(task)
    }

    /// Un-completes a task, sending it back to the list it came from.
    ///
    /// The origin list is recreated when it no longer exists. A task with no
    /// usable origin — hand-written into `Completas.md`, or pointing at a name
    /// that is no longer valid — lands in the Inbox rather than nowhere.
    pub fn uncomplete_task(&self, id: &str) -> Result<Task> {
        self.ensure_writable()?;
        let completed = self.completed()?;
        let task = completed
            .find(id)
            .ok_or_else(|| Error::TaskNotFound(id.to_string()))?;

        let target = match task.origin.as_deref() {
            Some(origin) if self.list_path(origin).is_ok() => origin.to_string(),
            _ => INBOX_LIST.to_string(),
        };

        self.transfer(
            id,
            COMPLETED_LIST,
            &target,
            OriginAction::Clear,
            Some(false),
        )
    }
}
