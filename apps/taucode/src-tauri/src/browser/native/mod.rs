#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
mod navigation_events;
pub mod policy;

use super::backend::BrowserBackend;
use futures_util::future::BoxFuture;
use policy::{label, navigation, Navigation, Rect};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{
    webview::{DownloadEvent, NewWindowResponse, PageLoadEvent},
    AppHandle, LogicalPosition, LogicalSize, Manager, Webview, WebviewBuilder, WebviewUrl,
};
use tokio::sync::watch;

const STORE: [u8; 16] = *b"taucode-browser1";
const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15";
struct Views {
    live: Vec<String>,
    shown: Option<String>,
    rect: Option<Rect>,
    visible: bool,
    last_poll: Instant,
    private: bool,
}
pub struct NativeBackend {
    app: AppHandle,
    state: watch::Sender<Value>,
    views: tokio::sync::Mutex<Views>,
    downloads: Arc<Mutex<HashSet<String>>>,
    settings_path: std::path::PathBuf,
}
impl NativeBackend {
    pub fn new(
        app: AppHandle,
        state: watch::Sender<Value>,
        settings_path: std::path::PathBuf,
    ) -> Self {
        Self {
            app,
            state,
            views: tokio::sync::Mutex::new(Views {
                live: vec![],
                shown: None,
                rect: None,
                visible: false,
                last_poll: Instant::now(),
                private: false,
            }),
            downloads: Arc::default(),
            settings_path,
        }
    }
    fn settings(&self) -> Result<super::Settings, String> {
        Ok(crate::storage::read_settings(&self.settings_path)?.browser)
    }
    fn active(&self) -> String {
        self.state.borrow()["active"].as_str().unwrap_or("").into()
    }
    fn tab(&self, id: &str) -> Option<Value> {
        self.state.borrow()["tabs"]
            .as_array()?
            .iter()
            .find(|t| t["id"] == id)
            .cloned()
    }
    fn update(&self, id: &str, values: Value) {
        update(&self.state, id, values);
    }
    fn add_tab(&self, url: &str) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.state.send_modify(|s| {
            if !s["tabs"].is_array() {
                s["tabs"] = json!([]);
            }
            s["tabs"]
                .as_array_mut()
                .unwrap()
                .push(json!({"id":id,"url":url,"title":"New tab","loading":true,"ready":false}));
            s["active"] = json!(id);
        });
        id
    }
    async fn hide(&self, views: &mut Views) -> Result<(), String> {
        if let Some(id) = views.shown.take() {
            if let Some(view) = self.app.get_webview(&label(&id)?) {
                #[cfg(target_os = "macos")]
                if self.tab(&id).is_some_and(|t| t["error"].is_null()) {
                    if let Ok(image) = macos::image(&view).await {
                        self.update(&id, json!({"snapshot":image}));
                    }
                }
                view.hide().map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
    async fn view(&self, id: &str, views: &mut Views) -> Result<Webview, String> {
        let name = label(id)?;
        if let Some(view) = self.app.get_webview(&name) {
            policy::touch(&mut views.live, id);
            return Ok(view);
        }
        let tab = self.tab(id).ok_or("Unknown browser tab")?;
        let url = tab["url"].as_str().unwrap_or("about:blank");
        if navigation(url, url) != Navigation::Allow {
            return Err("Navigation blocked".into());
        }
        if let Some(evicted) = policy::touch(&mut views.live, id) {
            if let Some(view) = self.app.get_webview(&label(&evicted)?) {
                close_view(view)?;
            }
            self.update(&evicted, json!({"ready":false}));
        }
        let settings = self.settings()?;
        let state = self.state.clone();
        let tab_id = id.to_string();
        let title_state = self.state.clone();
        let title_id = id.to_string();
        let nav_state = self.state.clone();
        let nav_id = id.to_string();
        let new_state = self.state.clone();
        let download_state = self.state.clone();
        let downloads = self.downloads.clone();
        let downloads_dir = self.app.path().download_dir().map_err(|e| e.to_string())?;
        let mut builder = WebviewBuilder::new(&name, WebviewUrl::External(url.parse().map_err(|_|"Invalid URL")?))
            .user_agent(UA).devtools(cfg!(debug_assertions)).focused(false)
            .background_color(tauri::window::Color(255,255,255,255))
            .incognito(settings.private_browsing)
            .on_navigation(move |url| {
                let current = nav_state.borrow()["tabs"].as_array().and_then(|tabs|tabs.iter().find(|t|t["id"] == nav_id)).and_then(|t|t["url"].as_str()).unwrap_or("about:blank").to_string();
                match navigation(url.as_str(), &current) {
                    Navigation::Allow if allowed_origin(url, &settings) => { update(&nav_state,&nav_id,json!({"url":url.as_str()})); true },
                    Navigation::External => { nav_state.send_modify(|s|s["externalRequest"] = json!(url.as_str())); false },
                    _ => { update(&nav_state,&nav_id,json!({"error":"Navigation blocked by browser policy"})); false },
                }
            })
            .on_page_load(move |_view, payload| {
                let finished = matches!(payload.event(),PageLoadEvent::Finished);
                update(&state,&tab_id,json!({"url":payload.url().as_str(),"loading":!finished,"ready":finished,"progress":if finished {1.} else {0.1},"error":null}));
            })
            .on_document_title_changed(move |_view,title|update(&title_state,&title_id,json!({"title":title})))
            .on_new_window(move |url,_| {
                match navigation(url.as_str(),url.as_str()) {
                    Navigation::Allow => { new_state.send_modify(|s|s["newWindow"] = json!(url.as_str())); },
                    Navigation::External => { new_state.send_modify(|s|s["externalRequest"] = json!(url.as_str())); },
                    Navigation::Block => (),
                }
                NewWindowResponse::Deny
            })
            .on_download(move |view,event| match event {
                DownloadEvent::Requested { url,destination } => {
                    update(&download_state, view.label().trim_start_matches("browser-"), json!({"ready":true,"loading":false,"error":null}));
                    if downloads.lock().unwrap().remove(url.as_str()) {
                        let name = destination.file_name().unwrap_or(std::ffi::OsStr::new("download"));
                        let mut path = downloads_dir.join(name);
                        if path.exists() { path = downloads_dir.join(format!("{}-{}",uuid::Uuid::new_v4(),name.to_string_lossy())); }
                        *destination = path; true
                    } else { download_state.send_modify(|s|s["downloadRequest"] = json!(url.as_str())); false }
                },
                DownloadEvent::Finished {url,success,..} => { download_state.send_modify(|s|s["download"] = json!({"url":url.as_str(),"success":success})); true },
                _ => false,
            });
        #[cfg(target_os = "macos")]
        {
            builder = if macos::persistent_store_available() && !views.private {
                builder.data_store_identifier(STORE)
            } else {
                builder.incognito(true)
            };
        }
        #[cfg(target_os = "windows")]
        {
            builder = builder.data_directory(
                self.settings_path
                    .parent()
                    .ok_or("Missing data directory")?
                    .join("browser/native-webview2"),
            );
        }
        let window = self
            .app
            .get_window("main")
            .ok_or("Main window unavailable")?;
        // Create offscreen: no white flash before first load or before hide is processed.
        let view = window
            .add_child(
                builder,
                LogicalPosition::new(-32000., -32000.),
                LogicalSize::new(800., 600.),
            )
            .map_err(|e| e.to_string())?;
        view.hide().map_err(|e| e.to_string())?;
        #[cfg(target_os = "macos")]
        navigation_events::install(&view, id.to_string(), self.state.clone())?;
        Ok(view)
    }
    async fn place(&self, views: &mut Views) -> Result<(), String> {
        let id = self.active();
        let window = self
            .app
            .get_window("main")
            .ok_or("Main window unavailable")?;
        let visible = views.visible
            && views.rect.is_some()
            && window.is_visible().unwrap_or(false)
            && !window.is_minimized().unwrap_or(true);
        let ready = self
            .tab(&id)
            .is_some_and(|t| t["ready"] == true && t["error"].is_null());
        if !visible || !ready || views.shown.as_deref().is_some_and(|shown| shown != id) {
            self.hide(views).await?;
        }
        if !visible || id.is_empty() {
            return Ok(());
        }
        let view = self.view(&id, views).await?;
        if self
            .tab(&id)
            .is_some_and(|t| t["ready"] == true && t["error"].is_null())
        {
            let rect = views.rect.unwrap();
            view.set_bounds(tauri::Rect {
                position: LogicalPosition::new(rect.x, rect.y).into(),
                size: LogicalSize::new(rect.w, rect.h).into(),
            })
            .map_err(|e| e.to_string())?;
            if views.shown.as_deref() != Some(&id) {
                view.show().map_err(|e| e.to_string())?;
                views.shown = Some(id);
            }
        }
        Ok(())
    }
    async fn command(&self, p: Value) -> Result<Value, String> {
        let mut views = self.views.lock().await;
        self.state.send_if_modified(|s| {
            if s["backend"] == "native" {
                false
            } else {
                s["backend"] = json!("native");
                true
            }
        });
        let settings = self.settings()?;
        if views.private != settings.private_browsing {
            self.hide(&mut views).await?;
            for id in views.live.drain(..) {
                if let Some(view) = self.app.get_webview(&label(&id)?) {
                    close_view(view)?;
                }
                self.update(&id, json!({"ready":false}));
            }
            views.private = settings.private_browsing;
        }
        let action = p["action"].as_str().ok_or("Missing browser action")?;
        if action == "ensure" {
            return Ok(json!({"native":cfg!(target_os = "macos")}));
        }
        // Process popup requests for agent-only sessions too, without requiring a pane subscription.
        {
            let new_url = self.state.borrow()["newWindow"].as_str().map(String::from);
            if let Some(url) = new_url {
                self.state.send_modify(|s| s["newWindow"] = Value::Null);
                self.hide(&mut views).await?;
                let id = self.add_tab(&url);
                self.view(&id, &mut views).await?;
            }
        }
        if action == "poll" {
            if views.last_poll.elapsed() > Duration::from_millis(250) {
                views.last_poll = Instant::now();
                let id = self.active();
                #[cfg(target_os = "macos")]
                if let Some(view) = label(&id).ok().and_then(|name| self.app.get_webview(&name)) {
                    if let Ok(status) = macos::control(&view, "status".into()).await {
                        self.update(&id, status);
                    }
                    if self
                        .tab(&id)
                        .is_some_and(|t| t["ready"] == true && t["error"].is_null())
                    {
                        match macos::script(&view, json!({"op":"meta"})).await {
                            Ok(meta) => {
                                if meta["userInput"] == true && self.state.borrow()["agent"] == true
                                {
                                    self.state.send_modify(|s| {
                                        s["agent"] = json!(false);
                                        s["takeover"] =
                                            json!(s["takeover"].as_u64().unwrap_or(0) + 1);
                                    });
                                }
                                self.update(&id, meta);
                            }
                            Err(error) => {
                                self.update(&id,json!({"error":format!("Page unavailable: {error}. Reload to recover.")}));
                                self.hide(&mut views).await?;
                            }
                        }
                    }
                }
            }
            self.place(&mut views).await?;
            return Ok(self.state.borrow().clone());
        }
        if action == "activity" {
            self.state.send_modify(|s| s["agent"] = p["active"].clone());
            return Ok(Value::Null);
        }
        if action == "detach" {
            views.visible = false;
            self.hide(&mut views).await?;
            return Ok(Value::Null);
        }
        if action == "reset" {
            self.hide(&mut views).await?;
            for id in views.live.drain(..) {
                if let Some(view) = self.app.get_webview(&label(&id)?) {
                    close_view(view)?;
                }
            }
            self.state
                .send_replace(json!({"tabs":[],"active":"","agent":false}));
            return Ok(Value::Null);
        }
        if action == "clear" {
            if p["confirmed"] != true {
                return Err("Confirm clearing browsing data first".into());
            }
            self.hide(&mut views).await?;
            // Materialize a view so persistent storage can also be cleared after restart.
            if self.active().is_empty() {
                self.add_tab("about:blank");
            }
            let view = self.view(&self.active(), &mut views).await?;
            view.clear_all_browsing_data().map_err(|e| e.to_string())?;
            for id in views.live.drain(..) {
                if let Some(view) = self.app.get_webview(&label(&id)?) {
                    close_view(view)?;
                }
            }
            self.state
                .send_replace(json!({"backend":"native","tabs":[],"active":"","agent":false}));
            return Ok(Value::Null);
        }
        if action == "attach" || action == "bounds" || action == "resize" {
            if let Some(id) = p["tabId"].as_str() {
                label(id)?;
                if self.tab(id).is_none() {
                    return Err("Unknown browser tab".into());
                }
                if id != self.active() {
                    return Ok(Value::Null);
                }
            }
            if !p["rect"].is_null() {
                views.rect = Some(
                    serde_json::from_value::<Rect>(p["rect"].clone())
                        .map_err(|e| e.to_string())?
                        .validate()?,
                );
            }
            if let Some(visible) = p["visible"].as_bool() {
                views.visible = visible;
            }
            self.place(&mut views).await?;
            return Ok(Value::Null);
        }
        let mut action = action;
        let mut id = p["id"].as_str().map(String::from);
        if action == "tabs" {
            action = match p["params"]["action"].as_str().unwrap_or("list") {
                "list" => return Ok(self.state.borrow()["tabs"].clone()),
                "new" => "new",
                "select" => "activate",
                "close" => "close",
                _ => return Err("Unknown tabs action".into()),
            };
            if let Some(index) = p["params"]["index"].as_u64() {
                id = Some(
                    self.state.borrow()["tabs"][index as usize]["id"]
                        .as_str()
                        .ok_or("Unknown tab index")?
                        .into(),
                );
            }
        }
        if action == "new" {
            self.hide(&mut views).await?;
            let url = p["url"].as_str().unwrap_or("about:blank");
            if navigation(url, url) != Navigation::Allow {
                return Err("Navigation blocked".into());
            }
            let id = self.add_tab(url);
            self.view(&id, &mut views).await?;
            self.place(&mut views).await?;
            return Ok(json!({"id":id}));
        }
        if action == "activate" {
            let id = id.ok_or("Missing tab ID")?;
            label(&id)?;
            if self.tab(&id).is_none() {
                return Err("Unknown browser tab".into());
            }
            self.hide(&mut views).await?;
            self.state.send_modify(|s| s["active"] = json!(id));
            self.place(&mut views).await?;
            return Ok(Value::Null);
        }
        if action == "close" {
            let id = id.unwrap_or_else(|| self.active());
            label(&id)?;
            if views.shown.as_deref() == Some(&id) {
                self.hide(&mut views).await?;
            }
            if let Some(view) = self.app.get_webview(&label(&id)?) {
                close_view(view)?;
            }
            views.live.retain(|v| v != &id);
            self.state.send_modify(|s| {
                if let Some(tabs) = s["tabs"].as_array_mut() {
                    tabs.retain(|t| t["id"] != id);
                }
                if s["active"] == id {
                    s["active"] = s["tabs"][0]["id"]
                        .as_str()
                        .map(|s| json!(s))
                        .unwrap_or(json!(""));
                }
            });
            self.place(&mut views).await?;
            return Ok(Value::Null);
        }
        if action == "dismiss" {
            for key in ["externalRequest", "newWindow", "downloadRequest"] {
                if p["key"] == key {
                    self.state.send_modify(|s| s[key] = Value::Null);
                }
            }
            return Ok(Value::Null);
        }
        if self.active().is_empty() {
            self.add_tab("about:blank");
        }
        let id = self.active();
        let view = self.view(&id, &mut views).await?;
        match action {
            "navigate" | "download" => {
                let url = p["url"].as_str().ok_or("Missing URL")?;
                if navigation(
                    url,
                    self.tab(&id)
                        .as_ref()
                        .and_then(|t| t["url"].as_str())
                        .unwrap_or(""),
                ) != Navigation::Allow
                {
                    return Err("Navigation blocked".into());
                }
                if action == "download" {
                    if p["confirmed"] != true || self.state.borrow()["downloadRequest"] != url {
                        return Err("Confirm download first".into());
                    }
                    self.downloads.lock().unwrap().insert(url.into());
                    self.state
                        .send_modify(|s| s["downloadRequest"] = Value::Null);
                }
                if action == "navigate" {
                    self.update(&id, json!({"ready":false,"loading":true,"error":null}));
                }
                view.navigate(url.parse().map_err(|_| "Invalid URL")?)
                    .map_err(|e| e.to_string())?;
            }
            "reload" => {
                self.update(&id, json!({"error":null,"ready":false}));
                view.reload().map_err(|e| e.to_string())?;
            }
            "zoom" => {
                let zoom = p["value"]
                    .as_f64()
                    .filter(|v| (0.25..=5.).contains(v))
                    .ok_or("Invalid zoom")?;
                view.set_zoom(zoom).map_err(|e| e.to_string())?;
            }
            "clearSite" => {
                if p["confirmed"] != true {
                    return Err("Confirm clearing site data first".into());
                }
                #[cfg(target_os = "macos")]
                {
                    let url = view.url().map_err(|e| e.to_string())?;
                    macos::clear_site(
                        &view,
                        url.host_str().ok_or("This page has no site data")?.into(),
                    )
                    .await?;
                }
                #[cfg(target_os = "windows")]
                return Err("Use Settings → Browser → Clear browsing data on Windows".into());
                #[cfg(target_os = "macos")]
                view.reload().map_err(|e| e.to_string())?;
            }
            #[cfg(target_os = "macos")]
            "back" | "forward" | "stop" => {
                return macos::control(&view, action.into()).await;
            }
            #[cfg(target_os = "macos")]
            "script" => {
                return macos::script(&view, p).await;
            }
            #[cfg(target_os = "macos")]
            "screenshot" => {
                return macos::image(&view).await;
            }
            #[cfg(target_os = "macos")]
            "wait" => {
                drop(views);
                let deadline = Instant::now() + Duration::from_secs(20);
                let time = p["time"].as_f64().unwrap_or(0.);
                if !(0. ..=20.).contains(&time) {
                    return Err("Wait must be between 0 and 20 seconds".into());
                }
                tokio::time::sleep(Duration::from_secs_f64(time)).await;
                loop {
                    if macos::script(&view,json!({"op":"wait","text":p["text"],"textGone":p["textGone"],"ref":p["ref"]})).await? == true { return Ok(json!(true)); }
                    if Instant::now() >= deadline {
                        return Err("Wait timed out. Try the Headless Chromium backend (Settings → Browser)".into());
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
            #[cfg(target_os = "windows")]
            "back" | "forward" | "stop" => {
                view.eval(match action {
                    "back" => "history.back()",
                    "forward" => "history.forward()",
                    _ => "window.stop()",
                })
                .map_err(|e| e.to_string())?;
            }
            _ => return Err("Try the Headless Chromium backend (Settings → Browser)".into()),
        }
        self.place(&mut views).await?;
        Ok(Value::Null)
    }
}
impl BrowserBackend for NativeBackend {
    fn navigate(&self, url: &str) -> BoxFuture<'_, Result<Value, String>> {
        let url = url.to_string();
        Box::pin(async move {
            self.command(json!({"action":"navigate","url":url})).await?;
            let id = self.active();
            let mut events = self.state.subscribe();
            tokio::time::timeout(Duration::from_secs(20), async {
                loop {
                    let tab = self.tab(&id).ok_or("Browser tab was closed")?;
                    if let Some(error) = tab["error"].as_str() { return Err(error.to_string()); }
                    if tab["ready"] == true && (url == "about:blank" || tab["url"] != "about:blank") { return Ok(tab); }
                    events.changed().await.map_err(|_|"Native browser closed")?;
                }
            }).await.map_err(|_|"Page load timed out; retry or use the Headless Chromium backend (Settings → Browser)".to_string())?
        })
    }
    fn request(&self, p: Value) -> BoxFuture<'_, Result<Value, String>> {
        Box::pin(self.command(p))
    }
}
fn update(state: &watch::Sender<Value>, id: &str, values: Value) {
    state.send_if_modified(|s| {
        let Some(tab) = s["tabs"]
            .as_array_mut()
            .and_then(|tabs| tabs.iter_mut().find(|t| t["id"] == id))
        else {
            return false;
        };
        let mut changed = false;
        for (key, value) in values.as_object().into_iter().flatten() {
            if tab[key] != *value {
                tab[key] = value.clone();
                changed = true;
            }
        }
        changed
    });
}
fn allowed_origin(url: &reqwest::Url, settings: &super::Settings) -> bool {
    if !matches!(url.scheme(), "http" | "https") {
        return true;
    }
    let origin = url.origin().ascii_serialization();
    let matches = |pattern: &String| {
        pattern == "*"
            || pattern == &origin
            || pattern.strip_prefix("*.").is_some_and(|host| {
                url.host_str()
                    .is_some_and(|h| h == host || h.ends_with(&format!(".{host}")))
            })
    };
    !settings.blocked_origins.iter().any(matches)
        && (settings.allowed_origins.is_empty() || settings.allowed_origins.iter().any(matches))
}

fn close_view(view: Webview) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    navigation_events::release(&view);
    view.close().map_err(|e| e.to_string())
}
