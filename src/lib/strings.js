// Every string the user reads, in one place.
//
// Spec 4.4: the interface is English until i18n lands after v1, and the
// strings are born centralized — translating later means adding a file here,
// not hunting text through components. Functions take the variable parts.

export const S = {
  // App shell
  onboardingIntro:
    "Choose a folder to be your notebook. If it is not one yet, Memo creates " +
    "the structure inside it — your files stay plain .md, readable in any editor.",
  chooseFolder: "Choose notebook folder…",
  switchNotebook: "switch notebook…",
  newListButton: "+ new list",
  today: "Today",
  week: "Week",
  completed: "Completed",
  readOnly: "read-only",
  renameList: "rename list",
  deleteList: "delete list",
  promptNewList: "Name of the new list:",
  promptRenameList: (name) => `New name for "${name}":`,
  confirmDeleteList: (name) =>
    `Delete "${name}"? Remaining tasks go to the Inbox.`,
  tasksRescued: (count, name) =>
    `${count} task(s) from "${name}" were moved to the Inbox.`,
  conflictsTitle: (count) => `${count} sync conflict(s) in this notebook`,
  conflictsBody:
    "Another device edited the same files. Memo does not choose for you — " +
    "open the folder and decide which version stays.",
  dismissError: "ok",

  // Workspaces (phase 7.5)
  workspacesTitle: "Workspaces",
  readOnlyWorkspace: "read-only (newer version)",
  emptyWorkspace: "This workspace has no widgets yet.",
  unsupportedWidgetTitle: (kind) =>
    kind ? `"${kind}" widget` : "Widget without a type",
  unsupportedWidgetBody:
    "This version of Memo does not know how to show this widget. " +
    "Its files are untouched — a newer version may support it.",
  invalidWidgetFolder:
    "This widget's folder points outside the workspace, so it was not loaded.",
  widgetNoLists: "No lists in this widget yet.",

  // Notes (phase 8)
  notes: "Notes",
  newNote: "+ new note",
  newNoteTitle: "New note",
  promptNewNote: "Title of the new note:",
  promptRenameNote: (title) => `New title for "${title}":`,
  promptNewNoteFolder: "Name of the new folder:",
  newNoteFolder: "+ new folder",
  confirmDeleteNote: (title) => `Delete "${title}"? This cannot be undone.`,
  searchNotes: "Search notes…",
  noNotes: "No notes yet.",
  noNotesFound: "No notes match this search.",
  allNotes: "All notes",
  emptyNote: "Empty note",
  pin: "pin",
  unpin: "unpin",
  pinned: "pinned",
  deleteNote: "delete",
  renameNote: "rename",
  moveNote: "move to…",
  promptMoveNote: (folders) =>
    `Move to which folder?\n\nAvailable: ${folders || "(root)"}`,
  backToNotes: "← notes",
  noteBodyPlaceholder: "Write here…",
  gridView: "grid",
  treeView: "folders",

  // Lists
  newTaskPlaceholder: "New task…",
  addTask: "Add",
  emptyList: "No tasks in this list.",
  pullToToday: "→ Today",
  pullToWeek: "→ Week",

  // Today / Week
  weekTitle: "This week",
  weekOf: (start) => `week of ${start}`,
  newTaskToInboxPlaceholder: "New task (goes to the Inbox)…",
  nothingPulled: "Nothing chosen yet. Pull something from the suggestions below.",
  suggestionsTitle: "Suggestions",
  noSuggestions: "No tasks available.",
  groupUrgent: "Urgent",
  groupSoon: "Soon",
  groupThisWeek: "This week",
  groupLists: "From the lists",
  pull: "pull",
  removeFromPeriod: "remove",

  // Completed
  nothingCompleted: "Nothing completed yet.",
  goesBackTo: (name) => `back to ${name}`,

  // Task row and inspector
  complete: "complete",
  uncheck: "uncheck",
  taskRowHint: "click to open, double-click to rename",
  repeatsHint: "repeats",
  taskName: "task name",
  closePanel: "close",
  subtasksTitle: "Subtasks",
  subtaskLabel: (text) => `subtask: ${text}`,
  newSubtaskPlaceholder: "New subtask…",
  removeSubtask: "remove subtask",
  tagsTitle: "Tags",
  newTagPlaceholder: "New tag…",
  removeTag: (tag) => `remove #${tag}`,
  descriptionTitle: "Description",
  dueDateLabel: "Due date",
  clearDate: "clear date",
  clearDateHint: "clear",
  priorityLabel: "Priority",
  priorityNone: "none",
  priorityHigh: "!1 high",
  priorityMedium: "!2 medium",
  priorityLow: "!3 low",
  repeatLabel: "Repeat",
  repeatEvery: "every",
  noRepeat: "does not repeat",
  repeatDays: "day(s)",
  repeatWeeks: "week(s)",
  repeatMonths: "month(s)",
};
