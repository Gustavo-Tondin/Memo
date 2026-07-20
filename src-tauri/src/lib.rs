//! Tauri shell — thin layer only.
//!
//! No business logic here: every command delegates to `memo_core`. The shell
//! owns exactly two things the core cannot: which notebook is open right now,
//! and the bridge to the frontend (commands in, events out).

pub mod commands;
pub mod error;
pub mod prefs;
pub mod state;

use state::AppState;

/// Applies the app's configuration to a builder.
///
/// Split out so the integration tests drive the *same* set of commands the
/// real app registers — a command that exists only in production is a
/// command nothing tests.
pub fn configure<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::core_version,
            commands::is_notebook_open,
            // notebook
            commands::pick_notebook_folder,
            commands::open_notebook,
            commands::current_notebook,
            commands::last_notebook,
            commands::notebook_settings,
            commands::set_notebook_settings,
            commands::screen_to_restore,
            commands::remember_screen,
            commands::list_counts,
            // lists
            commands::list_names,
            commands::list_conflicts,
            commands::list_tasks,
            commands::create_list,
            commands::rename_list,
            commands::delete_list,
            // tasks
            commands::create_task,
            commands::edit_task_text,
            commands::complete_task,
            commands::uncomplete_task,
            // day and week
            commands::period_state,
            commands::period_tasks,
            commands::period_suggestions,
            commands::period_clock,
            commands::pull_into_period,
            commands::remove_from_period,
            commands::add_task_in_period,
            commands::refresh_periods,
        ])
}

pub fn run() {
    configure(tauri::Builder::default())
        .run(tauri::generate_context!())
        .expect("error while running Memo");
}
