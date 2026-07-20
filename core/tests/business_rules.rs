//! Phase 2 business rules: completing, undoing, list management, and the
//! day/week states.
//!
//! Same standard as the other integration tests — assertions look at the
//! files on disk, because the files are the product.

use std::path::Path;

use memo_core::config::{Config, RolloverMode};
use memo_core::state::Period;
use memo_core::{Error, Notebook, TaskList};

fn read(path: impl AsRef<Path>) -> String {
    std::fs::read_to_string(path).unwrap()
}

/// A notebook with one task in the Inbox, returned with its id.
fn notebook_with_task(dir: &Path, text: &str) -> (Notebook, String) {
    let notebook = Notebook::init(dir).unwrap();
    let mut inbox = notebook.inbox().unwrap();
    let id = inbox.add_text(text);
    inbox.save().unwrap();
    (notebook, id)
}

// ------------------------------------------------------------- completing

#[test]
fn completing_moves_the_task_to_completas_with_its_origin() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();
    notebook.create_list("Compras").unwrap();

    let mut compras = notebook.open_list("Compras").unwrap();
    let id = compras.add_text("Comprar leite");
    compras.save().unwrap();

    let task = notebook.complete_task("Compras", &id).unwrap();
    assert!(task.done);
    assert_eq!(task.origin.as_deref(), Some("Compras"));

    let completed = read(dir.path().join("Tarefas/Completas.md"));
    assert!(completed.contains("- [x] Comprar leite"));
    assert!(completed.contains(&format!("id:{id}")));
    assert!(completed.contains("origin:Compras"));

    // And it really left the source file.
    assert!(!read(dir.path().join("Tarefas/Compras.md")).contains("Comprar leite"));
}

#[test]
fn completing_drops_the_task_from_today_and_this_week() {
    let dir = tempfile::tempdir().unwrap();
    let (notebook, id) = notebook_with_task(dir.path(), "Ligar pro dentista");

    notebook.pull_into(Period::Day, "Inbox", &id).unwrap();
    notebook.pull_into(Period::Week, "Inbox", &id).unwrap();

    notebook.complete_task("Inbox", &id).unwrap();

    // A reference left behind would render as a ghost row in Today.
    assert!(notebook.open_state(Period::Day).unwrap().state.is_empty());
    assert!(notebook.open_state(Period::Week).unwrap().state.is_empty());
}

#[test]
fn undoing_sends_the_task_back_to_its_origin_list() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();
    notebook.create_list("Compras").unwrap();

    let mut compras = notebook.open_list("Compras").unwrap();
    let id = compras.add_text("Comprar leite");
    compras.save().unwrap();

    notebook.complete_task("Compras", &id).unwrap();
    let task = notebook.uncomplete_task(&id).unwrap();

    assert!(!task.done);
    assert_eq!(task.origin, None, "origin is consumed by the undo");

    let compras = read(dir.path().join("Tarefas/Compras.md"));
    assert!(compras.contains("- [ ] Comprar leite"));
    assert!(!read(dir.path().join("Tarefas/Completas.md")).contains("Comprar leite"));
}

#[test]
fn undoing_recreates_an_origin_list_that_was_deleted_outside_the_app() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();
    notebook.create_list("Compras").unwrap();

    let mut compras = notebook.open_list("Compras").unwrap();
    let id = compras.add_text("Comprar leite");
    compras.save().unwrap();
    notebook.complete_task("Compras", &id).unwrap();

    // The user deletes the list in the file manager while the task sits in
    // Completas.
    std::fs::remove_file(dir.path().join("Tarefas/Compras.md")).unwrap();

    notebook.uncomplete_task(&id).unwrap();
    assert!(read(dir.path().join("Tarefas/Compras.md")).contains("Comprar leite"));
}

#[test]
fn undoing_a_task_without_a_usable_origin_falls_back_to_the_inbox() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();

    // Written by hand in Obsidian: done, with an id, but no origin.
    std::fs::write(
        dir.path().join("Tarefas/Completas.md"),
        "- [x] Pagar internet <!--id:abc123-->\n",
    )
    .unwrap();

    notebook.uncomplete_task("abc123").unwrap();
    assert!(read(dir.path().join("Tarefas/Inbox.md")).contains("Pagar internet"));
}

