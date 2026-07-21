//! One `invoke()` command per core operation.
//!
//! Every function here is a wire: read the arguments, call `memo_core`, hand
//! back the result. Any `if` that decides something about tasks, lists or
//! dates belongs in the core instead — see the architecture rule in
//! `CLAUDE.md`.

use std::path::PathBuf;

use memo_core::config::{Config, RolloverMode};
use memo_core::state::{Period, PeriodState};
use memo_core::{Conflict, ListedTask, Notebook, Task, TurnOffset, WeekStart};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::DialogExt;

use crate::error::CommandResult;
use crate::state::AppState;

/// The addresses the core creates, so the frontend never hard-codes them.
///
/// The frontend used to mirror these in a `names.js` — and when the core
/// renamed `Completas` to `Completed` in phase 5, the completed screen kept
/// reading a file that no longer existed and showed "nothing done yet"
/// forever. Coming over the bridge, a rename reaches every screen at once.
///
/// Since phase 7 these are **paths**, not names: `Tasks/Inbox.md`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookLayout {
    /// Where quick-captured tasks land.
    pub inbox: String,
    /// The fixed workspace's completed list.
    pub completed: String,
    /// The folder new lists are created in, until the UI is workspace-aware.
    pub tasks_folder: String,
    /// The per-folder completed list's NAME (`Completed`) — every tasks
    /// widget has one, and the UI must not hard-code it (the names.js lesson).
    pub completed_name: String,
}

/// What the frontend needs to know about the open notebook.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookInfo {
    pub path: PathBuf,
    /// Folder name, which is what the user recognizes as the notebook's name.
    pub name: String,
    pub read_only: bool,
    /// Every list as `{ path, name }` — the path is the address commands
    /// take, the name is what the user reads.
    pub lists: Vec<memo_core::notebook::ListEntry>,
    pub layout: NotebookLayout,
}

impl NotebookInfo {
    fn of(notebook: &Notebook) -> CommandResult<Self> {
        Ok(Self {
            path: notebook.root().to_path_buf(),
            name: notebook
                .root()
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            read_only: notebook.is_read_only(),
            lists: notebook.lists()?,
            layout: NotebookLayout {
                inbox: Notebook::inbox_path(),
                completed: Notebook::completed_path_of(&Notebook::inbox_path())?,
                tasks_folder: memo_core::TASKS_DIR.to_string(),
                completed_name: memo_core::COMPLETED_LIST.to_string(),
            },
        })
    }
}

/// The notebook preferences, flattened for the UI.
///
/// Every field is a plain string or bool: the frontend should not have to know
/// the core's types, and a value it cannot parse still round-trips.
///
/// Everything is optional on the way **in**: a missing field keeps whatever is
/// stored, instead of failing the whole call. Otherwise adding a preference
/// here would break every caller that does not know about it yet — including
/// an older frontend against a newer shell. On the way **out** all fields are
/// filled.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NotebookSettings {
    pub daily_mode: Option<String>,
    pub daily_at: Option<String>,
    pub weekly_mode: Option<String>,
    pub weekly_at: Option<String>,
    pub week_starts_on: Option<String>,
    pub restore_last_screen: Option<bool>,
    pub show_list_counts: Option<bool>,
    pub auto_urgent_by_date: Option<bool>,
}

#[tauri::command]
pub fn core_version() -> String {
    memo_core::version().to_string()
}

// --------------------------------------------------------------- notebook

