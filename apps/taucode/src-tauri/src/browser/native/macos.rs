use base64::Engine;
use block2::RcBlock;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSImage};
use objc2_foundation::{NSDictionary, NSError, NSString};
use objc2_web_kit::{WKContentWorld, WKWebView};
use serde_json::{json, Value};
use std::sync::Mutex;
use tauri::Webview;
thread_local! {
    // Retain the named world for the app lifetime so refs survive separate calls.
    static WORLD: objc2::rc::Retained<WKContentWorld> = unsafe { WKContentWorld::worldWithName(&NSString::from_str("taucode.browser.private"), objc2::MainThreadMarker::new().expect("WebKit main thread")) };
}

pub async fn evaluate(view: &Webview, script: String) -> Result<Value, String> {
    if objc2_foundation::NSProcessInfo::processInfo()
        .operatingSystemVersion()
        .majorVersion
        < 11
    {
        return Err("Native agent control requires macOS 11. Try the Headless Chromium backend (Settings → Browser)".into());
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    let tx = Mutex::new(Some(tx));
    view.with_webview(move |platform| unsafe {
        let wk = &*(platform.inner() as *const WKWebView);
        let callback = RcBlock::new(move |value: *mut AnyObject, error: *mut NSError| {
            let result = if let Some(error) = error.as_ref() {
                Err(error.localizedDescription().to_string())
            } else if value.is_null() {
                Ok(Value::Null)
            } else if let Some(string) = (&*value).downcast_ref::<NSString>() {
                serde_json::from_str(&string.to_string()).map_err(|e| e.to_string())
            } else {
                Err("Unexpected native bridge response".into())
            };
            if let Some(tx) = tx.lock().unwrap().take() {
                let _ = tx.send(result);
            }
        });
        WORLD.with(|world| {
            wk.evaluateJavaScript_inFrame_inContentWorld_completionHandler(
                &NSString::from_str(&script),
                None,
                world,
                Some(&callback),
            )
        });
    })
    .map_err(|e| e.to_string())?;
    tokio::time::timeout(std::time::Duration::from_secs(10), rx)
        .await
        .map_err(|_| "Native page did not respond; reload the tab")?
        .map_err(|_| "Native view closed")?
}

pub async fn script(view: &Webview, params: Value) -> Result<Value, String> {
    let result = evaluate(
        view,
        format!(
            "{}\nglobalThis.__tauBrowser({params})",
            include_str!("bridge.js")
        ),
    )
    .await?;
    if let Some(error) = result["error"].as_str() {
        Err(error.into())
    } else {
        Ok(result["result"].clone())
    }
}

pub async fn image(view: &Webview) -> Result<Value, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let tx = Mutex::new(Some(tx));
    view.with_webview(move |platform| unsafe {
        let wk = &*(platform.inner() as *const WKWebView);
        let callback = RcBlock::new(move |image: *mut NSImage, error: *mut NSError| {
            let result = (|| {
                if let Some(error) = error.as_ref() {
                    return Err(error.localizedDescription().to_string());
                }
                let tiff = image
                    .as_ref()
                    .and_then(|i| i.TIFFRepresentation())
                    .ok_or("No native snapshot")?;
                let bitmap = NSBitmapImageRep::imageRepWithData(&tiff)
                    .ok_or("Cannot decode native snapshot")?;
                let data = bitmap
                    .representationUsingType_properties(
                        NSBitmapImageFileType::PNG,
                        &NSDictionary::new(),
                    )
                    .ok_or("Cannot encode native snapshot")?;
                Ok(json!(
                    base64::engine::general_purpose::STANDARD.encode(data.to_vec())
                ))
            })();
            if let Some(tx) = tx.lock().unwrap().take() {
                let _ = tx.send(result);
            }
        });
        wk.takeSnapshotWithConfiguration_completionHandler(None, &callback);
    })
    .map_err(|e| e.to_string())?;
    tokio::time::timeout(std::time::Duration::from_secs(3), rx)
        .await
        .map_err(|_| "Native snapshot timed out")?
        .map_err(|_| "Native view closed")?
}

pub async fn control(view: &Webview, action: String) -> Result<Value, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    view.with_webview(move |platform| unsafe {
        let wk = &*(platform.inner() as *const WKWebView);
        match action.as_str() { "back" => { wk.goBack(); }, "forward" => { wk.goForward(); }, "stop" => wk.stopLoading(), _ => () }
        let _ = tx.send(json!({"canGoBack":wk.canGoBack(),"canGoForward":wk.canGoForward(),"progress":wk.estimatedProgress()}));
    }).map_err(|e|e.to_string())?;
    rx.await.map_err(|_| "Native view closed".into())
}

pub fn persistent_store_available() -> bool {
    // data_store_identifier is only supported on macOS 14+.
    let version = objc2_foundation::NSProcessInfo::processInfo().operatingSystemVersion();
    version.majorVersion >= 14
}

/// WebKit groups website records by registrable domain. Clear only records
/// belonging to this host (including its parent domain), never the whole store.
pub async fn clear_site(view: &Webview, host: String) -> Result<(), String> {
    use objc2_foundation::NSArray;
    use objc2_web_kit::{WKWebsiteDataRecord, WKWebsiteDataStore};
    let (tx, rx) = tokio::sync::oneshot::channel();
    let tx = std::sync::Arc::new(Mutex::new(Some(tx)));
    view.with_webview(move |platform| unsafe {
        let wk = &*(platform.inner() as *const WKWebView);
        let store = wk.configuration().websiteDataStore();
        let types = WKWebsiteDataStore::allWebsiteDataTypes(
            objc2::MainThreadMarker::new().expect("WebKit main thread"),
        );
        let fetch_store = store.clone();
        let fetch_types = types.clone();
        let callback = RcBlock::new(
            move |records: std::ptr::NonNull<NSArray<WKWebsiteDataRecord>>| {
                let records: Vec<_> = records
                    .as_ref()
                    .iter()
                    .filter(|record| {
                        let name = record.displayName().to_string();
                        host == name || host.ends_with(&format!(".{name}"))
                    })
                    .collect();
                let selected = NSArray::from_retained_slice(&records);
                let tx = tx.clone();
                let done = RcBlock::new(move || {
                    if let Some(tx) = tx.lock().unwrap().take() {
                        let _ = tx.send(());
                    }
                });
                fetch_store.removeDataOfTypes_forDataRecords_completionHandler(
                    &fetch_types,
                    &selected,
                    &done,
                );
            },
        );
        store.fetchDataRecordsOfTypes_completionHandler(&types, &callback);
    })
    .map_err(|e| e.to_string())?;
    tokio::time::timeout(std::time::Duration::from_secs(10), rx)
        .await
        .map_err(|_| "Clearing site data timed out")?
        .map_err(|_| "Native view closed".into())
}
