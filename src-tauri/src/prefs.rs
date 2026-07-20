//! Machine preferences — the ones that must NOT travel with the notebook.
//!
//! Which notebook was open last is a property of this computer, not of the
//! notebook: syncing it would make two machines fight over which notebook is
//! "the" one. So it lives in the OS config folder, while everything about the
//! notebook itself lives in `.memo/config.json` (spec 3.4).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

const FILE_NAME: &str = "machine-prefs.json";

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MachinePrefs {
    last_notebook: Option<PathBuf>,
}

fn path_of<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join(FILE_NAME))
}

fn load<R: Runtime>(app: &AppHandle<R>) -> MachinePrefs {
    // Losing this file costs the user one folder pick, so every failure path
    // degrades to the default instead of surfacing an error.
    path_of(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// The notebook open when the app was last closed, if it still exists.
pub fn last_notebook<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    load(app)
        .last_notebook
        .filter(|path| memo_core::Notebook::is_notebook(path))
}

/// Remembers a notebook as the last one opened. Best effort: failing to
/// write must never stop the notebook from opening.
pub fn remember_notebook<R: Runtime>(app: &AppHandle<R>, notebook: &Path) {
    let Some(path) = path_of(app) else { return };

    let mut prefs = load(app);
    prefs.last_notebook = Some(notebook.to_path_buf());

    let Ok(text) = serde_json::to_string_pretty(&prefs) else {
        return;
    };
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    if let Err(e) = std::fs::write(&path, text) {
        eprintln!("[memo] could not remember the last notebook: {e}");
    }
}