/// Opens the native folder picker. `None` when the user cancels.
///
/// Two things here are not optional, and getting either wrong freezes the
/// window the moment the dialog opens:
///
/// 1. **The command must be `async`.** A synchronous `#[tauri::command]` runs
///    on the main thread, which is the same thread that drives the GTK event
///    loop — and therefore the dialog itself.
/// 2. **The callback form, not `blocking_pick_folder`.** The plugin is
///    explicit that the blocking variants must never run on the main thread.
///    Waiting for the answer on a blocking-pool thread keeps the main thread
///    free to actually draw the dialog, whatever thread Tauri picks for the
///    command in the future.
#[tauri::command]
pub async fn pick_notebook_folder<R: Runtime>(app: AppHandle<R>) -> Option<PathBuf> {
    let (tx, rx) = std::sync::mpsc::channel();

    // Fires on the main thread when the user answers; sending never blocks.
    app.dialog().file().pick_folder(move |folder| {
        let _ = tx.send(folder);
    });

    tauri::async_runtime::spawn_blocking(move || rx.recv().ok().flatten())
        .await
        .ok()
        .flatten()
        .and_then(|folder| folder.into_path().ok())
}

/// Opens a notebook, creating one in that folder if it is not one yet.
#[tauri::command]
pub fn open_notebook<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    path: PathBuf,
) -> CommandResult<NotebookInfo> {
    let notebook = Notebook::open_or_init(&path)?;
    let info = NotebookInfo::of(&notebook)?;
    state.open(&app, notebook)?;
    crate::prefs::remember_notebook(&app, &path);
    Ok(info)
}

/// The notebook open when the app was last closed, so onboarding can reopen
/// it instead of asking for the folder every launch. `None` when there is
/// none, or when the folder is gone.
#[tauri::command]
pub fn last_notebook<R: Runtime>(app: AppHandle<R>) -> Option<PathBuf> {
    crate::prefs::last_notebook(&app)
}

/// The notebook currently open, if any.
#[tauri::command]
pub fn current_notebook(state: State<'_, AppState>) -> Option<NotebookInfo> {
    if !state.is_open() {
        return None;
    }
    state.with_notebook(|nb| NotebookInfo::of(nb)).ok()
}

/// Open task count per list, for the navigation. Empty when the user turned
/// the counters off — the frontend does not need to know the rule.
#[tauri::command]
pub fn list_counts(
    state: State<'_, AppState>,
) -> CommandResult<std::collections::BTreeMap<String, usize>> {
    state.with_notebook(|nb| {
        if !nb.config().show_list_counts {
            return Ok(Default::default());
        }
        Ok(nb.open_task_counts()?)
    })
}

/// Which screen to open on launch.
///
/// `None` means "use the default" — either the user never left one, or the
/// notebook has `restoreLastScreen` off. The value itself is machine-local;
/// the preference to use it travels with the notebook.
#[tauri::command]
pub fn screen_to_restore<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> CommandResult<Option<String>> {
    state.with_notebook(|nb| {
        if !nb.config().restore_last_screen {
            return Ok(None);
        }
        Ok(crate::prefs::last_screen(&app))
    })
}

/// Records the current screen. No-op when the notebook has the preference off,
/// so turning it on later does not restore a screen from months ago.
#[tauri::command]
pub fn remember_screen<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    screen: String,
) -> CommandResult<()> {
    state.with_notebook(|nb| {
        if nb.config().restore_last_screen {
            crate::prefs::remember_screen(&app, &screen);
        }
        Ok(())
    })
}

#[tauri::command]
pub fn notebook_settings(state: State<'_, AppState>) -> CommandResult<NotebookSettings> {
    state.with_notebook(|nb| {
        let config = nb.config();
        let rollover = config.rollover;
        Ok(NotebookSettings {
            daily_mode: Some(rollover.daily.mode.render().to_string()),
            daily_at: Some(rollover.daily.at.render()),
            weekly_mode: Some(rollover.weekly.mode.render().to_string()),
            weekly_at: Some(rollover.weekly.at.render()),
            week_starts_on: Some(rollover.weekly.starts_on.render().to_string()),
            restore_last_screen: Some(config.restore_last_screen),
            show_list_counts: Some(config.show_list_counts),
            auto_urgent_by_date: Some(config.auto_urgent_by_date),
        })
    })
}

