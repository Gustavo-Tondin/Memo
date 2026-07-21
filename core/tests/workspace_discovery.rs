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
    assert_eq!(names, vec!["Project A"]);
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

    assert!(nb.workspaces().unwrap().is_empty());
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
    assert_eq!(names, vec!["Alpha", "Meu Espaço", "Zeta"]);
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
    let ws = &workspaces[0];
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