#[test]
fn undoing_an_unknown_id_fails_without_touching_anything() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();

    let err = notebook.uncomplete_task("nope").unwrap_err();
    assert!(matches!(err, Error::TaskNotFound(_)));
}

// ----------------------------------------------------------------- reading

#[test]
fn reading_a_list_adopts_checkboxes_written_by_hand() {
    // Someone adds a line in Obsidian; without an id the app could not act on
    // it, so reading the list is what adopts it.
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();
    std::fs::write(
        dir.path().join("Tarefas/Inbox.md"),
        "# Minha lista\n\n- [ ] escrita no Obsidian\n",
    )
    .unwrap();

    let tasks = notebook.tasks_in("Inbox").unwrap();
    assert_eq!(tasks.len(), 1);
    let id = tasks[0].id.clone().expect("id assigned on read");

    // Persisted, and the heading the user wrote is still there.
    let on_disk = read(dir.path().join("Tarefas/Inbox.md"));
    assert!(on_disk.contains(&format!("id:{id}")));
    assert!(on_disk.contains("# Minha lista"));

    // And the task is now actionable.
    notebook.complete_task("Inbox", &id).unwrap();
}

#[test]
fn reading_a_read_only_notebook_does_not_adopt_ids() {
    let dir = tempfile::tempdir().unwrap();
    Notebook::init(dir.path()).unwrap();
    std::fs::write(
        dir.path().join("Tarefas/Inbox.md"),
        "- [ ] sem id\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join(".memo/config.json"),
        r#"{ "schemaVersion": 99 }"#,
    )
    .unwrap();

    let notebook = Notebook::open(dir.path()).unwrap();
    let tasks = notebook.tasks_in("Inbox").unwrap();

    assert_eq!(tasks.len(), 1);
    assert!(tasks[0].id.is_none(), "read-only must not write ids");
    assert_eq!(read(dir.path().join("Tarefas/Inbox.md")), "- [ ] sem id\n");
}

// ------------------------------------------------------------------ lists

#[test]
fn renaming_a_list_repoints_completed_origins_and_states() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();
    notebook.create_list("Compras").unwrap();

    let mut compras = notebook.open_list("Compras").unwrap();
    let done_id = compras.add_text("Comprar leite");
    let pulled_id = compras.add_text("Comprar pão");
    compras.save().unwrap();

    notebook.complete_task("Compras", &done_id).unwrap();
    notebook.pull_into(Period::Day, "Compras", &pulled_id).unwrap();

    notebook.rename_list("Compras", "Mercado").unwrap();

    assert!(dir.path().join("Tarefas/Mercado.md").is_file());
    assert!(!dir.path().join("Tarefas/Compras.md").exists());
    assert!(read(dir.path().join("Tarefas/Completas.md")).contains("origin:Mercado"));

    let state = notebook.open_state(Period::Day).unwrap();
    assert!(state.state.contains("Mercado", &pulled_id));

    // The undo still works, which is the whole point of repointing origins.
    notebook.uncomplete_task(&done_id).unwrap();
    assert!(read(dir.path().join("Tarefas/Mercado.md")).contains("Comprar leite"));
}

#[test]
fn default_lists_cannot_be_renamed_or_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();

    for name in ["Inbox", "Completas"] {
        assert!(matches!(
            notebook.rename_list(name, "Outra").unwrap_err(),
            Error::ProtectedList(_)
        ));
        assert!(matches!(
            notebook.delete_list(name).unwrap_err(),
            Error::ProtectedList(_)
        ));
    }
}

#[test]
fn renaming_onto_an_existing_list_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();
    notebook.create_list("Compras").unwrap();
    notebook.create_list("Mercado").unwrap();

    assert!(notebook.rename_list("Compras", "Mercado").is_err());
    // Neither file was harmed.
    assert!(dir.path().join("Tarefas/Compras.md").is_file());
    assert!(dir.path().join("Tarefas/Mercado.md").is_file());
}