/// Saves the rollover preferences. Unparseable values fall back to the
/// defaults in the core, so the UI cannot write a broken config.
#[tauri::command]
pub fn set_notebook_settings(
    state: State<'_, AppState>,
    settings: NotebookSettings,
) -> CommandResult<()> {
    state.with_notebook_mut(|nb| {
        let mut config: Config = nb.config().clone();
        let r = &mut config.rollover;

        // A value that arrives unparseable falls back to the core's default,
        // so the UI cannot write a broken config; a value that does not arrive
        // at all is left exactly as it was.
        if let Some(v) = &settings.daily_mode {
            r.daily.mode = RolloverMode::parse_or_default(v);
        }
        if let Some(v) = &settings.daily_at {
            r.daily.at = TurnOffset::parse_or_default(v);
        }
        if let Some(v) = &settings.weekly_mode {
            r.weekly.mode = RolloverMode::parse_or_default(v);
        }
        if let Some(v) = &settings.weekly_at {
            r.weekly.at = TurnOffset::parse_or_default(v);
        }
        if let Some(v) = &settings.week_starts_on {
            r.weekly.starts_on = WeekStart::parse_or_default(v);
        }
        if let Some(v) = settings.restore_last_screen {
            config.restore_last_screen = v;
        }
        if let Some(v) = settings.show_list_counts {
            config.show_list_counts = v;
        }
        if let Some(v) = settings.auto_urgent_by_date {
            config.auto_urgent_by_date = v;
        }

        nb.set_config(config)?;
        Ok(())
    })
}

// ------------------------------------------------------------------ lists

#[tauri::command]
pub fn list_names(state: State<'_, AppState>) -> CommandResult<Vec<memo_core::notebook::ListEntry>> {
    state.with_notebook(|nb| Ok(nb.lists()?))
}

/// Conflicting copies a sync tool left in the notebook.
///
/// The app reports them; resolving is the user's call, since guessing which
/// side to keep is how work gets lost.
#[tauri::command]
pub fn list_conflicts(state: State<'_, AppState>) -> CommandResult<Vec<Conflict>> {
    state.with_notebook(|nb| Ok(nb.conflicts()?))
}

#[tauri::command]
pub fn list_tasks(state: State<'_, AppState>, list: String) -> CommandResult<Vec<Task>> {
    state.with_notebook(|nb| Ok(nb.tasks_in(&list)?))
}

/// Creates a list inside `folder` (a root-relative workspace folder, e.g.
/// `Tasks` — the UI takes it from `layout.tasksFolder` until it is
/// workspace-aware).
#[tauri::command]
pub fn create_list(
    state: State<'_, AppState>,
    folder: String,
    name: String,
) -> CommandResult<()> {
    state.with_notebook(|nb| {
        nb.create_list(&folder, &name)?;
        Ok(())
    })
}

#[tauri::command]
pub fn rename_list(state: State<'_, AppState>, from: String, to: String) -> CommandResult<()> {
    state.with_notebook(|nb| Ok(nb.rename_list(&from, &to)?))
}

/// Deletes a list. Returns how many tasks were moved to the Inbox.
#[tauri::command]
pub fn delete_list(state: State<'_, AppState>, name: String) -> CommandResult<usize> {
    state.with_notebook(|nb| Ok(nb.delete_list(&name)?))
}

// ------------------------------------------------------------------ tasks

/// Creates a task and returns its **position** in the list, not an id.
///
/// A new task has no id: ids are handed out only when something needs to
/// address the task (see `ensure_task_id`), which is what keeps a plain
/// checklist free of comments.
#[tauri::command]
pub fn create_task(
    state: State<'_, AppState>,
    list: String,
    text: String,
) -> CommandResult<usize> {
    state.with_notebook(|nb| {
        let mut tasks = nb.open_list(&list)?;
        let position = tasks.add_text(text);
        tasks.save()?;
        Ok(position)
    })
}

/// Gives the task at `position` a stable id, and returns it.
///
/// The UI works with positions; the moment the user acts on a task — pulls it
/// into a period, completes it — it needs a name that survives reordering.
#[tauri::command]
pub fn ensure_task_id(
    state: State<'_, AppState>,
    list: String,
    position: usize,
) -> CommandResult<String> {
    state.with_notebook(|nb| Ok(nb.ensure_task_id(&list, position)?))
}

