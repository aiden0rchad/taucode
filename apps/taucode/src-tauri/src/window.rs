use tauri::Manager;

// window-state 2.4.1 restores and tracks geometry through Window, but its exit
// snapshot enumerates WebviewWindows and misses main after add_child. Tauri runs
// plugin Exit callbacks before our app callback: supplement that same file here.
#[cfg(desktop)]
pub(crate) fn save_main_state<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<(), String> {
    use tauri_plugin_window_state::AppHandleExt;
    let Some(window) = app.get_window("main") else {
        return Ok(());
    };
    let save = || -> Result<(), Box<dyn std::error::Error>> {
        let path = app.path().app_config_dir()?.join(app.filename());
        let mut states: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
        let state = states
            .get_mut("main")
            .and_then(|s| s.as_object_mut())
            .ok_or("Missing main window state")?;
        let maximized = window.is_maximized()?;
        let fullscreen = window.is_fullscreen()?;
        for (key, value) in [
            ("maximized", maximized),
            ("fullscreen", fullscreen),
            ("visible", window.is_visible()?),
            ("decorated", window.is_decorated()?),
        ] {
            state.insert(key.into(), value.into());
        }
        if !maximized && !fullscreen && !window.is_minimized()? {
            let size = window.inner_size()?;
            let position = window.outer_position()?;
            if size.width > 0 && size.height > 0 {
                state.insert("width".into(), size.width.into());
                state.insert("height".into(), size.height.into());
            }
            for key in ["x", "prev_x"] {
                state.insert(key.into(), position.x.into());
            }
            for key in ["y", "prev_y"] {
                state.insert(key.into(), position.y.into());
            }
        }
        let temporary = path.with_extension("tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(&states)?)?;
        std::fs::rename(temporary, path)?;
        Ok(())
    };
    save().map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
fn zoom_shortcut(key: &str, command: bool, control: bool, alt: bool) -> Option<&'static str> {
    if (!command && !control) || alt { return None; }
    match key {
        "+" | "=" => Some("zoom_in"),
        "-" | "_" => Some("zoom_out"),
        "0" => Some("zoom_reset"),
        _ => None,
    }
}

#[cfg(all(test, target_os = "macos"))]
#[test]
fn zoom_shortcuts_preserve_aliases_and_unmodified_browser_input() {
    for (key, action) in [("+", "zoom_in"), ("=", "zoom_in"), ("-", "zoom_out"), ("_", "zoom_out"), ("0", "zoom_reset")] {
        assert_eq!(zoom_shortcut(key, true, false, false), Some(action));
        assert_eq!(zoom_shortcut(key, false, true, false), Some(action));
        assert_eq!(zoom_shortcut(key, false, false, false), None);
        assert_eq!(zoom_shortcut(key, true, false, true), None);
    }
    assert_eq!(zoom_shortcut("a", true, false, false), None);
}

#[cfg(target_os = "macos")]
thread_local! {
    static FOCUS_MONITOR: std::cell::RefCell<Option<objc2::rc::Retained<objc2::runtime::AnyObject>>> = const { std::cell::RefCell::new(None) };
}

#[cfg(target_os = "macos")]
pub(crate) fn install_main_focus(webview: &tauri::Webview) -> tauri::Result<()> {
    let app = webview.app_handle().clone();
    webview.with_webview(move |platform| unsafe {
        use objc2::{rc::Retained, MainThreadMarker};
        use objc2_app_kit::{NSApplication, NSEvent, NSEventMask, NSEventModifierFlags, NSEventType, NSView};
        use tauri::Emitter;
        let main = Retained::retain(platform.inner().cast::<NSView>()).expect("main WKWebView");
        let handler = block2::RcBlock::new(move |event: std::ptr::NonNull<NSEvent>| {
            let event_ref = event.as_ref();
            // Resolve the actual topmost hit: a browser child must retain its focus.
            if let Some(window) = main.window() {
                if event_ref.window(MainThreadMarker::new().expect("AppKit main thread"))
                    == Some(window.clone())
                {
                    if event_ref.r#type() == NSEventType::KeyDown {
                        let modifiers = event_ref.modifierFlags();
                        let command = modifiers.contains(NSEventModifierFlags::Command);
                        let control = modifiers.contains(NSEventModifierFlags::Control);
                        let alt = modifiers.contains(NSEventModifierFlags::Option);
                        let key = event_ref.charactersIgnoringModifiers().map(|s| s.to_string()).unwrap_or_default();
                        // Child WKWebViews can consume menu equivalents before AppKit.
                        // Handle zoom once here, including the shifted + / _ aliases.
                        if let Some(action) = zoom_shortcut(&key, command, control, alt) {
                            let _ = app.emit_to(tauri::EventTarget::webview("main"), "menu-view-action", action);
                            return std::ptr::null_mut();
                        }
                        if command && NSApplication::sharedApplication(MainThreadMarker::new().expect("AppKit main thread"))
                            .mainMenu().is_some_and(|menu| menu.performKeyEquivalent(event_ref)) {
                            return std::ptr::null_mut();
                        }
                        return event.as_ptr();
                    }
                    if let Some(content) = window.contentView() {
                        let point =
                            content.convertPoint_fromView(event_ref.locationInWindow(), None);
                        if content
                            .hitTest(point)
                            .is_some_and(|hit| hit.isDescendantOf(&main))
                        {
                            window.makeFirstResponder(Some(&main));
                        }
                    }
                }
            }
            event.as_ptr()
        });
        let _ = MainThreadMarker::new().expect("WebKit main thread");
        FOCUS_MONITOR.with(|monitor| {
            if let Some(old) = monitor.borrow_mut().take() {
                NSEvent::removeMonitor(&old);
            }
            *monitor.borrow_mut() = NSEvent::addLocalMonitorForEventsMatchingMask_handler(
                NSEventMask::LeftMouseDown | NSEventMask::KeyDown,
                &handler,
            );
        });
    })
}

