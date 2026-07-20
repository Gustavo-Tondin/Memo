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
fn app_with_notebook() -> (MockApp, tempfile::TempDir) {
    let app = app();
    WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("failed to build the mock webview");

    let dir = tempfile::tempdir().unwrap();
    invoke(&app, "open_notebook", json!({ "path": dir.path() }))
        .expect("open_notebook should succeed");
    (app, dir)
}

fn ok(app: &MockApp, cmd: &str, args: Value) -> Value {
    invoke(app, cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

#[test]
fn core_version_answers_over_the_bridge() {
    let app = app();
    WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();

    let version = ok(&app, "core_version", json!({}));
    assert_eq!(version, json!(memo_core::version()));
}

#[test]
fn commands_fail_cleanly_before_a_notebook_is_open() {
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
    let (app, dir) = app_with_notebook();

    let info = ok(&app, "current_notebook", json!({}));
    assert_eq!(info["readOnly"], json!(false));
    assert_eq!(
        info["lists"],
        json!(["Completas", "Inbox"]),
        "default lists should exist and be sorted"
    );
    assert!(dir.path().join(".memo/config.json").is_file());
    assert_eq!(ok(&app, "is_notebook_open", json!({})), json!(true));
}

#[test]
fn the_full_task_lifecycle_over_the_bridge() {
    let (app, dir) = app_with_notebook();

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

    let completed = std::fs::read_to_string(dir.path().join("Tarefas/Completas.md")).unwrap();
    assert!(completed.contains("- [x] Comprar leite integral"));
    assert!(completed.contains("origin:Compras"));

    ok(&app, "uncomplete_task", json!({ "id": id }));
    let tasks = ok(&app, "list_tasks", json!({ "list": "Compras" }));
    assert_eq!(tasks[0]["done"], json!(false));
}

#[test]
fn creating_a_task_from_today_writes_it_to_the_inbox() {
    let (app, dir) = app_with_notebook();

    let id = ok(
        &app,
        "add_task_in_period",
        json!({ "period": "day", "text": "Responder e-mail" }),
    );
    let id = id.as_str().unwrap();

    let inbox = std::fs::read_to_string(dir.path().join("Tarefas/Inbox.md")).unwrap();
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
    let (app, dir) = app_with_notebook();

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
    assert!(dir.path().join("Tarefas/Mercado.md").is_file());

    // Deleting rescues the task into the Inbox rather than dropping it.
    let rescued = ok(&app, "delete_list", json!({ "name": "Mercado" }));
    assert_eq!(rescued, json!(1));
    assert!(std::fs::read_to_string(dir.path().join("Tarefas/Inbox.md"))
        .unwrap()
        .contains("Comprar leite"));
}

#[test]
fn errors_arrive_typed_so_the_ui_can_branch_on_them() {
    let (app, _dir) = app_with_notebook();

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
    let (app, dir) = app_with_notebook();

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
    let (app, _dir) = app_with_notebook();

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
    let (app, _dir) = app_with_notebook();

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
    let (app, _dir) = app_with_notebook();

    let states = ok(&app, "refresh_periods", json!({}));
    assert_eq!(states.as_array().unwrap().len(), 2);
    assert!(states[0]["date"].is_string());
    assert!(states[1]["items"].is_array());
}

#[test]
fn external_changes_reach_the_frontend_as_events() {
    // The Syncthing scenario: something else writes the file, and the app has
    // to hear about it without polling.
    use tauri::Listener;

    let (app, dir) = app_with_notebook();
    let (tx, rx) = std::sync::mpsc::channel();

    app.listen_any("notebook://changed", move |event| {
        let _ = tx.send(event.payload().to_string());
    });

    std::fs::write(
        dir.path().join("Tarefas/Inbox.md"),
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
    let (app, first) = app_with_notebook();
    ok(&app, "create_list", json!({ "name": "SoNoPrimeiro" }));

    let second = tempfile::tempdir().unwrap();
    ok(&app, "open_notebook", json!({ "path": second.path() }));

    let info = ok(&app, "current_notebook", json!({}));
    assert_eq!(info["path"], json!(second.path()));
    assert_eq!(info["lists"], json!(["Completas", "Inbox"]));

    // The first notebook is untouched on disk, just no longer open.
    assert!(first.path().join("Tarefas/SoNoPrimeiro.md").is_file());
}