#[tauri::command]
pub fn edit_task_text(
    state: State<'_, AppState>,
    list: String,
    id: String,
    text: String,
) -> CommandResult<()> {
    state.with_notebook(|nb| {
        let mut tasks = nb.open_list(&list)?;
        tasks.edit_text(&id, text)?;
        tasks.save()?;
        Ok(())
    })
}

/// The editable fields of a task, all optional.
///
/// Absent means "leave alone"; present-but-null means "clear". Without that
/// distinction there would be no way to remove a due date.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TaskFields {
    pub text: Option<String>,
    #[serde(deserialize_with = "present_or_absent")]
    pub due: Option<Option<String>>,
    #[serde(deserialize_with = "present_or_absent")]
    pub priority: Option<Option<u8>>,
    pub tags: Option<Vec<String>>,
    pub description: Option<Vec<String>>,
    #[serde(deserialize_with = "present_or_absent")]
    pub repeat: Option<Option<String>>,
    pub subtasks: Option<Vec<SubtaskInput>>,
}

/// Tells "field absent" apart from "field sent as null".
///
/// By default serde collapses both into `None`, which would make clearing a
/// due date impossible: the UI has no other way to say "remove this".
fn present_or_absent<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(deserializer).map(Some)
}

#[derive(Debug, Deserialize)]
pub struct SubtaskInput {
    pub text: String,
    pub done: bool,
}

/// Edits any field of a task in one call.
///
/// One command instead of one per field: the UI edits a task in a panel and
/// saves it as a whole, and a half-applied edit would be worse than none.
#[tauri::command]
pub fn set_task_fields(
    state: State<'_, AppState>,
    list: String,
    id: String,
    fields: TaskFields,
) -> CommandResult<()> {
    state.with_notebook(|nb| {
        let mut tasks = nb.open_list(&list)?;
        let task = tasks.task_mut(&id)?;

        if let Some(text) = fields.text {
            task.text = memo_core::task::single_line(&text);
        }
        if let Some(due) = fields.due {
            // An unparseable date clears it rather than being stored wrong.
            task.due = due.as_deref().and_then(memo_core::task::parse_date);
        }
        if let Some(priority) = fields.priority {
            task.priority = priority.filter(|p| (1..=3).contains(p));
        }
        if let Some(tags) = fields.tags {
            // Normalised by the core: a spaced tag would silently turn the
            // whole metadata line into description on the next read.
            let mut cleaned: Vec<String> = Vec::new();
            for tag in &tags {
                if let Some(tag) = memo_core::task::normalize_tag(tag) {
                    if !cleaned.contains(&tag) {
                        cleaned.push(tag);
                    }
                }
            }
            task.tags = cleaned;
        }
        if let Some(description) = fields.description {
            // An embedded newline becomes a further line; a blank line would
            // end the task's block in the file and cut the description short.
            task.description = description
                .iter()
                .flat_map(|entry| entry.lines())
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect();
        }
        if let Some(repeat) = fields.repeat {
            task.repeat = repeat.as_deref().and_then(memo_core::task::Repeat::parse);
        }
        if let Some(subtasks) = fields.subtasks {
            task.subtasks = subtasks
                .into_iter()
                .map(|s| memo_core::task::Subtask {
                    text: memo_core::task::single_line(&s.text),
                    done: s.done,
                })
                .collect();
        }

        tasks.save()?;
        Ok(())
    })
}

/// Reorders a task inside its list. Positions count tasks, not lines.
#[tauri::command]
pub fn move_task_to(
    state: State<'_, AppState>,
    list: String,
    from: usize,
    to: usize,
) -> CommandResult<()> {
    state.with_notebook(|nb| {
        let mut tasks = nb.open_list(&list)?;
        tasks.move_task_to(from, to)?;
        tasks.save()?;
        Ok(())
    })
}

