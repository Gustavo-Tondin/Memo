//! Workspace discovery against the real filesystem (phase 7, step A).
//!
//! The rule under test: a first-level folder is a workspace when — and only
//! when — it carries a `.workspace.json`. Everything else in the notebook
//! stays invisible, no matter how much it looks like content.

use memo_core::{Notebook, Workspace};

fn notebook() -> (tempfile::TempDir, Notebook) {
    let dir = tempfile::tempdir().unwrap();
    let nb = Notebook::init(dir.path()).unwrap();
    (dir, nb)
}

fn make_workspace(root: &std::path::Path, name: &str, config: &str) {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(".workspace.json"), config).unwrap();
}

#[test]
fn only_marked_folders_are_workspaces() {
    let (dir, nb) = notebook();
    make_workspace(dir.path(), "Project A", r#"{ "schemaVersion": 1 }"#);

    // Folders without the marker — however plausible — are not interface.
    std::fs::create_dir_all(dir.path().join("Downloads")).unwrap();
    std::fs::create_dir_all(dir.path().join("attachments")).unwrap();
    // A stray file at the first level is not one either.
    std::fs::write(dir.path().join("README.md"), "hi\n").unwrap();

    let names: Vec<String> = nb
        .workspaces()
        .unwrap()
        .iter()
        .map(|w| w.folder_name().to_string())
        .collect();
    // The three fixed workspaces exist since init (step D), plus the marked one.
    assert_eq!(names, vec!["Home", "Notes", "Project A", "Tasks"]);
}

#[test]
fn the_config_folder_is_never_a_workspace() {
    let (dir, nb) = notebook();
    // Even sabotaged with a marker, a hidden folder stays invisible.
    std::fs::write(
        dir.path().join(".memo/.workspace.json"),
        r#"{ "schemaVersion": 1 }"#,
    )
    .unwrap();

    let names: Vec<String> = nb
        .workspaces()
        .unwrap()
        .iter()
        .map(|w| w.folder_name().to_string())
        .collect();
    assert_eq!(names, vec!["Home", "Notes", "Tasks"], "no .memo in here");
}

#[test]
fn workspaces_come_back_sorted_by_folder_name() {
    let (dir, nb) = notebook();
    for name in ["Zeta", "Alpha", "Meu Espaço"] {
        make_workspace(dir.path(), name, r#"{ "schemaVersion": 1 }"#);
    }

    let names: Vec<String> = nb
        .workspaces()
        .unwrap()
        .iter()
        .map(|w| w.folder_name().to_string())
        .collect();
    assert_eq!(
        names,
        vec!["Alpha", "Home", "Meu Espaço", "Notes", "Tasks", "Zeta"]
    );
}

#[test]
fn a_template_from_the_future_opens_but_stays_untouchable() {
    // The community-template scenario end to end: unzip a folder written by
    // a newer version into the notebook, and nothing breaks, nothing is lost.
    let (dir, nb) = notebook();
    make_workspace(
        dir.path(),
        "Do Futuro",
        r#"{ "schemaVersion": 99, "widgets": [
            { "type": "hologram", "folder": "Cards", "shader": "neon" }
        ] }"#,
    );

    let workspaces = nb.workspaces().unwrap();
    let ws = workspaces
        .iter()
        .find(|w| w.folder_name() == "Do Futuro")
        .unwrap();
    assert!(ws.config.is_read_only());
    assert!(!ws.config.widgets[0].is_known());
    assert!(ws.config.save(ws.config_path()).is_err(), "must refuse to rewrite");

    // The bytes on disk are exactly what the template author wrote.
    let on_disk =
        std::fs::read_to_string(dir.path().join("Do Futuro/.workspace.json")).unwrap();
    assert!(on_disk.contains("shader"));
}

#[test]
fn opening_a_workspace_directly_requires_the_marker() {
    let (dir, _nb) = notebook();
    std::fs::create_dir_all(dir.path().join("Solta")).unwrap();
    assert!(Workspace::open(dir.path().join("Solta")).is_err());
}

#[test]
fn a_second_tasks_widget_feeds_lists_counts_and_suggestions() {
    // The point of the whole phase: a user workspace with its own tasks
    // widget joins the navigation and the suggestions without touching the
    // fixed Tasks/ — and two lists called Inbox never get confused.
    use memo_core::state::Period;

    let (dir, nb) = notebook();
    make_workspace(
        dir.path(),
        "Project A",
        r#"{ "schemaVersion": 1, "widgets": [
            { "type": "tasks", "folder": "Backlog" }
        ] }"#,
    );
    std::fs::create_dir_all(dir.path().join("Project A/Backlog")).unwrap();
    std::fs::write(
        dir.path().join("Project A/Backlog/Inbox.md"),
        "- [ ] tarefa do projeto\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("Tasks/Inbox.md"),
        "- [ ] tarefa pessoal\n",
    )
    .unwrap();

    // Both Inboxes are listed, distinguished by address.
    let lists = nb.lists().unwrap();
    let paths: Vec<&str> = lists.iter().map(|l| l.path.as_str()).collect();
    assert!(paths.contains(&"Tasks/Inbox.md"));
    assert!(paths.contains(&"Project A/Backlog/Inbox.md"));

    // Counts keyed by address never collide.
    let counts = nb.open_task_counts().unwrap();
    assert_eq!(counts.get("Tasks/Inbox.md"), Some(&1));
    assert_eq!(counts.get("Project A/Backlog/Inbox.md"), Some(&1));

    // The day suggests from both folders.
    let suggestions = nb.suggestions_for(Period::Day).unwrap();
    let texts: Vec<&str> = suggestions.iter().map(|s| s.task.text.as_str()).collect();
    assert!(texts.contains(&"tarefa do projeto"));
    assert!(texts.contains(&"tarefa pessoal"));

    // Completing in the project keeps everything inside the project's folder.
    let id = nb.ensure_task_id("Project A/Backlog/Inbox.md", 0).unwrap();
    nb.pull_into(Period::Day, "Project A/Backlog/Inbox.md", &id).unwrap();
    nb.complete_task("Project A/Backlog/Inbox.md", &id).unwrap();

    let completed =
        std::fs::read_to_string(dir.path().join("Project A/Backlog/Completed.md")).unwrap();
    assert!(completed.contains("tarefa do projeto"));
    assert!(
        !std::fs::read_to_string(dir.path().join("Tasks/Completed.md"))
            .unwrap()
            .contains("tarefa do projeto"),
        "the fixed Completed must not receive another workspace's task"
    );

    // The personal Inbox was never touched by any of it.
    assert_eq!(
        std::fs::read_to_string(dir.path().join("Tasks/Inbox.md")).unwrap(),
        "- [ ] tarefa pessoal\n"
    );

    // And the undo goes back to the project's own Inbox.
    nb.uncomplete_task("Project A/Backlog/Completed.md", &id).unwrap();
    assert!(
        std::fs::read_to_string(dir.path().join("Project A/Backlog/Inbox.md"))
            .unwrap()
            .contains("tarefa do projeto")
    );
}
