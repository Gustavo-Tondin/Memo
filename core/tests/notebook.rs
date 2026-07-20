//! Notebook lifecycle and the writers that move tasks around.
//!
//! Every assertion here checks the file on disk, not just the in-memory
//! model: the `.md` file is the product, the struct is an implementation
//! detail.

use std::path::Path;

use memo_core::{Notebook, OriginAction, Task, TaskList};

fn read(path: impl AsRef<Path>) -> String {
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn a_plain_folder_is_not_a_notebook() {
    let dir = tempfile::tempdir().unwrap();
    assert!(!Notebook::is_notebook(dir.path()));
    assert!(Notebook::open(dir.path()).is_err());
}

#[test]
fn init_creates_the_documented_layout() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();

    assert!(dir.path().join(".memo").is_dir());
    assert!(dir.path().join("Tasks").is_dir());
    assert!(dir.path().join("Notes").is_dir());
    assert!(dir.path().join("Tasks/Inbox.md").is_file());
    assert!(dir.path().join("Tasks/Completed.md").is_file());
    assert!(Notebook::is_notebook(notebook.root()));
}

#[test]
fn init_writes_a_config_container_with_a_schema_version() {
    let dir = tempfile::tempdir().unwrap();
    Notebook::init(dir.path()).unwrap();

    let config: serde_json::Value =
        serde_json::from_str(&read(dir.path().join(".memo/config.json"))).unwrap();
    assert_eq!(config["schemaVersion"], 1);
}

#[test]
fn init_refuses_to_overwrite_an_existing_notebook() {
    let dir = tempfile::tempdir().unwrap();
    Notebook::init(dir.path()).unwrap();
    assert!(Notebook::init(dir.path()).is_err());
}

#[test]
fn open_recreates_default_lists_deleted_from_outside() {
    let dir = tempfile::tempdir().unwrap();
    Notebook::init(dir.path()).unwrap();

    std::fs::remove_file(dir.path().join("Tasks/Inbox.md")).unwrap();
    std::fs::remove_file(dir.path().join("Tasks/Completed.md")).unwrap();

    Notebook::open(dir.path()).unwrap();

    assert!(dir.path().join("Tasks/Inbox.md").is_file());
    assert!(dir.path().join("Tasks/Completed.md").is_file());
}

#[test]
fn open_does_not_touch_lists_that_already_have_content() {
    let dir = tempfile::tempdir().unwrap();
    Notebook::init(dir.path()).unwrap();

    let inbox_path = dir.path().join("Tasks/Inbox.md");
    let content = "- [ ] Comprar leite <!--id:a1b2c3-->\n";
    std::fs::write(&inbox_path, content).unwrap();

    Notebook::open(dir.path()).unwrap();
    assert_eq!(read(&inbox_path), content, "reopening rewrote the list");
}

#[test]
fn adding_a_task_writes_it_to_the_file_with_an_id() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();

    let mut inbox = notebook.inbox().unwrap();
    let id = inbox.add_text("Comprar leite");
    inbox.save().unwrap();

    let on_disk = read(dir.path().join("Tasks/Inbox.md"));
    assert!(on_disk.contains("- [ ] Comprar leite"));
    assert!(on_disk.contains(&format!("id:{id}")));

    // And it survives a round trip through the disk.
    let reloaded = notebook.inbox().unwrap();
    assert_eq!(reloaded.find(&id).unwrap().text, "Comprar leite");
}

#[test]
fn editing_a_task_keeps_its_id_and_its_state() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();

    let mut inbox = notebook.inbox().unwrap();
    let id = inbox.add_text("Comprar leit");
    inbox.set_done(&id, true).unwrap();
    inbox.save().unwrap();

    let mut inbox = notebook.inbox().unwrap();
    inbox.edit_text(&id, "Comprar leite").unwrap();
    inbox.save().unwrap();

    let task = notebook.inbox().unwrap().find(&id).unwrap().clone();
    assert_eq!(task.text, "Comprar leite");
    assert_eq!(task.id.as_deref(), Some(id.as_str()));
    assert!(task.done, "editing the text changed the completion state");
}

#[test]
fn editing_an_unknown_id_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();
    let mut inbox = notebook.inbox().unwrap();
    assert!(inbox.edit_text("nao-existe", "x").is_err());
}

#[test]
fn moving_a_task_preserves_the_id_and_records_the_origin() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();
    notebook.create_list("Compras").unwrap();

    let mut compras = notebook.open_list("Compras").unwrap();
    let id = compras.add_text("Comprar leite");
    compras.save().unwrap();

    let moved = notebook
        .move_task(&id, "Compras", "Completed", OriginAction::Record)
        .unwrap();

    assert_eq!(moved.id.as_deref(), Some(id.as_str()));
    assert_eq!(moved.origin.as_deref(), Some("Compras"));

    // Gone from the source, present in the target, both on disk.
    assert!(notebook.open_list("Compras").unwrap().find(&id).is_none());
    let completed = notebook.completed().unwrap();
    let task = completed.find(&id).unwrap();
    assert_eq!(task.text, "Comprar leite");
    assert_eq!(task.origin.as_deref(), Some("Compras"));

    assert!(read(dir.path().join("Tasks/Completed.md")).contains("origin:Compras"));
}

