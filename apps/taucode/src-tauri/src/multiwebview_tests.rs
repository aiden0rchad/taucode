use crate::{
    backend::events::{AppServerEvent, EventSink, TerminalExit, TerminalOutput},
    event_sink,
};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tauri::{
    test::{mock_builder, mock_context, noop_assets, MockRuntime},
    Listener, Manager,
};

fn ipc(
    view: &tauri::Webview<MockRuntime>,
    cmd: &str,
    body: serde_json::Value,
) -> Result<tauri::ipc::InvokeResponseBody, serde_json::Value> {
    struct View<'a>(&'a tauri::Webview<MockRuntime>);
    impl AsRef<tauri::Webview<MockRuntime>> for View<'_> {
        fn as_ref(&self) -> &tauri::Webview<MockRuntime> {
            self.0
        }
    }
    tauri::test::get_ipc_response(
        &View(view),
        tauri::webview::InvokeRequest {
            cmd: cmd.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body: tauri::ipc::InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.into(),
        },
    )
}

#[test]
fn trusted_webview_labels_are_explicit() {
    for label in ["main", "about"] {
        assert!(event_sink::is_app_webview(label));
    }
    for label in [
        "browser-x",
        "browser-main",
        "browser-",
        "",
        "unknown",
        "Main",
    ] {
        assert!(!event_sink::is_app_webview(label));
    }
}

#[test]
fn multiwebview_commands_and_events() {
    let app = mock_builder()
        .invoke_handler(tauri::generate_handler![
            crate::chats::set_window_mode,
            crate::themes::window_set_material,
            crate::computer_commands::computer_window_probe,
        ])
        .build(mock_context(noop_assets()))
        .unwrap();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let main = app.get_webview("main").unwrap();
    let about = tauri::WebviewWindowBuilder::new(&app, "about", Default::default())
        .build()
        .unwrap();
    let native = window.as_ref().window();
    let browser = native
        .add_child(
            tauri::webview::WebviewBuilder::new("browser-x", Default::default()),
            tauri::LogicalPosition::new(0., 0.),
            tauri::LogicalSize::new(100., 100.),
        )
        .unwrap();
    assert!(app.get_webview_window("main").is_none());
    assert_eq!(main.window().label(), "main");

    let directory = std::env::temp_dir().join(format!("multiwebview-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let state = crate::state::AppState::from_data_dir(app.handle(), directory.clone());
    app.manage(state);
    for mode in ["work", "chat", "code"] {
        ipc(&main, "set_window_mode", json!({"mode":mode})).unwrap();
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(directory.join("settings.json")).unwrap())
                .unwrap();
        assert_eq!(saved["windowModes"]["main"], mode);
    }
    // Invalid material exits before AppKit (the mock has no NSWindow); the actual
    // command still extracts Window from the multiwebview IPC invocation.
    assert_eq!(
        ipc(&main, "window_set_material", json!({"material":"invalid"})).unwrap_err(),
        "Invalid window material"
    );
    ipc(&main, "computer_window_probe", json!({})).unwrap();
    assert_eq!(
        ipc(about.as_ref(), "computer_window_probe", json!({})).unwrap_err(),
        "Approve in the main app UI"
    );

    let received = Arc::new(Mutex::new(Vec::new()));
    for view in [&main, about.as_ref(), &browser] {
        for event in ["app-server-event", "terminal-output", "terminal-exit"] {
            let received = received.clone();
            let label = view.label().to_string();
            view.listen(event, move |_| {
                received.lock().unwrap().push((label.clone(), event))
            });
        }
    }
    let sink = event_sink::TauriEventSink::new(app.handle().clone());
    sink.emit_app_server_event(AppServerEvent {
        workspace_id: "w".into(),
        message: json!({"method":"test"}),
    });
    sink.emit_terminal_output(TerminalOutput {
        workspace_id: "w".into(),
        terminal_id: "t".into(),
        data: "secret".into(),
    });
    sink.emit_terminal_exit(TerminalExit {
        workspace_id: "w".into(),
        terminal_id: "t".into(),
    });
    let received = received.lock().unwrap();
    assert_eq!(received.len(), 6);
    assert_eq!(
        received.iter().filter(|(label, _)| label == "main").count(),
        3
    );
    assert_eq!(
        received
            .iter()
            .filter(|(label, _)| label == "about")
            .count(),
        3
    );
    assert!(!received
        .iter()
        .any(|(label, _)| label.starts_with("browser-")));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn no_single_webview_assumptions_in_app_sources() {
    fn inspect(path: &std::path::Path) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                inspect(&path);
                continue;
            }
            if path.extension().is_none_or(|extension| extension != "rs")
                || path.file_name().unwrap() == "multiwebview_tests.rs"
            {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            let compact: String = source.chars().filter(|c| !c.is_whitespace()).collect();
            assert!(
                !compact.contains("get_webview_window(\"main\")"),
                "{}",
                path.display()
            );
            assert!(
                !compact.contains(".webview_windows()"),
                "{}",
                path.display()
            );
            // Reject command parameter types even when written across lines.
            for command in source.split("#[tauri::command").skip(1) {
                let signature = command.split('{').next().unwrap();
                assert!(
                    !signature.contains("WebviewWindow"),
                    "{}: {signature}",
                    path.display()
                );
            }
        }
    }
    inspect(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"));
}

#[test]
fn window_events_leave_main_webview_geometry_to_tauri() {
    // Regression: resizing main on WindowEvent::Resized to Window::inner_size() pinned it
    // at its first size on macOS, so the transparent window resized and the UI did not.
    let lib = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"),
    )
    .unwrap();
    let handler = lib
        .split(".on_window_event(")
        .nth(1)
        .unwrap()
        .split(".setup(")
        .next()
        .unwrap()
        .to_lowercase();
    for banned in ["webview", "resize", "set_size", "set_bounds", "set_position"] {
        assert!(!handler.contains(banned), "on_window_event touches {banned}");
    }
}