#[test]
fn deleting_a_list_rescues_its_tasks_into_the_inbox() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();
    notebook.create_list("Compras").unwrap();

    let mut compras = notebook.open_list("Compras").unwrap();
    let id = compras.add_text("Comprar leite");
    compras.add_text("Comprar pão");
    compras.save().unwrap();
    notebook.pull_into(Period::Day, "Compras", &id).unwrap();

    let rescued = notebook.delete_list("Compras").unwrap();

    assert_eq!(rescued, 2);
    assert!(!dir.path().join("Tarefas/Compras.md").exists());

    let inbox = read(dir.path().join("Tarefas/Inbox.md"));
    assert!(inbox.contains("Comprar leite"));
    assert!(inbox.contains("Comprar pão"));

    // A task that was pulled into Today stays pulled, now via the Inbox.
    let state = notebook.open_state(Period::Day).unwrap();
    assert!(state.state.contains("Inbox", &id));
}

#[test]
fn path_traversal_is_still_refused_by_the_new_operations() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();

    for evil in ["../escape", "sub/dir", ".hidden", ""] {
        assert!(notebook.rename_list(evil, "Ok").is_err(), "{evil:?}");
        assert!(notebook.delete_list(evil).is_err(), "{evil:?}");
    }
}

// ------------------------------------------------------------ day and week

#[test]
fn pulling_a_task_writes_a_reference_not_a_copy() {
    let dir = tempfile::tempdir().unwrap();
    let (notebook, id) = notebook_with_task(dir.path(), "Comprar leite");

    assert!(notebook.pull_into(Period::Week, "Inbox", &id).unwrap());

    let state = read(dir.path().join(".memo/weekly-state.json"));
    assert!(state.contains(&id));
    // The text must live in exactly one place: the list file.
    assert!(!state.contains("Comprar leite"));
}

#[test]
fn pulling_the_same_task_twice_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let (notebook, id) = notebook_with_task(dir.path(), "Comprar leite");

    assert!(notebook.pull_into(Period::Day, "Inbox", &id).unwrap());
    assert!(!notebook.pull_into(Period::Day, "Inbox", &id).unwrap());
    assert_eq!(notebook.open_state(Period::Day).unwrap().state.len(), 1);
}

#[test]
fn pulling_a_task_that_does_not_exist_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();

    let err = notebook.pull_into(Period::Day, "Inbox", "ghost").unwrap_err();
    assert!(matches!(err, Error::TaskNotFound(_)));
    assert!(!dir.path().join(".memo/daily-state.json").exists());
}

#[test]
fn removing_from_a_period_leaves_the_task_alone() {
    let dir = tempfile::tempdir().unwrap();
    let (notebook, id) = notebook_with_task(dir.path(), "Comprar leite");

    notebook.pull_into(Period::Day, "Inbox", &id).unwrap();
    assert!(notebook.remove_from(Period::Day, "Inbox", &id).unwrap());

    assert!(notebook.open_state(Period::Day).unwrap().state.is_empty());
    assert!(read(dir.path().join("Tarefas/Inbox.md")).contains("Comprar leite"));
}

#[test]
fn a_task_created_in_today_is_physically_written_to_the_inbox() {
    // Spec 3: Day and Week never store content of their own.
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();

    let id = notebook
        .add_task_in_period(Period::Day, "Responder e-mail")
        .unwrap();

    let inbox = read(dir.path().join("Tarefas/Inbox.md"));
    assert!(inbox.contains("- [ ] Responder e-mail"));
    assert!(inbox.contains(&format!("id:{id}")));

    let state = notebook.open_state(Period::Day).unwrap();
    assert!(state.state.contains("Inbox", &id));
}

