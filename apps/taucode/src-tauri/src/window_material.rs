// Adapted from tauri-plugin-liquid-glass 0.1.6 glass_effect/{operations,backend}.
// MIT, Copyright (c) 2025 hkandala; see window_material.LICENSE.
// Only the regular, untinted glass used by our themes is needed here.
#[cfg(target_os = "macos")]
mod macos {
    use objc2::{msg_send, rc::Retained, runtime::AnyClass};
    use objc2_app_kit::{NSView, NSWindow, NSWindowOrderingMode};
    use std::{cell::RefCell, collections::HashMap};

    thread_local! {
        static GLASS: RefCell<HashMap<String, Retained<NSView>>> = RefCell::new(HashMap::new());
    }

    pub(super) fn remove(label: &str) {
        GLASS.with(|views| {
            if let Some(view) = views.borrow_mut().remove(label) {
                view.removeFromSuperview();
            }
        });
    }

    pub(super) fn apply<R: tauri::Runtime>(
        window: &tauri::Window<R>,
        enabled: bool,
    ) -> Result<bool, String> {
        let window = window.clone();
        let dispatcher = window.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        dispatcher
            .run_on_main_thread(move || {
                let result = (|| unsafe {
                    let Some(class) = AnyClass::get(c"NSGlassEffectView") else {
                        return Ok(false);
                    };
                    if !enabled {
                        remove(window.label());
                        return Ok(false);
                    }
                    let native = window.ns_window().map_err(|e| e.to_string())?;
                    let native = &*native.cast::<NSWindow>();
                    let content = native.contentView().ok_or("Missing window content view")?;
                    GLASS.with(|views| -> Result<(), String> {
                        let mut views = views.borrow_mut();
                        if !views.contains_key(window.label()) {
                            let allocated: objc2::rc::Allocated<NSView> = msg_send![class, alloc];
                            let glass: Option<Retained<NSView>> =
                                msg_send![allocated, initWithFrame: content.bounds()];
                            let glass = glass.ok_or("Cannot create glass view")?;
                            glass.setAutoresizingMask(
                                objc2_app_kit::NSAutoresizingMaskOptions::ViewWidthSizable
                                    | objc2_app_kit::NSAutoresizingMaskOptions::ViewHeightSizable,
                            );
                            glass.setWantsLayer(true);
                            let layer = glass.layer().ok_or("Missing glass layer")?;
                            let _: () = msg_send![&*layer, setCornerRadius: 16.0_f64];
                            let _: () = msg_send![&*layer, setMasksToBounds: true];
                            // NSGlassEffectView defaults to regular, untinted glass.
                            views.insert(window.label().to_string(), glass);
                        }
                        let glass = &views[window.label()];
                        glass.setFrame(content.bounds());
                        // A sibling below every WKWebView, including browser children.
                        content.addSubview_positioned_relativeTo(
                            glass,
                            NSWindowOrderingMode::Below,
                            None,
                        );
                        Ok(())
                    })?;
                    Ok(true)
                })();
                let _ = tx.send(result);
            })
            .map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())?
    }

    pub(super) fn cleanup<R: tauri::Runtime>(window: &tauri::Window<R>) {
        let label = window.label().to_string();
        let _ = window.run_on_main_thread(move || remove(&label));
    }
}

pub(crate) fn set_glass<R: tauri::Runtime>(
    window: &tauri::Window<R>,
    enabled: bool,
) -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        macos::apply(window, enabled)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, enabled);
        Ok(false)
    }
}

pub(crate) fn cleanup<R: tauri::Runtime>(window: &tauri::Window<R>) {
    #[cfg(target_os = "macos")]
    macos::cleanup(window);
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}
