// Every call into Rust goes through here.
//
// One place to see the whole surface, and one place to fix when a command
// changes. Nothing in this file decides anything — it just names the bridge.

import { invoke } from "@tauri-apps/api/core";

export const api = {
  // notebook
  // Everything the shell needs after any change, in one round trip —
  // info (with the layout names), clock, counts and conflicts.
  notebookSnapshot: () => invoke("notebook_snapshot"),
  lastNotebook: () => invoke("last_notebook"),
  pickFolder: () => invoke("pick_notebook_folder"),
  openNotebook: (path) => invoke("open_notebook", { path }),
  currentNotebook: () => invoke("current_notebook"),
  screenToRestore: () => invoke("screen_to_restore"),
  rememberScreen: (screen) => invoke("remember_screen", { screen }),

  // lists
  listNames: () => invoke("list_names"),
  listCounts: () => invoke("list_counts"),
  listConflicts: () => invoke("list_conflicts"),
  listTasks: (list) => invoke("list_tasks", { list }),
  // `folder` is a workspace folder ("Tasks"); the UI takes it from
  // layout.tasksFolder until it is workspace-aware.
  createList: (folder, name) => invoke("create_list", { folder, name }),
  renameList: (from, to) => invoke("rename_list", { from, to }),
  deleteList: (name) => invoke("delete_list", { name }),

  // tasks
  createTask: (list, text) => invoke("create_task", { list, text }),
  editTaskText: (list, id, text) => invoke("edit_task_text", { list, id, text }),
  // Every field at once. Absent means "leave alone", null means "clear" —
  // see `TaskFields` in commands.rs.
  setTaskFields: (list, id, fields) =>
    invoke("set_task_fields", { list, id, fields }),
  moveTaskTo: (list, from, to) => invoke("move_task_to", { list, from, to }),
  ensureTaskId: (list, position) => invoke("ensure_task_id", { list, position }),
  completeTask: (list, id) => invoke("complete_task", { list, id }),
  // `list` is the Completed list the task sits in — one per widget.
  uncompleteTask: (list, id) => invoke("uncomplete_task", { list, id }),

  // day and week
  periodTasks: (period) => invoke("period_tasks", { period }),
  periodSuggestions: (period) => invoke("period_suggestions", { period }),
  groupedSuggestions: (period) => invoke("grouped_suggestions", { period }),
  pullInto: (period, list, id) => invoke("pull_into_period", { period, list, id }),
  removeFrom: (period, list, id) =>
    invoke("remove_from_period", { period, list, id }),
  addTaskInPeriod: (period, text) => invoke("add_task_in_period", { period, text }),
  periodClock: () => invoke("period_clock"),
  refreshPeriods: () => invoke("refresh_periods"),
};

/// Errors cross the bridge as { kind, message }; anything else is a bug.
export function describeError(error) {
  if (error && typeof error === "object" && "kind" in error) {
    return `${error.kind}: ${error.message}`;
  }
  return String(error);
}