#[test]
fn the_state_rolls_over_when_the_notebook_is_reopened_later() {
    let dir = tempfile::tempdir().unwrap();
    let (notebook, id) = notebook_with_task(dir.path(), "Comprar leite");
    notebook.pull_into(Period::Day, "Inbox", &id).unwrap();

    // Simulate the app having been closed since an old day, by rewriting the
    // state's date the way it would look on disk.
    let path = dir.path().join(".memo/daily-state.json");
    let mut state: serde_json::Value =
        serde_json::from_str(&read(&path)).unwrap();
    state["date"] = serde_json::json!("2020-01-01");
    std::fs::write(&path, state.to_string()).unwrap();

    let reopened = Notebook::open(dir.path()).unwrap();
    let rolled = reopened.open_state(Period::Day).unwrap();

    // Default mode is reset: the day starts empty...
    assert!(rolled.state.is_empty());
    assert_eq!(rolled.state.date, reopened.today());
    // ...and the task itself is untouched, back to being a suggestion.
    assert!(read(dir.path().join("Tarefas/Inbox.md")).contains("Comprar leite"));
}

#[test]
fn carry_mode_keeps_the_pulled_tasks_across_the_turn() {
    let dir = tempfile::tempdir().unwrap();
    let (mut notebook, id) = notebook_with_task(dir.path(), "Comprar leite");

    let mut config = Config::default();
    config.rollover.daily.mode = RolloverMode::Carry;
    notebook.set_config(config).unwrap();

    notebook.pull_into(Period::Day, "Inbox", &id).unwrap();

    let path = dir.path().join(".memo/daily-state.json");
    let mut state: serde_json::Value = serde_json::from_str(&read(&path)).unwrap();
    state["date"] = serde_json::json!("2020-01-01");
    std::fs::write(&path, state.to_string()).unwrap();

    let reopened = Notebook::open(dir.path()).unwrap();
    let rolled = reopened.open_state(Period::Day).unwrap();

    assert!(rolled.state.contains("Inbox", &id));
    assert_eq!(rolled.state.date, reopened.today());
}

// ------------------------------------------------------------- sugestões

#[test]
fn the_day_suggests_the_week_first_then_the_other_lists() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();
    notebook.create_list("Compras").unwrap();

    let mut inbox = notebook.inbox().unwrap();
    let solta = inbox.add_text("Tarefa solta");
    inbox.save().unwrap();

    let mut compras = notebook.open_list("Compras").unwrap();
    let da_semana = compras.add_text("Escolhida pra semana");
    compras.save().unwrap();

    notebook
        .pull_into(Period::Week, "Compras", &da_semana)
        .unwrap();

    let suggestions = notebook.suggestions_for(Period::Day).unwrap();
    let ids: Vec<_> = suggestions
        .iter()
        .map(|s| s.task.id.clone().unwrap())
        .collect();

    // What the user already chose for the week comes first.
    assert_eq!(ids, vec![da_semana.clone(), solta]);
    assert_eq!(suggestions[0].list, "Compras");
}

#[test]
fn a_task_already_pulled_is_not_suggested_again() {
    let dir = tempfile::tempdir().unwrap();
    let (notebook, id) = notebook_with_task(dir.path(), "Comprar leite");

    assert_eq!(notebook.suggestions_for(Period::Day).unwrap().len(), 1);
    notebook.pull_into(Period::Day, "Inbox", &id).unwrap();
    assert!(notebook.suggestions_for(Period::Day).unwrap().is_empty());
}

#[test]
fn completed_tasks_are_never_suggested() {
    let dir = tempfile::tempdir().unwrap();
    let (notebook, id) = notebook_with_task(dir.path(), "Comprar leite");

    notebook.complete_task("Inbox", &id).unwrap();

    assert!(notebook.suggestions_for(Period::Day).unwrap().is_empty());
    assert!(notebook.suggestions_for(Period::Week).unwrap().is_empty());
}

#[test]
fn the_week_suggests_from_the_lists_only() {
    // The week is not fed by the day — pulling into today does not remove a
    // task from the week's suggestions.
    let dir = tempfile::tempdir().unwrap();
    let (notebook, id) = notebook_with_task(dir.path(), "Comprar leite");

    notebook.pull_into(Period::Day, "Inbox", &id).unwrap();

    let week = notebook.suggestions_for(Period::Week).unwrap();
    assert_eq!(week.len(), 1);
    assert_eq!(week[0].task.id.as_deref(), Some(id.as_str()));
}

