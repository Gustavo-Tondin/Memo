//! One `invoke()` command per core operation.
//!
//! Every function here is a wire: read the arguments, call `memo_core`, hand
//! back the result. Any `if` that decides something about tasks, lists or
//! dates belongs in the core instead — see the architecture rule in
//! `CLAUDE.md`.

use std::path::PathBuf;

use memo_core::config::{Config, RolloverMode};
use memo_core::state::{Period, PeriodState};
use memo_core::{ListedTask, Notebook, Task, TurnOffset, WeekStart};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::DialogExt;

use crate::error::CommandResult;
use crate::state::AppState;

/// What the frontend needs to know about the open notebook.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookInfo {
    pub path: PathBuf,
    /// Folder name, which is what the user recognizes as the notebook's name.
    pub name: String,
    pub read_only: bool,
    pub lists: Vec<String>,
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
            lists: notebook.list_names()?,
        })
    }
}

/// The notebook preferences, flattened for the UI.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolloverSettings {
    pub daily_mode: String,
    pub daily_at: String,
    pub weekly_mode: String,
    pub weekly_at: String,
    pub week_starts_on: String,
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

#[tauri::command]
pub fn notebook_settings(state: State<'_, AppState>) -> CommandResult<RolloverSettings> {
    state.with_notebook(|nb| {
        let rollover = nb.config().rollover;
        Ok(RolloverSettings {
            daily_mode: rollover.daily.mode.render().to_string(),
            daily_at: rollover.daily.at.render(),
            weekly_mode: rollover.weekly.mode.render().to_string(),
            weekly_at: rollover.weekly.at.render(),
            week_starts_on: rollover.weekly.starts_on.render().to_string(),
        })
    })
}

/// Saves the rollover preferences. Unparseable values fall back to the
/// defaults in the core, so the UI cannot write a broken config.
#[tauri::command]
pub fn set_notebook_settings(
    state: State<'_, AppState>,
    settings: RolloverSettings,
) -> CommandResult<()> {
    state.with_notebook_mut(|nb| {
        let mut config: Config = nb.config().clone();
        config.rollover.daily.mode = RolloverMode::parse_or_default(&settings.daily_mode);
        config.rollover.daily.at = TurnOffset::parse_or_default(&settings.daily_at);
        config.rollover.weekly.mode = RolloverMode::parse_or_default(&settings.weekly_mode);
        config.rollover.weekly.at = TurnOffset::parse_or_default(&settings.weekly_at);
        config.rollover.weekly.starts_on = WeekStart::parse_or_default(&settings.week_starts_on);
        nb.set_config(config)?;
        Ok(())
    })
}

// ------------------------------------------------------------------ lists

#[tauri::command]
pub fn list_names(state: State<'_, AppState>) -> CommandResult<Vec<String>> {
    state.with_notebook(|nb| Ok(nb.list_names()?))
}

#[tauri::command]
pub fn list_tasks(state: State<'_, AppState>, list: String) -> CommandResult<Vec<Task>> {
    state.with_notebook(|nb| Ok(nb.tasks_in(&list)?))
}

#[tauri::command]
pub fn create_list(state: State<'_, AppState>, name: String) -> CommandResult<()> {
    state.with_notebook(|nb| {
        nb.create_list(&name)?;
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

#[tauri::command]
pub fn create_task(
    state: State<'_, AppState>,
    list: String,
    text: String,
) -> CommandResult<String> {
    state.with_notebook(|nb| {
        let mut tasks = nb.open_list(&list)?;
        let id = tasks.add_text(text);
        tasks.save()?;
        Ok(id)
    })
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

#[tauri::command]
pub fn complete_task(
    state: State<'_, AppState>,
    list: String,
    id: String,
) -> CommandResult<Task> {
    state.with_notebook(|nb| Ok(nb.complete_task(&list, &id)?))
}

#[tauri::command]
pub fn uncomplete_task(state: State<'_, AppState>, id: String) -> CommandResult<Task> {
    state.with_notebook(|nb| Ok(nb.uncomplete_task(&id)?))
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

#[tauri::command]
pub fn period_clock(state: State<'_, AppState>) -> CommandResult<PeriodClock> {
    state.with_notebook(|nb| {
        Ok(PeriodClock {
            today: nb.today().to_string(),
            week_start: nb.current_week().to_string(),
            next_daily_turn: nb.next_turn_at(Period::Day).to_rfc3339(),
            next_weekly_turn: nb.next_turn_at(Period::Week).to_rfc3339(),
        })
    })
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
