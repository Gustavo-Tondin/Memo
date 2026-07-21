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