#[cfg(target_os = "macos")]
pub(crate) fn remove_main_focus<R: tauri::Runtime>(window: &tauri::Window<R>) {
    let _ = window.run_on_main_thread(|| {
        FOCUS_MONITOR.with(|monitor| {
            if let Some(monitor) = monitor.borrow_mut().take() {
                unsafe {
                    objc2_app_kit::NSEvent::removeMonitor(&monitor);
                }
            }
        })
    });
}

#[cfg(target_os = "ios")]
pub(crate) fn configure_ios_webview_edge_to_edge(
    webview_window: &tauri::Webview,
) -> Result<(), String> {
    use objc2::runtime::AnyObject;

    webview_window
        .with_webview(|webview| unsafe {
            let wk_webview = webview.inner().cast::<AnyObject>();
            if !wk_webview.is_null() {
                let scroll_view: *mut AnyObject = objc2::msg_send![wk_webview, scrollView];
                if !scroll_view.is_null() {
                    // UIScrollViewContentInsetAdjustmentNever
                    let adjustment_never: isize = 2;
                    let () = objc2::msg_send![
                        scroll_view,
                        setContentInsetAdjustmentBehavior: adjustment_never
                    ];
                    let () = objc2::msg_send![
                        scroll_view,
                        setAutomaticallyAdjustsScrollIndicatorInsets: false
                    ];
                }
            }

            let view_controller = webview.view_controller().cast::<AnyObject>();
            if !view_controller.is_null() {
                // UIRectEdgeAll
                let all_edges: usize = 15;
                let () = objc2::msg_send![view_controller, setEdgesForExtendedLayout: all_edges];
                let () = objc2::msg_send![
                    view_controller,
                    setExtendedLayoutIncludesOpaqueBars: true
                ];
            }
        })
        .map_err(|error| error.to_string())
}