#[tauri::command]
pub fn complete_task(
    state: State<'_, AppState>,
    list: String,
    id: String,
) -> CommandResult<Task> {
    state.with_notebook(|nb| Ok(nb.complete_task(&list, &id)?))
}

/// Un-completes a task. `list` is the address of the Completed list it sits
/// in — with one Completed per widget, the id alone cannot say which folder
/// to undo in.
#[tauri::command]
pub fn uncomplete_task(
    state: State<'_, AppState>,
    list: String,
    id: String,
) -> CommandResult<Task> {
    state.with_notebook(|nb| Ok(nb.uncomplete_task(&list, &id)?))
}

// --------------------------------------------------------- day and week

/// The state of Today or This Week, with any pending rollover applied.
#[tauri::command]
pub fn period_state(
    state: State<'_, AppState>,
    period: Period,
) -> CommandResult<PeriodState> {
    state.with_notebook(|nb| Ok(nb.open_state(period)?.state))
}

#[tauri::command]
pub fn pull_into_period(
    state: State<'_, AppState>,
    period: Period,
    list: String,
    id: String,
) -> CommandResult<bool> {
    state.with_notebook(|nb| Ok(nb.pull_into(period, &list, &id)?))
}

#[tauri::command]
pub fn remove_from_period(
    state: State<'_, AppState>,
    period: Period,
    list: String,
    id: String,
) -> CommandResult<bool> {
    state.with_notebook(|nb| Ok(nb.remove_from(period, &list, &id)?))
}

/// Creates a task straight from Today or This Week. It is written to the
/// Inbox — the periods only ever hold references.
#[tauri::command]
pub fn add_task_in_period(
    state: State<'_, AppState>,
    period: Period,
    text: String,
) -> CommandResult<String> {
    state.with_notebook(|nb| Ok(nb.add_task_in_period(period, text)?))
}

/// The tasks pulled into a period, resolved to the real thing.
#[tauri::command]
pub fn period_tasks(
    state: State<'_, AppState>,
    period: Period,
) -> CommandResult<Vec<ListedTask>> {
    state.with_notebook(|nb| Ok(nb.period_tasks(period)?))
}

/// What to offer pulling into a period, already in display order.
#[tauri::command]
pub fn period_suggestions(
    state: State<'_, AppState>,
    period: Period,
) -> CommandResult<Vec<ListedTask>> {
    state.with_notebook(|nb| Ok(nb.suggestions_for(period)?))
}

/// The current logical day and week, and when each turns next. The UI needs
/// this both to label the screens and to schedule the in-app rollover.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeriodClock {
    pub today: String,
    pub week_start: String,
    pub next_daily_turn: String,
    pub next_weekly_turn: String,
}

fn clock_of(nb: &Notebook) -> PeriodClock {
    PeriodClock {
        today: nb.today().to_string(),
        week_start: nb.current_week().to_string(),
        next_daily_turn: nb.next_turn_at(Period::Day).to_rfc3339(),
        next_weekly_turn: nb.next_turn_at(Period::Week).to_rfc3339(),
    }
}

#[tauri::command]
pub fn period_clock(state: State<'_, AppState>) -> CommandResult<PeriodClock> {
    state.with_notebook(|nb| Ok(clock_of(nb)))
}

/// Re-reads both period states, applying any rollover that came due while the
/// app was open. The frontend calls this when the scheduled turn arrives.
#[tauri::command]
pub fn refresh_periods(state: State<'_, AppState>) -> CommandResult<Vec<PeriodState>> {
    state.with_notebook(|nb| {
        Ok(vec![
            nb.open_state(Period::Day)?.state,
            nb.open_state(Period::Week)?.state,
        ])
    })
}

/// Kept from phase 0 so the frontend can prove the bridge is alive.
#[tauri::command]
pub fn is_notebook_open(state: State<'_, AppState>) -> bool {
    state.is_open()
}

