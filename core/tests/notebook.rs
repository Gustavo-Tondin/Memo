//! Notebook lifecycle and the writers that move tasks around.
//!
//! Every assertion here checks the file on disk, not just the in-memory
//! model: the `.md` file is the product, the struct is an implementation
//! detail.

use std::path::Path;

use memo_core::{Notebook, OriginAction, Task};

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
    let id = inbox.add_text_with_id("Comprar leite");
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
    let id = inbox.add_text_with_id("Comprar leit");
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
    notebook.create_list("Tasks", "Compras").unwrap();

    let mut compras = notebook.open_list("Tasks/Compras.md").unwrap();
    let id = compras.add_text_with_id("Comprar leite");
    compras.save().unwrap();

    let moved = notebook
        .move_task(&id, "Tasks/Compras.md", "Tasks/Completed.md", OriginAction::Record)
        .unwrap();

    assert_eq!(moved.id.as_deref(), Some(id.as_str()));
    assert_eq!(moved.origin.as_deref(), Some("Compras"));

    // Gone from the source, present in the target, both on disk.
    assert!(notebook.open_list("Tasks/Compras.md").unwrap().find(&id).is_none());
    let completed = notebook.open_list("Tasks/Completed.md").unwrap();
    let task = completed.find(&id).unwrap();
    assert_eq!(task.text, "Comprar leite");
    assert_eq!(task.origin.as_deref(), Some("Compras"));

    assert!(read(dir.path().join("Tasks/Completed.md")).contains("origin:Compras"));
}

#[test]
fn moving_a_task_back_can_clear_the_origin() {
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();
    notebook.create_list("Tasks", "Compras").unwrap();

    let mut compras = notebook.open_list("Tasks/Compras.md").unwrap();
    let id = compras.add_text_with_id("Comprar leite");
    compras.save().unwrap();

    notebook
        .move_task(&id, "Tasks/Compras.md", "Tasks/Completed.md", OriginAction::Record)
        .unwrap();
    let back = notebook
        .move_task(&id, "Tasks/Completed.md", "Tasks/Compras.md", OriginAction::Clear)
        .unwrap();

    assert_eq!(back.origin, None);
    assert!(notebook.open_list("Tasks/Completed.md").unwrap().find(&id).is_none());
    assert!(notebook.open_list("Tasks/Compras.md").unwrap().find(&id).is_some());
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
        .move_task("bbb222", "Tasks/Inbox.md", "Tasks/Completed.md", OriginAction::Record)
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

    notebook.create_list("Tasks", "Compras").unwrap();
    notebook.create_list("Tasks", "Projeto Y").unwrap();

    let names = notebook.lists().unwrap();
    let names: Vec<String> = names.into_iter().map(|l| l.name).collect();
    assert_eq!(names, vec!["Completed", "Compras", "Inbox", "Projeto Y"]);

    assert!(dir.path().join("Tasks/Projeto Y.md").is_file());
    assert!(
        notebook.create_list("Tasks", "Compras").is_err(),
        "creating a duplicate list should fail"
    );
}

#[test]
fn list_addresses_that_could_escape_the_notebook_are_rejected() {
    // Phase 7 inverted the rule: `/` stopped being forbidden and became the
    // separator. What stays forbidden is anything that climbs out, hides, or
    // breaks the comment format.
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();

    for address in [
        "../fora.md",           // climbs out of the notebook
        "Tasks/../.memo/x.md",  // climbs through a valid folder
        "/etc/passwd.md",       // absolute
        "Tasks/.oculta.md",     // hidden file
        ".memo/lista.md",       // hidden folder
        "Tasks/a\0b.md",        // NUL
        "Tasks/Mi\"casa.md",    // the comment quote character
        "Tasks/lista",          // not a .md file
        "Solta.md",             // a list never lives at the notebook root
        "Tasks//x.md",          // empty component
    ] {
        assert!(
            notebook.open_list(address).is_err(),
            "should have rejected list address {address:?}"
        );
    }

    // And the names inside create_list follow the same rule.
    for name in ["../fora", "sub/lista", "", "   ", ".oculta", "a\0b"] {
        assert!(
            notebook.create_list("Tasks", name).is_err(),
            "should have rejected list name {name:?}"
        );
    }
}

