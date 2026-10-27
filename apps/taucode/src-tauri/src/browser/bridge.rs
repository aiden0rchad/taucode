use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, VecDeque},
    future::Future,
    path::PathBuf,
    time::Duration,
};
use tokio::sync::{mpsc, oneshot, watch};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

pub trait Transport: Send {
    fn send(&mut self, value: Value) -> impl Future<Output = Result<(), String>> + Send;
    fn receive(&mut self) -> impl Future<Output = Result<Value, String>> + Send;
}
pub struct Socket(WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>);
impl Socket {
    pub async fn connect(url: &str) -> Result<Self, String> {
        // String requests use tungstenite's default headers: no Origin header.
        let (socket, _) = connect_async(url).await.map_err(|e| e.to_string())?;
        Ok(Self(socket))
    }
}
impl Transport for Socket {
    async fn send(&mut self, value: Value) -> Result<(), String> {
        self.0
            .send(Message::Text(value.to_string()))
            .await
            .map_err(|e| e.to_string())
    }
    async fn receive(&mut self) -> Result<Value, String> {
        loop {
            match self
                .0
                .next()
                .await
                .ok_or("Browser disconnected")?
                .map_err(|e| e.to_string())?
            {
                Message::Text(text) => {
                    return serde_json::from_str(&text).map_err(|e| e.to_string())
                }
                Message::Close(_) => return Err("Browser disconnected".into()),
                _ => (),
            }
        }
    }
}
pub type Request = (Value, oneshot::Sender<Result<Value, String>>);
pub struct Bridge<T> {
    peer: T,
    next: u64,
    events: VecDeque<Value>,
    tabs: BTreeMap<String, Value>,
    active: String,
    session: String,
    width: u32,
    height: u32,
    dpr: f64,
    streaming: bool,
    casting: bool,
    pub state: watch::Sender<Value>,
    settings_path: PathBuf,
}
impl<T: Transport> Bridge<T> {
    pub fn new(peer: T, state: watch::Sender<Value>, settings_path: PathBuf) -> Self {
        state.send_modify(|s| { s["backend"] = json!("cdp"); s["capabilities"] = json!({"screenshot":true,"evaluate":true}); });
        Self {
            peer,
            next: 0,
            events: VecDeque::new(),
            tabs: BTreeMap::new(),
            active: String::new(),
            session: String::new(),
            width: 1000,
            height: 700,
            dpr: 1.,
            streaming: false,
            casting: false,
            state,
            settings_path,
        }
    }
    async fn call(&mut self, method: &str, params: Value, page: bool) -> Result<Value, String> {
        self.next += 1;
        let id = self.next;
        let mut request = json!({"id":id,"method":method,"params":params});
        if page {
            if self.session.is_empty() {
                return Err("No active browser tab".into());
            }
            request["sessionId"] = json!(self.session);
        }
        self.peer.send(request).await?;
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let value = self.peer.receive().await?;
                if value["id"] == id {
                    return if value.get("error").is_some() {
                        Err(value["error"]["message"]
                            .as_str()
                            .unwrap_or("CDP command failed")
                            .into())
                    } else {
                        Ok(value["result"].clone())
                    };
                }
                if value.get("method").is_some() {
                    self.events.push_back(value);
                }
            }
        })
        .await
        .map_err(|_| "Browser command timed out".to_string())?
    }
    fn publish(&self) {
        self.state.send_modify(|s| {
            s["tabs"] = json!(self.tabs.values().collect::<Vec<_>>());
            s["active"] = json!(self.active);
        });
    }
    fn frame(&self, data: Value) {
        self.state.send_modify(|s| {
            s["frame"] = data;
            s["width"] = json!(self.width);
            s["height"] = json!(self.height);
            s["dpr"] = json!(self.dpr);
            s["frameTarget"] = json!(self.active);
        });
    }
    pub async fn initialize(&mut self, downloads: &str) -> Result<(), String> {
        self.call("Target.setDiscoverTargets", json!({"discover":true}), false)
            .await?;
        self.call(
            "Browser.setDownloadBehavior",
            json!({"behavior":"allow", "downloadPath":downloads}),
            false,
        )
        .await?;
        let targets = self.call("Target.getTargets", json!({}), false).await?;
        for target in targets["targetInfos"].as_array().into_iter().flatten() {
            self.track(target);
        }
        if let Some(id) = self.tabs.keys().next().cloned() {
            self.activate(&id).await?;
        }
        self.publish();
        Ok(())
    }
    fn track(&mut self, info: &Value) {
        if info["type"] == "page" {
            if let Some(id) = info["targetId"].as_str() {
                let tab = self
                    .tabs
                    .entry(id.into())
                    .or_insert_with(|| json!({"id":id,"loading":false,"favicon":null}));
                tab["url"] = info["url"].clone();
                tab["title"] = info["title"].clone();
            }
        }
    }
    async fn metrics(&mut self) -> Result<(), String> {
        self.call("Emulation.setDeviceMetricsOverride", json!({"width":self.width,"height":self.height,"deviceScaleFactor":self.dpr,"mobile":false}), true).await?;
        Ok(())
    }
    async fn start_frames(&mut self) -> Result<(), String> {
        self.metrics().await?;
        if self.casting {
            self.call("Page.stopScreencast", json!({}), true).await?;
            self.casting = false;
        }
        self.call("Page.startScreencast", json!({"format":"jpeg","quality":90,"everyNthFrame":1,"maxWidth":(self.width as f64*self.dpr).round() as u32,"maxHeight":(self.height as f64*self.dpr).round() as u32}), true).await?;
        self.casting = true;
        let first = self
            .call(
                "Page.captureScreenshot",
                json!({"format":"jpeg","quality":90}),
                true,
            )
            .await?;
        self.frame(first["data"].clone());
        Ok(())
    }
    async fn activate(&mut self, id: &str) -> Result<(), String> {
        if !self.tabs.contains_key(id) {
            return Err("Unknown browser tab".into());
        }
        if self.active != id {
            if !self.session.is_empty() {
                if self.streaming {
                    let _ = self.call("Page.stopScreencast", json!({}), true).await;
                }
                let _ = self
                    .call(
                        "Target.detachFromTarget",
                        json!({"sessionId":self.session}),
                        false,
                    )
                    .await;
            }
            self.casting = false;
            self.session.clear();
            let attached = self
                .call(
                    "Target.attachToTarget",
                    json!({"targetId":id,"flatten":true}),
                    false,
                )
                .await?;
            self.session = attached["sessionId"]
                .as_str()
                .ok_or("Missing CDP session")?
                .into();
            self.active = id.into();
            self.state.send_modify(|s| {
                s["frame"] = Value::Null;
            });
            self.call("Page.enable", json!({}), true).await?;
            self.call("Runtime.enable", json!({}), true).await?;
            self.call(
                "Runtime.addBinding",
                json!({"name":"__tauBrowserMetadata"}),
                true,
            )
            .await?;
            let script = "(() => { const report = () => { const link = document.querySelector('link[rel~=icon]'); __tauBrowserMetadata(JSON.stringify({favicon:link ? link.href : null})); }; addEventListener('load', report); addEventListener('mouseover', e => __tauBrowserMetadata(JSON.stringify({cursor:getComputedStyle(e.target).cursor})), true); report(); })()";
            self.call(
                "Page.addScriptToEvaluateOnNewDocument",
                json!({"source":script}),
                true,
            )
            .await?;
            self.call("Runtime.evaluate", json!({"expression":script}), true)
                .await?;
            if self.streaming {
                self.start_frames().await?;
            }
        }
        self.call("Target.activateTarget", json!({"targetId":id}), false)
            .await?;
        self.publish();
        Ok(())
    }
    pub(super) async fn event(&mut self, event: Value) -> Result<(), String> {
        let p = &event["params"];
        match event["method"].as_str().unwrap_or("") {
            "Target.targetCreated" | "Target.targetInfoChanged" => {
                self.track(&p["targetInfo"]);
                if self.active.is_empty() {
                    if let Some(id) = self.tabs.keys().next().cloned() {
                        self.activate(&id).await?;
                    }
                }
                self.publish();
            }
            "Target.targetDestroyed" => {
                let id = p["targetId"].as_str().unwrap_or("");
                self.tabs.remove(id);
                if self.active == id {
                    self.active.clear();
                    self.casting = false;
                    self.session.clear();
                    if let Some(id) = self.tabs.keys().next().cloned() {
                        self.activate(&id).await?;
                    } else {
                        self.state.send_modify(|s| s["frame"] = Value::Null);
                    }
                }
                self.publish();
            }
            "Page.screencastFrame" => {
                // Ack even a late frame from the previous tab.
                self.next += 1;
                self.peer.send(json!({"id":self.next,"method":"Page.screencastFrameAck","sessionId":event["sessionId"],"params":{"sessionId":p["sessionId"]}})).await?;
                if event["sessionId"] == self.session {
                    self.frame(p["data"].clone());
                }
            }
            "Page.frameStartedLoading" | "Page.frameStoppedLoading"
                if event["sessionId"] == self.session =>
            {
                if let Some(tab) = self.tabs.get_mut(&self.active) {
                    tab["loading"] = json!(event["method"] == "Page.frameStartedLoading");
                }
                self.publish();
            }
            "Runtime.bindingCalled"
                if event["sessionId"] == self.session && p["name"] == "__tauBrowserMetadata" =>
            {
                if let Ok(meta) = serde_json::from_str::<Value>(p["payload"].as_str().unwrap_or(""))
                {
                    if let Some(tab) = self.tabs.get_mut(&self.active) {
                        if meta.get("favicon").is_some() {
                            tab["favicon"] = meta["favicon"].clone();
                        }
                    }
                    if let Some(cursor) = meta["cursor"].as_str() {
                        self.state.send_modify(|s| s["cursor"] = json!(cursor));
                    }
                    self.publish();
                }
            }
            _ => (),
        }
        Ok(())
    }
    async fn guarded_page(&mut self, params: &Value) -> Result<(String, String, String), String> {
        super::policy::check_target(params, &self.active, None)?;
        let result = self.call("Page.getFrameTree", json!({}), true).await?;
        let frame = &result["frameTree"]["frame"];
        let id = frame["id"].as_str().ok_or("Missing active page frame")?.to_string();
        let url = format!("{}{}", frame["url"].as_str().ok_or("Missing active page URL")?, frame["urlFragment"].as_str().unwrap_or(""));
        let loader = frame["loaderId"].as_str().ok_or("Missing active page loader")?.to_string();
        let parsed = reqwest::Url::parse(&url).map_err(|_| "Invalid active page URL")?;
        if !super::policy::automation_allowed(&parsed, &crate::storage::read_settings(&self.settings_path)?.browser) {
            return Err("Browser origin blocked by policy".into());
        }
        super::policy::check_target(params, &self.active, Some(&url))?;
        Ok((id, url, loader))
    }
    pub async fn command(&mut self, p: Value) -> Result<Value, String> {
        match p["action"].as_str().ok_or("Missing browser action")? {
            "attach" | "resize" => {
                // A bare `attach` (opening a page before the pane has measured itself) keeps the
                // current size; fractional CSS pixels are rounded rather than rejected.
                let attach = p["action"] == "attach";
                let dim = |key: &str, max: f64, current: u32, default: u32, error: &str| -> Result<u32, String> {
                    match p[key].as_f64() {
                        Some(v) if v >= 0.5 && v <= max => Ok(v.round() as u32),
                        None if attach => Ok(if current == 0 { default } else { current }),
                        _ => Err(error.to_string()),
                    }
                };
                self.width = dim("width", 7680., self.width, 1000, "Invalid pane width")?;
                self.height = dim("height", 4320., self.height, 700, "Invalid pane height")?;
                self.dpr = match p["dpr"].as_f64() {
                    Some(v) if (0.5..=4.).contains(&v) => v,
                    None if attach => if self.dpr > 0. { self.dpr } else { 1. },
                    _ => return Err("Invalid pixel ratio".into()),
                };
                self.streaming = true;
                self.state.send_modify(|s| {
                    s["streaming"] = json!(true);
                    s["width"] = json!(self.width);
                    s["height"] = json!(self.height);
                    s["dpr"] = json!(self.dpr);
                });
                if !self.session.is_empty() {
                    self.start_frames().await?;
                }
            }
            "detach" => {
                if self.streaming && !self.session.is_empty() {
                    self.call("Page.stopScreencast", json!({}), true).await?;
                }
                self.casting = false;
                self.streaming = false;
                self.state.send_modify(|s| s["streaming"] = json!(false));
            }
            "activate" => {
                self.activate(p["id"].as_str().ok_or("Missing tab ID")?)
                    .await?;
            }
            "agentTab" => {
                if let Some(url) = p["url"].as_str() {
                    let candidates: Vec<_> = self
                        .tabs
                        .iter()
                        .filter(|(_, t)| t["url"] == url)
                        .map(|(id, _)| id.clone())
                        .collect();
                    for id in &candidates {
                        // The MCP launcher brings its selected page to the front first.
                        // Inspect focus through a temporary session to disambiguate
                        // identical URLs without navigating or changing page data.
                        let focused = if candidates.len() == 1 {
                            true
                        } else {
                            let attached = self
                                .call(
                                    "Target.attachToTarget",
                                    json!({"targetId":id,"flatten":true}),
                                    false,
                                )
                                .await?;
                            let session = attached["sessionId"]
                                .as_str()
                                .ok_or("Missing CDP session")?
                                .to_string();
                            let previous = std::mem::replace(&mut self.session, session.clone());
                            let result = self.call("Runtime.evaluate", json!({"expression":"document.hasFocus()","returnByValue":true}), true).await;
                            self.session = previous;
                            let _ = self
                                .call(
                                    "Target.detachFromTarget",
                                    json!({"sessionId":session}),
                                    false,
                                )
                                .await;
                            result?["result"]["value"] == true
                        };
                        if focused {
                            self.activate(id).await?;
                            break;
                        }
                    }
                }
            }
            "new" => {
                let target = self
                    .call("Target.createTarget", json!({"url":"about:blank"}), false)
                    .await?;
                let id = target["targetId"].as_str().ok_or("Missing new tab ID")?;
                self.track(
                    &json!({"targetId":id,"type":"page","url":"about:blank","title":"New tab"}),
                );
                self.activate(id).await?;
            }
            "close" => {
                self.call(
                    "Target.closeTarget",
                    json!({"targetId":p["id"].as_str().ok_or("Missing tab ID")?}),
                    false,
                )
                .await?;
            }
            "navigate" => {
                let url = p["url"].as_str().ok_or("Missing URL")?;
                if url != "about:blank"
                    && !matches!(
                        reqwest::Url::parse(url)
                            .map(|u| u.scheme().to_string())
                            .as_deref(),
                        Ok("http" | "https")
                    )
                {
                    return Err("Use an http or https URL".into());
                }
                let result = self.call("Page.navigate", json!({"url":url}), true).await?;
                if let Some(error) = result["errorText"].as_str() {
                    return Err(error.into());
                }
            }
            "back" | "forward" => {
                let history = self
                    .call("Page.getNavigationHistory", json!({}), true)
                    .await?;
                let index = history["currentIndex"].as_i64().unwrap_or(0)
                    + if p["action"] == "back" { -1 } else { 1 };
                if index >= 0 {
                    if let Some(entry) = history["entries"].get(index as usize) {
                        self.call(
                            "Page.navigateToHistoryEntry",
                            json!({"entryId":entry["id"]}),
                            true,
                        )
                        .await?;
                    }
                }
            }
            "reload" => {
                self.call("Page.reload", json!({}), true).await?;
            }
            "stop" => {
                self.call("Page.stopLoading", json!({}), true).await?;
            }
            "screenshot" => {
                let page = self.guarded_page(&p).await?;
                let result = self.call("Page.captureScreenshot", json!({"format":"png","captureBeyondViewport":false}), true).await?;
                if self.guarded_page(&p).await? != page {
                    return Err("Page changed during capture; retry on the current page".into());
                }
                return result["data"].as_str().map(|data| json!(data)).ok_or("Missing browser screenshot".into());
            }
            "evaluate" => {
                let function = p["function"].as_str().filter(|value| !value.trim().is_empty() && value.len() <= 200000).ok_or("Invalid JavaScript function")?;
                if !p["ref"].is_null() || !p["target"].is_null() {
                    return Err("Targeted evaluation is unavailable; use the connector's element tools".into());
                }
                let (_, url, _) = self.guarded_page(&p).await?;
                // Reuse the page-world JSON/size/DOM-node checks and execution-time URL guard.
                let expression = format!("({})(() => ({}\n), {}, null)", include_str!("native/evaluate.js"), function, json!(url));
                let result = self.call("Runtime.evaluate", json!({"expression":expression,"awaitPromise":true,"returnByValue":true,"timeout":10000}), true).await?;
                if !result["exceptionDetails"].is_null() { return Err("Page evaluation failed".into()); }
                let serialized = result["result"]["value"].as_str().ok_or("Unexpected page evaluation response")?;
                if serialized.len() > 200000 { return Err("Evaluation result exceeds 200 KB".into()); }
                let value: Value = serde_json::from_str(serialized).map_err(|_| "Invalid page evaluation response")?;
                if let Some(error) = value["error"].as_str() { return Err(error.into()); }
                return value.get("result").cloned().ok_or("Unexpected page evaluation response".into());
            }
            "mouse" | "key" | "text" => {
                let (method, params) = input(&p)?;
                self.call(method, params, true).await?;
            }
            _ => return Err("Unknown browser action".into()),
        }
        Ok(Value::Null)
    }
    pub async fn run(mut self, mut requests: mpsc::Receiver<Request>) {
        loop {
            if let Some(event) = self.events.pop_front() {
                if let Err(error) = self.event(event).await {
                    self.state.send_modify(|s| s["error"] = json!(error));
                    break;
                }
                continue;
            }
            tokio::select! {
                request = requests.recv() => match request {
                    Some((p, reply)) => { let result = self.command(p).await; let _ = reply.send(result); }
                    None => break,
                },
                event = self.peer.receive() => match event {
                    Ok(event) => self.events.push_back(event),
                    Err(error) => { self.state.send_modify(|s| s["error"] = json!(error)); break; }
                },
            }
        }
    }
}