/// A widget as the frontend renders it (phase 7.5).
///
/// `kind` is whatever the config says — an unknown one is delivered, not
/// dropped, so the UI can show its "unsupported" card and the folder stays
/// untouched. `folder` arrives resolved to a root-relative path; a folder
/// that tried to escape the workspace resolves to `None` with
/// `invalid_folder` set — a broken template must not take the workspace down.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WidgetInfo {
    pub kind: String,
    pub known: bool,
    pub folder: Option<String>,
    pub invalid_folder: bool,
    pub options: serde_json::Value,
}

/// A workspace as the navigation shows it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    /// The folder — the stable identity; renaming the folder renames the
    /// workspace.
    pub folder_name: String,
    /// What the user reads (config `name`, falling back to the folder).
    pub name: String,
    /// One of the three the app creates and recreates (Home, Tasks, Notes).
    /// The UI gives these dedicated navigation; user workspaces get the
    /// generic widget runtime.
    pub fixed: bool,
    pub read_only: bool,
    pub widgets: Vec<WidgetInfo>,
}

/// The workspaces of the notebook, ready to render.
#[tauri::command]
pub fn workspaces(state: State<'_, AppState>) -> CommandResult<Vec<WorkspaceInfo>> {
    state.with_notebook(|nb| Ok(workspaces_of(nb)?))
}

fn workspaces_of(nb: &Notebook) -> CommandResult<Vec<WorkspaceInfo>> {
    const FIXED: [&str; 3] = ["Home", memo_core::TASKS_DIR, memo_core::NOTES_DIR];

    let mut out = Vec::new();
    for workspace in nb.workspaces()? {
        let widgets = workspace
            .config
            .widgets
            .iter()
            .map(|spec| {
                let resolved = workspace.widget_dir(spec);
                let invalid = resolved.is_err();
                let folder = resolved.ok().flatten().map(|dir| {
                    dir.strip_prefix(nb.root())
                        .unwrap_or(&dir)
                        .to_string_lossy()
                        .replace('\\', "/")
                });
                WidgetInfo {
                    kind: spec.kind.clone(),
                    known: spec.is_known(),
                    folder,
                    invalid_folder: invalid,
                    options: spec.options.clone(),
                }
            })
            .collect();

        out.push(WorkspaceInfo {
            folder_name: workspace.folder_name().to_string(),
            name: workspace.display_name().to_string(),
            fixed: FIXED.contains(&workspace.folder_name()),
            read_only: workspace.config.is_read_only(),
            widgets,
        });
    }
    Ok(out)
}

/// Everything the shell of the UI needs after any change, in one round trip.
///
/// Every action used to fan out into four `invoke()`s (info, clock, counts,
/// conflicts) — and the auto-save fires that cascade on every pause in
/// typing. One command keeps the cost flat as notebooks grow. This is
/// consolidation of round trips only: nothing is cached, the files stay the
/// source of truth, and each call re-reads them.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookSnapshot {
    pub info: NotebookInfo,
    pub clock: PeriodClock,
    /// Empty when the user turned the counters off.
    pub counts: std::collections::BTreeMap<String, usize>,
    pub conflicts: Vec<Conflict>,
    pub workspaces: Vec<WorkspaceInfo>,
}

#[tauri::command]
pub fn notebook_snapshot(state: State<'_, AppState>) -> CommandResult<NotebookSnapshot> {
    state.with_notebook(|nb| {
        Ok(NotebookSnapshot {
            info: NotebookInfo::of(nb)?,
            clock: clock_of(nb),
            counts: if nb.config().show_list_counts {
                nb.open_task_counts()?
            } else {
                Default::default()
            },
            conflicts: nb.conflicts()?,
            workspaces: workspaces_of(nb)?,
        })
    })
}

/// Suggestions with the reason each one is being offered, so the UI can group
/// them without re-deriving the rule.
#[tauri::command]
pub fn grouped_suggestions(
    state: State<'_, AppState>,
    period: Period,
) -> CommandResult<Vec<memo_core::notebook::Suggestion>> {
    state.with_notebook(|nb| Ok(nb.grouped_suggestions(period)?))
}