#[test]
fn a_task_earns_an_id_only_when_something_needs_to_address_it() {
    // Changed deliberately in 2026-07-20: reading a list used to hand ids to
    // every task, which put a comment on lines the user never asked about.
    let dir = tempfile::tempdir().unwrap();
    let notebook = Notebook::init(dir.path()).unwrap();

    let inbox_path = dir.path().join("Tasks/Inbox.md");
    let original = "- [ ] escrita no Obsidian\n- [ ] já tinha id <!--id:aaa111-->\n";
    std::fs::write(&inbox_path, original).unwrap();

    // Reading changes nothing.
    let tasks = notebook.tasks_in("Tasks/Inbox.md").unwrap();
    assert_eq!(tasks[0].id, None, "a plain line stays plain");
    assert_eq!(tasks[1].id.as_deref(), Some("aaa111"));
    assert_eq!(std::fs::read_to_string(&inbox_path).unwrap(), original);

    // Acting on it does.
    let id = notebook.ensure_task_id("Tasks/Inbox.md", 0).unwrap();
    assert!(!id.is_empty());
    assert!(std::fs::read_to_string(&inbox_path).unwrap().contains(&id));

    // And asking twice gives the same id, without rewriting anything.
    assert_eq!(notebook.ensure_task_id("Tasks/Inbox.md", 0).unwrap(), id);
    assert!(
        notebook.open_list("Tasks/Inbox.md").unwrap().find("aaa111").is_some(),
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
    notebook.create_list("Tasks", "Compras").unwrap();

    let mut compras = notebook.open_list("Tasks/Compras.md").unwrap();
    let id = compras.add_text_with_id("Comprar leite");
    compras.save().unwrap();

    // complete
    notebook
        .move_task(&id, "Tasks/Compras.md", "Tasks/Completed.md", OriginAction::Record)
        .unwrap();
    let mut completed = notebook.open_list("Tasks/Completed.md").unwrap();
    completed.set_done(&id, true).unwrap();
    completed.save().unwrap();
    assert!(read(dir.path().join("Tasks/Completed.md")).contains("- [x] Comprar leite"));

    // undo, back to the recorded origin
    let origin = notebook
        .open_list("Tasks/Completed.md")
        .unwrap()
        .find(&id)
        .unwrap()
        .origin
        .clone()
        .unwrap();
    assert_eq!(origin, "Compras");

    // The origin is a bare name, relative to the folder; the caller builds
    // the address — exactly what uncomplete_task does for real.
    notebook
        .move_task(
            &id,
            "Tasks/Completed.md",
            &format!("Tasks/{origin}.md"),
            OriginAction::Clear,
        )
        .unwrap();
    let mut compras = notebook.open_list("Tasks/Compras.md").unwrap();
    compras.set_done(&id, false).unwrap();
    compras.save().unwrap();

    let final_state = read(dir.path().join("Tasks/Compras.md"));
    assert!(final_state.contains("- [ ] Comprar leite"));
    assert!(read(dir.path().join("Tasks/Completed.md")).trim().is_empty());
}

#[test]
fn a_legacy_notebook_is_refused_with_a_clear_message() {
    // Decided on 2026-07-21: no migrations before v1 — there is exactly one
    // (test) notebook in the world. Refusing loudly beats converting in
    // silence while the format still changes weekly. From v1 on this
    // inverts, permanently.
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".memo")).unwrap();
    std::fs::create_dir_all(dir.path().join("Tarefas")).unwrap();
    std::fs::write(dir.path().join("Tarefas/Inbox.md"), "- [ ] antiga\n").unwrap();

    let err = Notebook::open(dir.path()).unwrap_err();
    assert!(
        matches!(err, memo_core::Error::LegacyNotebook(_)),
        "expected LegacyNotebook, got {err:?}"
    );
    // And nothing was touched: the user's files are exactly as they were.
    assert_eq!(read(dir.path().join("Tarefas/Inbox.md")), "- [ ] antiga\n");
    assert!(!dir.path().join("Tasks").exists());
}