#[test]
fn moving_a_task_back_can_clear_the_origin() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();
    notebook.create_list("Compras").unwrap();

    let mut compras = notebook.open_list("Compras").unwrap();
    let id = compras.add_text("Comprar leite");
    compras.save().unwrap();

    notebook
        .move_task(&id, "Compras", "Completed", OriginAction::Record)
        .unwrap();
    let back = notebook
        .move_task(&id, "Completed", "Compras", OriginAction::Clear)
        .unwrap();

    assert_eq!(back.origin, None);
    assert!(notebook.completed().unwrap().find(&id).is_none());
    assert!(notebook.open_list("Compras").unwrap().find(&id).is_some());
    assert!(!read(dir.path().join("Tasks/Compras.md")).contains("origin:"));
}

#[test]
fn moving_a_task_does_not_disturb_the_other_lines() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();

    let inbox_path = dir.path().join("Tasks/Inbox.md");
    std::fs::write(
        &inbox_path,
        "# Inbox\n\
         \n\
         - [ ] fica <!--id:aaa111-->\n\
         - [ ] sai <!--id:bbb222-->\n",
    )
    .unwrap();

    notebook
        .move_task("bbb222", "Inbox", "Completed", OriginAction::Record)
        .unwrap();

    let inbox = read(&inbox_path);
    assert!(inbox.starts_with("# Inbox\n\n"), "heading was lost: {inbox:?}");
    assert!(inbox.contains("- [ ] fica <!--id:aaa111-->"));
    assert!(!inbox.contains("bbb222"));
}

#[test]
fn creating_and_listing_lists() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();

    notebook.create_list("Compras").unwrap();
    notebook.create_list("Projeto Y").unwrap();

    let names = notebook.list_names().unwrap();
    assert_eq!(names, vec!["Completed", "Compras", "Inbox", "Projeto Y"]);

    assert!(dir.path().join("Tasks/Projeto Y.md").is_file());
    assert!(
        notebook.create_list("Compras").is_err(),
        "creating a duplicate list should fail"
    );
}

#[test]
fn list_names_that_could_escape_the_folder_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();

    for name in ["../fora", "sub/lista", "", "   ", ".oculta", "a\0b"] {
        assert!(
            notebook.list_path(name).is_err(),
            "should have rejected list name {name:?}"
        );
    }
}

#[test]
fn tasks_written_by_hand_are_adopted_with_ids() {
    let dir = tempfile::tempdir().unwrap();
    Notebook::init(dir.path()).unwrap();

    let inbox_path = dir.path().join("Tasks/Inbox.md");
    std::fs::write(
        &inbox_path,
        "- [ ] escrita no Obsidian\n- [ ] já tinha id <!--id:aaa111-->\n",
    )
    .unwrap();

    let mut inbox = TaskList::load(&inbox_path).unwrap();
    assert_eq!(inbox.ensure_unique_ids(), 1, "should adopt exactly one task");
    inbox.save().unwrap();

    let inbox = TaskList::load(&inbox_path).unwrap();
    assert!(inbox.tasks().all(|t| t.id.is_some()));
    assert!(
        inbox.find("aaa111").is_some(),
        "the existing id must not be regenerated"
    );
}

#[test]
fn saving_is_atomic_and_leaves_no_temp_file_behind() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();

    let mut inbox = notebook.inbox().unwrap();
    inbox.add(Task::new("Comprar leite"));
    inbox.save().unwrap();

    let leftovers: Vec<_> = std::fs::read_dir(dir.path().join("Tasks"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|name| name.ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "temp files left behind: {leftovers:?}");
}

#[test]
fn a_full_round_trip_through_the_notebook() {
    // create → complete → undo, checking the files at every step.
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();
    notebook.create_list("Compras").unwrap();

    let mut compras = notebook.open_list("Compras").unwrap();
    let id = compras.add_text("Comprar leite");
    compras.save().unwrap();

    // complete
    notebook
        .move_task(&id, "Compras", "Completed", OriginAction::Record)
        .unwrap();
    let mut completed = notebook.completed().unwrap();
    completed.set_done(&id, true).unwrap();
    completed.save().unwrap();
    assert!(read(dir.path().join("Tasks/Completed.md")).contains("- [x] Comprar leite"));

    // undo, back to the recorded origin
    let origin = notebook
        .completed()
        .unwrap()
        .find(&id)
        .unwrap()
        .origin
        .clone()
        .unwrap();
    assert_eq!(origin, "Compras");

    notebook
        .move_task(&id, "Completed", &origin, OriginAction::Clear)
        .unwrap();
    let mut compras = notebook.open_list("Compras").unwrap();
    compras.set_done(&id, false).unwrap();
    compras.save().unwrap();

    let final_state = read(dir.path().join("Tasks/Compras.md"));
    assert!(final_state.contains("- [ ] Comprar leite"));
    assert!(read(dir.path().join("Tasks/Completed.md")).trim().is_empty());
}
