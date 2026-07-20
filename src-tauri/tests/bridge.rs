//! The Tauri bridge, driven through the real IPC layer.
//!
//! Uses Tauri's mock runtime, so every call goes through `invoke()` exactly
//! as it does from the frontend — argument names, camelCase conversion,
//! serialization and error shape all get exercised. A command that only works
//! when called directly in Rust would pass a unit test and still fail in the
//! app; this catches that.

use serde_json::{json, Value};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{mock_context, noop_assets, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{Manager, WebviewWindowBuilder};

type MockApp = tauri::App<tauri::test::MockRuntime>;

fn app() -> MockApp {
    memo_lib::configure(tauri::test::mock_builder())
        .build(mock_context(noop_assets()))
        .expect("failed to build the mock app")
}

/// Calls a command the way the webview does. `Err` carries whatever the
/// command returned as its error payload.
fn invoke(app: &MockApp, cmd: &str, args: Value) -> Result<Value, Value> {
    let webview = app
        .get_webview_window("main")
        .expect("main webview should exist");

    let request = InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: if cfg!(any(windows, target_os = "android")) {
            "http://tauri.localhost"
        } else {
            "tauri://localhost"
        }
        .parse()
        .unwrap(),
        body: InvokeBody::Json(args),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    };

    tauri::test::get_ipc_response(&webview, request)
        .map(|body| body.deserialize::<Value>().unwrap())
}

/// An app with a webview and a freshly created notebook, ready to drive.
fn app_with_notebook() -> (std::sync::MutexGuard<'static, ()>, MockApp, tempfile::TempDir) {
    let lock = exclusive();
    let app = app();
    WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("failed to build the mock webview");

    let dir = tempfile::tempdir().unwrap();
    invoke(&app, "open_notebook", json!({ "path": dir.path() }))
        .expect("open_notebook should succeed");
    (lock, app, dir)
}

