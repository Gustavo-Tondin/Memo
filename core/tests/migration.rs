//! Migrating notebooks created before the app settled on English names.
//!
//! This rename shipped when only the author's test notebook existed, but the
//! same code path runs on every user's notebook when they update. Breaking one
//! is never acceptable, so the interesting cases here are the awkward ones:
//! half-migrated folders, destinations that already exist, and read-only
//! notebooks.

use std::path::Path;

use memo_core::state::Period;
use memo_core::Notebook;

fn read(path: impl AsRef<Path>) -> String {
    std::fs::read_to_string(path).unwrap()
}

/// Builds a notebook the way version 0.4 wrote it, without going through the
/// current code — otherwise the test would migrate on the way in.
fn legacy_notebook(dir: &Path) {
    std::fs::create_dir_all(dir.join(".memo")).unwrap();
    std::fs::create_dir_all(dir.join("Tarefas")).unwrap();
    std::fs::create_dir_all(dir.join("Notas")).unwrap();
    std::fs::write(dir.join(".memo/config.json"), "{\n  \"schemaVersion\": 1\n}\n").unwrap();
    std::fs::write(
        dir.join("Tarefas/Inbox.md"),
        "- [ ] Tarefa antiga <!--id:abc123-->\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("Tarefas/Completas.md"),
        "- [x] Concluída antiga <!--id:d4e5f6 origin:Compras-->\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("Tarefas/Compras.md"),
        "- [ ] Comprar leite <!--id:g7h8i9-->\n",
    )
    .unwrap();
}

#[test]
fn an_old_notebook_is_migrated_on_open() {
    let dir = tempfile::tempdir().unwrap();
    legacy_notebook(dir.path());

    let notebook = Notebook::open(dir.path()).unwrap();

    assert!(dir.path().join("Tasks").is_dir());
    assert!(dir.path().join("Notes").is_dir());
    assert!(!dir.path().join("Tarefas").exists());
    assert!(!dir.path().join("Notas").exists());
    assert!(dir.path().join("Tasks/Completed.md").is_file());
    assert!(!dir.path().join("Tasks/Completas.md").exists());

    // Content survived the move, byte for byte.
    assert!(read(dir.path().join("Tasks/Inbox.md")).contains("Tarefa antiga"));
    assert!(read(dir.path().join("Tasks/Compras.md")).contains("Comprar leite"));
    assert!(read(dir.path().join("Tasks/Completed.md")).contains("Concluída antiga"));

    // And the notebook works normally afterwards.
    let lists = notebook.list_names().unwrap();
    assert_eq!(lists, vec!["Completed", "Compras", "Inbox"]);
    notebook.uncomplete_task("d4e5f6").unwrap();
    assert!(read(dir.path().join("Tasks/Compras.md")).contains("Concluída antiga"));
}

#[test]
fn migrating_twice_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    legacy_notebook(dir.path());

    Notebook::open(dir.path()).unwrap();
    let after_first = read(dir.path().join("Tasks/Inbox.md"));
    Notebook::open(dir.path()).unwrap();

    assert_eq!(read(dir.path().join("Tasks/Inbox.md")), after_first);
    assert!(dir.path().join("Tasks").is_dir());
}

#[test]
fn a_notebook_already_in_english_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();
    let mut inbox = notebook.inbox().unwrap();
    inbox.add_text("Tarefa nova");
    inbox.save().unwrap();

    let before = read(dir.path().join("Tasks/Inbox.md"));
    Notebook::open(dir.path()).unwrap();

    assert_eq!(read(dir.path().join("Tasks/Inbox.md")), before);
    assert!(!dir.path().join("Tarefas").exists());
}

#[test]
fn a_notebook_with_both_layouts_is_not_merged() {
    // Two folders means something unusual happened — a sync that resurrected
    // the old one, a half-restored backup. Merging blind could overwrite the
    // user's data, so the old folder is left untouched for them to sort out.
    let dir = tempfile::tempdir().unwrap();
    legacy_notebook(dir.path());
    std::fs::create_dir_all(dir.path().join("Tasks")).unwrap();
    std::fs::write(
        dir.path().join("Tasks/Inbox.md"),
        "- [ ] Tarefa nova <!--id:novo01-->\n",
    )
    .unwrap();

    Notebook::open(dir.path()).unwrap();

    assert!(dir.path().join("Tarefas").is_dir(), "old folder kept");
    assert!(read(dir.path().join("Tasks/Inbox.md")).contains("Tarefa nova"));
    assert!(
        read(dir.path().join("Tarefas/Inbox.md")).contains("Tarefa antiga"),
        "nothing in the old folder may be lost"
    );
}

#[test]
fn a_completed_file_is_not_overwritten_by_the_rename() {
    let dir = tempfile::tempdir().unwrap();
    legacy_notebook(dir.path());
    std::fs::write(
        dir.path().join("Tarefas/Completed.md"),
        "- [x] Já em inglês <!--id:eng001-->\n",
    )
    .unwrap();

    Notebook::open(dir.path()).unwrap();

    let completed = read(dir.path().join("Tasks/Completed.md"));
    assert!(completed.contains("Já em inglês"));
    assert!(
        dir.path().join("Tasks/Completas.md").is_file(),
        "the old file stays instead of being silently dropped"
    );
}

#[test]
fn hand_written_references_to_the_old_completed_list_are_repointed() {
    // Only reachable if someone edited the state or an origin by hand, but
    // the repoint is cheap and the alternative is a dangling reference.
    let dir = tempfile::tempdir().unwrap();
    legacy_notebook(dir.path());
    std::fs::write(
        dir.path().join(".memo/daily-state.json"),
        r#"{"date":"2026-07-20","items":[{"list":"Completas","id":"d4e5f6"}]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("Tarefas/Compras.md"),
        "- [ ] Voltou da lista <!--id:z9y8x7 origin:Completas-->\n",
    )
    .unwrap();

    let notebook = Notebook::open(dir.path()).unwrap();

    let state = notebook.open_state(Period::Day).unwrap().state;
    assert!(
        state.items.iter().all(|r| r.list != "Completas"),
        "no reference may keep pointing at the old name"
    );
    assert!(read(dir.path().join("Tasks/Compras.md")).contains("origin:Completed"));
}

#[test]
fn a_read_only_notebook_is_never_migrated() {
    // Writing to a notebook from a newer app is exactly what read-only
    // protects against — renaming its folders would be the worst version of
    // that.
    let dir = tempfile::tempdir().unwrap();
    legacy_notebook(dir.path());
    std::fs::write(
        dir.path().join(".memo/config.json"),
        r#"{ "schemaVersion": 99 }"#,
    )
    .unwrap();

    let notebook = Notebook::open(dir.path()).unwrap();

    assert!(notebook.is_read_only());
    assert!(dir.path().join("Tarefas").is_dir(), "must not be renamed");
    assert!(!dir.path().join("Tasks").exists());
}