pub fn input(p: &Value) -> Result<(&'static str, Value), String> {
    let modifiers = p["modifiers"].as_u64().filter(|m| *m <= 15).unwrap_or(0);
    match p["action"].as_str() {
        Some("text") => Ok((
            "Input.insertText",
            json!({"text":p["text"].as_str().ok_or("Missing text")?}),
        )),
        Some("mouse") => {
            let kind = p["type"]
                .as_str()
                .filter(|s| {
                    matches!(
                        *s,
                        "mouseMoved" | "mousePressed" | "mouseReleased" | "mouseWheel"
                    )
                })
                .ok_or("Invalid mouse event")?;
            let button = match p["button"].as_i64().unwrap_or(-1) {
                -1 => "none",
                0 => "left",
                1 => "middle",
                2 => "right",
                3 => "back",
                4 => "forward",
                _ => return Err("Invalid mouse button".into()),
            };
            let mut params = json!({"type":kind,"x":p["x"].as_f64().ok_or("Missing x")?,"y":p["y"].as_f64().ok_or("Missing y")?,"button":button,"buttons":p["buttons"].as_u64().unwrap_or(0),"clickCount":p["clickCount"].as_u64().unwrap_or(0).min(3),"modifiers":modifiers});
            if kind == "mouseWheel" {
                params["deltaX"] = json!(p["deltaX"].as_f64().unwrap_or(0.));
                params["deltaY"] = json!(p["deltaY"].as_f64().unwrap_or(0.));
            }
            Ok(("Input.dispatchMouseEvent", params))
        }
        Some("key") => {
            let kind = p["type"]
                .as_str()
                .filter(|s| matches!(*s, "keyDown" | "keyUp"))
                .ok_or("Invalid key event")?;
            let key = p["key"].as_str().ok_or("Missing key")?;
            let mut params = json!({"type":kind,"key":key,"code":p["code"].as_str().ok_or("Missing code")?,"modifiers":modifiers,"windowsVirtualKeyCode":p["keyCode"].as_u64().unwrap_or(0)});
            if kind == "keyDown" {
                if modifiers & 6 == 0 {
                    params["text"] = json!(p["text"].as_str().unwrap_or(""));
                }
                if p["mac"] == true && modifiers & 4 != 0 {
                    let command = match key.to_ascii_lowercase().as_str() {
                        "a" => "selectAll",
                        "c" => "copy",
                        "v" => "paste",
                        "x" => "cut",
                        "z" if modifiers & 8 != 0 => "redo",
                        "z" => "undo",
                        _ => "",
                    };
                    if !command.is_empty() {
                        params["commands"] = json!([command]);
                    }
                }
            }
            Ok(("Input.dispatchKeyEvent", params))
        }
        _ => Err("Invalid input action".into()),
    }
}
