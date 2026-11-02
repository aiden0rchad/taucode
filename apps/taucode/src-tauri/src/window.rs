use tauri::Manager;

// Capture before AppKit teardown; Exit can report a 1x scale on a Retina
// window. The plugin writes at Exit, so merge this live snapshot afterwards.
#[cfg(desktop)]
pub(crate) fn capture_main_state<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<serde_json::Map<String, serde_json::Value>> {
    let mut state = serde_json::Map::new();
    let Some(window) = app.get_window("main") else { return Ok(state); };
    let maximized = window.is_maximized()?;
    let fullscreen = window.is_fullscreen()?;
    for (key, value) in [
        ("maximized", maximized), ("fullscreen", fullscreen),
        ("visible", window.is_visible()?), ("decorated", window.is_decorated()?),
    ] { state.insert(key.into(), value.into()); }
    if !maximized && !fullscreen && !window.is_minimized()? {
        let scale = window.scale_factor()?;
        let size = window.inner_size()?.to_logical::<f64>(scale);
        let position = window.outer_position()?.to_logical::<f64>(scale);
        if size.width > 0. && size.height > 0. {
            state.insert("width".into(), size.width.round().into());
            state.insert("height".into(), size.height.round().into());
            state.insert("logical".into(), true.into());
        }
        for key in ["x", "prev_x"] { state.insert(key.into(), position.x.round().into()); }
        for key in ["y", "prev_y"] { state.insert(key.into(), position.y.round().into()); }
    }
    Ok(state)
}

// Size and position are saved here in logical points, never by the plugin: its physical
// values come back at half size when a save happened at a 1x scale (a SIGTERM quit).
#[cfg(desktop)]
pub(crate) fn restore_main_bounds<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_window_state::AppHandleExt;
    let Some(window) = app.get_window("main") else { return Ok(()); };
    let Some(config) = app.config().app.windows.iter().find(|w| w.label == "main") else { return Ok(()); };
    let path = app.path().app_config_dir()?.join(app.filename());
    let Ok(bytes) = std::fs::read(path) else { return Ok(()); };
    let states: serde_json::Value = serde_json::from_slice(&bytes)?;
    let main = &states["main"];
    if main["logical"] != true { return Ok(()); }
    let number = |key: &str| main[key].as_f64();
    let (Some(width), Some(height)) = (number("width"), number("height")) else { return Ok(()); };
    if width < config.min_width.unwrap_or(0.) || height < config.min_height.unwrap_or(0.) { return Ok(()); }
    // Move first: sizing on the launch screen and then moving to a screen with another
    // scale converts the size wrongly.
    if let (Some(x), Some(y)) = (number("x"), number("y")) {
        // Only onto a screen that is still attached; otherwise macOS places it.
        let on_screen = window.available_monitors()?.iter().any(|m| {
            let (scale, origin, size) = (m.scale_factor(), m.position(), m.size());
            let (left, top) = (origin.x as f64 / scale, origin.y as f64 / scale);
            x >= left && y >= top && x < left + size.width as f64 / scale && y < top + size.height as f64 / scale
        });
        if on_screen { window.set_position(tauri::LogicalPosition::new(x, y))?; }
    }
    window.set_size(tauri::LogicalSize::new(width, height))?;
    Ok(())
}

// window-state omits main from its exit snapshot after add_child. Preserve
// its other window records and supplement only main with the live snapshot.
#[cfg(desktop)]
pub(crate) fn save_main_state<R: tauri::Runtime>(app: &tauri::AppHandle<R>, snapshot: serde_json::Map<String, serde_json::Value>) -> Result<(), String> {
    use tauri_plugin_window_state::AppHandleExt;
    let save = || -> Result<(), Box<dyn std::error::Error>> {
        let path = app.path().app_config_dir()?.join(app.filename());
        let mut states: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
        states.as_object_mut().ok_or("Invalid window state file")?
            .entry("main").or_insert_with(|| serde_json::json!({}))
            .as_object_mut().ok_or("Invalid main window state")?.extend(snapshot);
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