#[test]
fn period_tasks_resolves_references_to_real_tasks() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();
    notebook.create_list("Compras").unwrap();

    let mut compras = notebook.open_list("Compras").unwrap();
    let id = compras.add_text("Comprar leite");
    compras.save().unwrap();
    notebook.pull_into(Period::Day, "Compras", &id).unwrap();

    let pulled = notebook.period_tasks(Period::Day).unwrap();
    assert_eq!(pulled.len(), 1);
    assert_eq!(pulled[0].list, "Compras");
    assert_eq!(pulled[0].task.text, "Comprar leite");
}

#[test]
fn a_reference_to_a_task_deleted_elsewhere_is_skipped() {
    // The notebook is shared with other editors; a stale reference is normal.
    let dir = tempfile::tempdir().unwrap();
    let (notebook, id) = notebook_with_task(dir.path(), "Comprar leite");
    notebook.pull_into(Period::Day, "Inbox", &id).unwrap();

    // Someone deletes the line in Obsidian.
    std::fs::write(dir.path().join("Tarefas/Inbox.md"), "").unwrap();

    assert!(notebook.period_tasks(Period::Day).unwrap().is_empty());
}

// -------------------------------------------------------------- read-only

#[test]
fn a_notebook_from_a_newer_app_refuses_every_write() {
    let dir = tempfile::tempdir().unwrap();
    let (notebook, id) = notebook_with_task(dir.path(), "Comprar leite");
    drop(notebook);

    std::fs::write(
        dir.path().join(".memo/config.json"),
        r#"{ "schemaVersion": 99, "somethingNew": true }"#,
    )
    .unwrap();

    let notebook = Notebook::open(dir.path()).unwrap();
    assert!(notebook.is_read_only());

    // Reading still works — the user can see their tasks.
    assert_eq!(notebook.inbox().unwrap().tasks().count(), 1);

    // Writing does not, so a newer app's fields are never destroyed.
    assert!(notebook.complete_task("Inbox", &id).is_err());
    assert!(notebook.create_list("Compras").is_err());
    assert!(notebook.pull_into(Period::Day, "Inbox", &id).is_err());
    assert!(notebook.add_task_in_period(Period::Day, "nova").is_err());
    assert!(notebook.delete_list("Compras").is_err());

    // And the unknown key is still on disk, untouched.
    assert!(read(dir.path().join(".memo/config.json")).contains("somethingNew"));
}

// ------------------------------------------------------- full phase-2 flow

#[test]
fn the_whole_phase_two_scenario_end_to_end() {
    // Roadmap's exit criterion: create → pull into the week → pull into the
    // day → complete → undo, checked against the files.
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();
    notebook.create_list("Compras").unwrap();

    let mut compras = notebook.open_list("Compras").unwrap();
    let id = compras.add_text("Comprar leite");
    compras.save().unwrap();

    notebook.pull_into(Period::Week, "Compras", &id).unwrap();
    notebook.pull_into(Period::Day, "Compras", &id).unwrap();

    assert!(notebook
        .open_state(Period::Week)
        .unwrap()
        .state
        .contains("Compras", &id));
    assert!(notebook
        .open_state(Period::Day)
        .unwrap()
        .state
        .contains("Compras", &id));

    notebook.complete_task("Compras", &id).unwrap();

    assert!(read(dir.path().join("Tarefas/Completas.md")).contains("- [x] Comprar leite"));
    assert!(notebook.open_state(Period::Day).unwrap().state.is_empty());
    assert!(notebook.open_state(Period::Week).unwrap().state.is_empty());

    notebook.uncomplete_task(&id).unwrap();

    let compras = TaskList::load(dir.path().join("Tarefas/Compras.md")).unwrap();
    let task = compras.find(&id).unwrap();
    assert_eq!(task.text, "Comprar leite");
    assert!(!task.done);
    assert!(read(dir.path().join("Tarefas/Completas.md")).trim().is_empty());
}
