//! Tauri shell — thin layer only.
//!
//! No business logic here: every command must delegate to `memo_core`.
//! Phase 3 of the roadmap fills this with one `invoke()` command per core
//! operation.

/// Smoke-test command, used in Phase 0 to prove the frontend can reach Rust.
#[tauri::command]
fn core_version() -> String {
    let version = memo_core::version().to_string();
    eprintln!("[memo] command core_version -> {version}");
    version
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![core_version])
        .run(tauri::generate_context!())
        .expect("error while running Memo");
}