fn ok(app: &MockApp, cmd: &str, args: Value) -> Value {
    invoke(app, cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

/// Serializes the whole suite and gives it a clean preferences file.
///
/// Machine preferences are one file per machine, and **every** test that opens
/// a notebook writes to it. Isolating only the tests that assert on it is not
/// enough: the other ones still race against those. Since the suite runs in
/// milliseconds, running it one test at a time is the cheap, honest fix.
///
/// The guard must be held for the whole test — bind it, do not discard it.
#[must_use]
fn exclusive() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();

    // A poisoned lock only means an earlier test panicked; the isolation still
    // works, so recover instead of failing every test after the first failure.
    let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let dir = DIR.get_or_init(|| tempfile::tempdir().unwrap());
    std::env::set_var("MEMO_CONFIG_DIR", dir.path());
    let _ = std::fs::remove_file(dir.path().join("machine-prefs.json"));

    guard
}

#[test]
fn core_version_answers_over_the_bridge() {
    let _lock = exclusive();
    let app = app();
    WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();

    let version = ok(&app, "core_version", json!({}));
    assert_eq!(version, json!(memo_core::version()));
}

#[test]
fn commands_fail_cleanly_before_a_notebook_is_open() {
    let _lock = exclusive();
    // The UI can call something before onboarding finishes; that must be a
    // typed error, not a panic that takes the window down.
    let app = app();
    WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();

    let err = invoke(&app, "list_names", json!({})).unwrap_err();
    assert_eq!(err["kind"], "noNotebook");
    assert_eq!(ok(&app, "is_notebook_open", json!({})), json!(false));
    assert_eq!(ok(&app, "current_notebook", json!({})), Value::Null);
}

#[test]
fn opening_a_notebook_reports_it_and_creates_the_layout() {
    let (_lock, app, dir) = app_with_notebook();

    let info = ok(&app, "current_notebook", json!({}));
    assert_eq!(info["readOnly"], json!(false));
    assert_eq!(
        info["lists"],
        json!(["Completed", "Inbox"]),
        "default lists should exist and be sorted"
    );
    assert!(dir.path().join(".memo/config.json").is_file());
    assert_eq!(ok(&app, "is_notebook_open", json!({})), json!(true));
}

#[test]
fn the_full_task_lifecycle_over_the_bridge() {
    let (_lock, app, dir) = app_with_notebook();

    ok(&app, "create_list", json!({ "name": "Compras" }));
    let id = ok(
        &app,
        "create_task",
        json!({ "list": "Compras", "text": "Comprar leite" }),
    );
    let id = id.as_str().unwrap().to_string();

    ok(
        &app,
        "edit_task_text",
        json!({ "list": "Compras", "id": id, "text": "Comprar leite integral" }),
    );

    let tasks = ok(&app, "list_tasks", json!({ "list": "Compras" }));
    assert_eq!(tasks[0]["text"], "Comprar leite integral");
    assert_eq!(tasks[0]["done"], json!(false));

    // Pull into both periods, then complete: the references must go away.
    ok(
        &app,
        "pull_into_period",
        json!({ "period": "week", "list": "Compras", "id": id }),
    );
    ok(
        &app,
        "pull_into_period",
        json!({ "period": "day", "list": "Compras", "id": id }),
    );
    let state = ok(&app, "period_state", json!({ "period": "day" }));
    assert_eq!(state["items"][0]["list"], "Compras");

    ok(
        &app,
        "complete_task",
        json!({ "list": "Compras", "id": id }),
    );
    let state = ok(&app, "period_state", json!({ "period": "day" }));
    assert_eq!(state["items"], json!([]));

    let completed = std::fs::read_to_string(dir.path().join("Tasks/Completed.md")).unwrap();
    assert!(completed.contains("- [x] Comprar leite integral"));
    assert!(completed.contains("origin:Compras"));

    ok(&app, "uncomplete_task", json!({ "id": id }));
    let tasks = ok(&app, "list_tasks", json!({ "list": "Compras" }));
    assert_eq!(tasks[0]["done"], json!(false));
}

#[test]
fn creating_a_task_from_today_writes_it_to_the_inbox() {
    let (_lock, app, dir) = app_with_notebook();

    let id = ok(
        &app,
        "add_task_in_period",
        json!({ "period": "day", "text": "Responder e-mail" }),
    );
    let id = id.as_str().unwrap();

    let inbox = std::fs::read_to_string(dir.path().join("Tasks/Inbox.md")).unwrap();
    assert!(inbox.contains("Responder e-mail"));

    let state = ok(&app, "period_state", json!({ "period": "day" }));
    assert_eq!(state["items"][0]["list"], "Inbox");
    assert_eq!(state["items"][0]["id"], id);

    assert_eq!(
        ok(
            &app,
            "remove_from_period",
            json!({ "period": "day", "list": "Inbox", "id": id })
        ),
        json!(true)
    );
}

#[test]
fn list_management_over_the_bridge() {
    let (_lock, app, dir) = app_with_notebook();

    ok(&app, "create_list", json!({ "name": "Compras" }));
    ok(
        &app,
        "create_task",
        json!({ "list": "Compras", "text": "Comprar leite" }),
    );

    ok(
        &app,
        "rename_list",
        json!({ "from": "Compras", "to": "Mercado" }),
    );
    assert!(dir.path().join("Tasks/Mercado.md").is_file());

    // Deleting rescues the task into the Inbox rather than dropping it.
    let rescued = ok(&app, "delete_list", json!({ "name": "Mercado" }));
    assert_eq!(rescued, json!(1));
    assert!(std::fs::read_to_string(dir.path().join("Tasks/Inbox.md"))
        .unwrap()
        .contains("Comprar leite"));
}

#[test]
fn errors_arrive_typed_so_the_ui_can_branch_on_them() {
    let (_lock, app, _dir) = app_with_notebook();

    let err = invoke(
        &app,
        "complete_task",
        json!({ "list": "Inbox", "id": "nao-existe" }),
    )
    .unwrap_err();
    assert_eq!(err["kind"], "taskNotFound");
    assert!(err["message"].as_str().unwrap().contains("nao-existe"));

    let err = invoke(&app, "delete_list", json!({ "name": "Inbox" })).unwrap_err();
    assert_eq!(err["kind"], "protectedList");

    let err = invoke(&app, "create_list", json!({ "name": "../fuga" })).unwrap_err();
    assert_eq!(err["kind"], "invalidListName");
}

#[test]
fn settings_round_trip_through_the_bridge() {
    let (_lock, app, dir) = app_with_notebook();

    let defaults = ok(&app, "notebook_settings", json!({}));
    assert_eq!(defaults["dailyMode"], "reset");
    assert_eq!(defaults["dailyAt"], "00:00");
    assert_eq!(defaults["weekStartsOn"], "monday");

    ok(
        &app,
        "set_notebook_settings",
        json!({
            "settings": {
                "dailyMode": "carry",
                "dailyAt": "-02:00",
                "weeklyMode": "reset",
                "weeklyAt": "02:00",
                "weekStartsOn": "sunday"
            }
        }),
    );

    let saved = ok(&app, "notebook_settings", json!({}));
    assert_eq!(saved["dailyMode"], "carry");
    assert_eq!(saved["dailyAt"], "-02:00");
    assert_eq!(saved["weekStartsOn"], "sunday");

    // And it really reached the file, not just the in-memory config.
    let on_disk = std::fs::read_to_string(dir.path().join(".memo/config.json")).unwrap();
    assert!(on_disk.contains("carry"));
    assert!(on_disk.contains("-02:00"));
}

#[test]
fn nonsense_settings_are_normalized_instead_of_corrupting_the_config() {
    let (_lock, app, _dir) = app_with_notebook();

    ok(
        &app,
        "set_notebook_settings",
        json!({
            "settings": {
                "dailyMode": "banana",
                "dailyAt": "99:99",
                "weeklyMode": "",
                "weeklyAt": "nope",
                "weekStartsOn": "caturday"
            }
        }),
    );

    let saved = ok(&app, "notebook_settings", json!({}));
    assert_eq!(saved["dailyMode"], "reset");
    assert_eq!(saved["dailyAt"], "00:00");
    assert_eq!(saved["weekStartsOn"], "monday");
}

#[test]
fn the_clock_command_reports_the_logical_periods() {
    let (_lock, app, _dir) = app_with_notebook();

    let clock = ok(&app, "period_clock", json!({}));
    let today = clock["today"].as_str().unwrap();
    let week_start = clock["weekStart"].as_str().unwrap();

    // Shape matters more than the value: the UI parses these.
    assert_eq!(today.len(), 10, "expected YYYY-MM-DD, got {today}");
    assert!(week_start <= today, "week must start on or before today");
    assert!(clock["nextDailyTurn"].as_str().unwrap().contains('T'));
}

#[test]
fn refresh_periods_returns_both_states() {
    let (_lock, app, _dir) = app_with_notebook();

    let states = ok(&app, "refresh_periods", json!({}));
    assert_eq!(states.as_array().unwrap().len(), 2);
    assert!(states[0]["date"].is_string());
    assert!(states[1]["items"].is_array());
}

/// Guards a bug that no IPC test can catch: a native dialog cannot be driven
/// from a test, so the only thing standing between us and a frozen window is
/// this rule.
///
/// A synchronous `#[tauri::command]` runs on the main thread, and the plugin's
/// `blocking_*` helpers explicitly must not. The combination locks the GTK
/// event loop the instant the dialog opens — which is exactly what happened
/// the first time this command shipped.
#[test]
fn dialog_helpers_are_never_called_from_a_blocking_command() {
    let source = include_str!("../src/commands.rs");

    for (number, line) in source.lines().enumerate() {
        let code = line.trim();
        // Comments may name it — the docs on the command explain the trap.
        if code.starts_with("//") {
            continue;
        }
        assert!(
            !code.contains("blocking_pick"),
            "commands.rs:{}: blocking_pick_* freezes the window; use the \
             callback form inside an async command instead",
            number + 1
        );
    }

    // And the command that opens the picker must stay async.
    let picker = source
        .split("pub async fn pick_notebook_folder")
        .count();
    assert_eq!(
        picker, 2,
        "pick_notebook_folder must be an async command — a sync one runs on \
         the main thread and freezes the dialog"
    );
}

#[test]
fn the_day_offers_the_week_first_then_the_rest() {
    let (_lock, app, _dir) = app_with_notebook();
    ok(&app, "create_list", json!({ "name": "Compras" }));

    let solta = ok(
        &app,
        "create_task",
        json!({ "list": "Inbox", "text": "Tarefa solta" }),
    );
    let semana = ok(
        &app,
        "create_task",
        json!({ "list": "Compras", "text": "Escolhida pra semana" }),
    );
    ok(
        &app,
        "pull_into_period",
        json!({ "period": "week", "list": "Compras", "id": semana }),
    );

    let suggestions = ok(&app, "period_suggestions", json!({ "period": "day" }));
    assert_eq!(suggestions[0]["task"]["id"], semana);
    assert_eq!(suggestions[0]["list"], "Compras");
    assert_eq!(suggestions[1]["task"]["id"], solta);

    // Once pulled, it stops being a suggestion and shows up as pulled.
    ok(
        &app,
        "pull_into_period",
        json!({ "period": "day", "list": "Compras", "id": semana }),
    );
    let pulled = ok(&app, "period_tasks", json!({ "period": "day" }));
    assert_eq!(pulled[0]["task"]["text"], "Escolhida pra semana");

    let suggestions = ok(&app, "period_suggestions", json!({ "period": "day" }));
    assert_eq!(suggestions.as_array().unwrap().len(), 1);
}

#[test]
fn sync_conflicts_reach_the_frontend() {
    let (_lock, app, dir) = app_with_notebook();
    ok(&app, "create_list", json!({ "name": "Compras" }));

    assert_eq!(ok(&app, "list_conflicts", json!({})), json!([]));

    std::fs::write(
        dir.path()
            .join("Tasks/Compras.sync-conflict-20260720-143000-K3F7NLM.md"),
        "- [ ] versão do celular\n",
    )
    .unwrap();

    let conflicts = ok(&app, "list_conflicts", json!({}));
    assert_eq!(conflicts.as_array().unwrap().len(), 1);
    assert_eq!(conflicts[0]["list"], "Compras");
    assert!(conflicts[0]["original"].as_str().unwrap().ends_with("Compras.md"));

    // And it must not have become a list in the sidebar.
    assert_eq!(ok(&app, "list_names", json!({})), json!(["Completed", "Compras", "Inbox"]));
}

#[test]
fn a_partial_settings_payload_keeps_what_it_did_not_mention() {
    // An older frontend, or a screen that only edits one thing, must not wipe
    // the preferences it does not know about.
    let (_lock, app, _dir) = app_with_notebook();

    ok(
        &app,
        "set_notebook_settings",
        json!({ "settings": { "dailyMode": "carry", "showListCounts": false } }),
    );
    ok(
        &app,
        "set_notebook_settings",
        json!({ "settings": { "weeklyAt": "02:00" } }),
    );

    let saved = ok(&app, "notebook_settings", json!({}));
    assert_eq!(saved["weeklyAt"], "02:00", "the field that was sent");
    assert_eq!(saved["dailyMode"], "carry", "survived the second call");
    assert_eq!(saved["showListCounts"], json!(false), "survived too");
}

#[test]
fn list_counts_follow_the_setting() {
    let (_lock, app, _dir) = app_with_notebook();
    ok(&app, "create_list", json!({ "name": "Compras" }));
    ok(
        &app,
        "create_task",
        json!({ "list": "Compras", "text": "Comprar leite" }),
    );

    let counts = ok(&app, "list_counts", json!({}));
    assert_eq!(counts["Compras"], json!(1));
    assert_eq!(counts["Inbox"], json!(0));

    // Turned off, the command answers empty — the frontend does not need to
    // know the rule, it just renders what it gets.
    let mut settings = ok(&app, "notebook_settings", json!({}));
    settings["showListCounts"] = json!(false);
    ok(&app, "set_notebook_settings", json!({ "settings": settings }));

    assert_eq!(ok(&app, "list_counts", json!({})), json!({}));
}

#[test]
fn the_last_screen_is_only_restored_when_the_user_asked_for_it() {
    let (_lock, app, _dir) = app_with_notebook();

    // Off by default: nothing is stored and nothing is restored.
    ok(&app, "remember_screen", json!({ "screen": "list:Compras" }));
    assert_eq!(ok(&app, "screen_to_restore", json!({})), Value::Null);

    let mut settings = ok(&app, "notebook_settings", json!({}));
    assert_eq!(settings["restoreLastScreen"], json!(false));
    settings["restoreLastScreen"] = json!(true);
    ok(&app, "set_notebook_settings", json!({ "settings": settings }));

    // Still null: turning the preference on must not resurrect a screen the
    // app was never allowed to record.
    assert_eq!(ok(&app, "screen_to_restore", json!({})), Value::Null);

    ok(&app, "remember_screen", json!({ "screen": "week" }));
    assert_eq!(ok(&app, "screen_to_restore", json!({})), json!("week"));
}

#[test]
fn the_last_notebook_is_remembered_across_launches() {
    let (_lock, app, dir) = app_with_notebook();

    let remembered = ok(&app, "last_notebook", json!({}));
    assert_eq!(remembered, json!(dir.path()));
}

#[test]
fn external_changes_reach_the_frontend_as_events() {
    // The Syncthing scenario: something else writes the file, and the app has
    // to hear about it without polling.
    use tauri::Listener;

    let (_lock, app, dir) = app_with_notebook();
    let (tx, rx) = std::sync::mpsc::channel();

    app.listen_any("notebook://changed", move |event| {
        let _ = tx.send(event.payload().to_string());
    });

    std::fs::write(
        dir.path().join("Tasks/Inbox.md"),
        "- [ ] escrita por outro app\n",
    )
    .unwrap();

    let payload = rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("an external write should emit a change event");

    let change: Value = serde_json::from_str(&payload).unwrap();
    assert_eq!(change["kind"], "list");
    assert!(change["path"].as_str().unwrap().ends_with("Inbox.md"));
}

#[test]
fn opening_a_second_notebook_switches_the_open_one() {
    let (_lock, app, first) = app_with_notebook();
    ok(&app, "create_list", json!({ "name": "SoNoPrimeiro" }));

    let second = tempfile::tempdir().unwrap();
    ok(&app, "open_notebook", json!({ "path": second.path() }));

    let info = ok(&app, "current_notebook", json!({}));
    assert_eq!(info["path"], json!(second.path()));
    assert_eq!(info["lists"], json!(["Completed", "Inbox"]));

    // The first notebook is untouched on disk, just no longer open.
    assert!(first.path().join("Tasks/SoNoPrimeiro.md").is_file());
}
