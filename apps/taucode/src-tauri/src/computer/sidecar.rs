use super::policy::Prediction;
use crate::{
    shared::providers::{self, Keychain},
    types::{AppSettings, ModelRef, ProviderKind},
};
use serde_json::{json, Value};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
};

fn parse_element_reply(reply: &str, elements: &Value) -> Result<Option<u64>, ()> {
    let clean: String = reply
        .chars()
        .filter(|c| !matches!(c, '*' | '`' | '_'))
        .collect();
    let clean = clean
        .trim()
        .trim_end_matches(['.', ',', ';', ':', '!', '?'])
        .trim();
    if clean.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    let mut tokens = clean
        .split(|c: char| !c.is_ascii_digit() && c != '-' && c != '+')
        .filter(|s| !s.is_empty());
    let token = tokens.next().ok_or(())?;
    if token.len() > 10 || tokens.next().is_some() {
        return Err(());
    }
    let index: u64 = token.parse().map_err(|_| ())?;
    if index >= 250
        || !elements
            .as_array()
            .is_some_and(|items| items.iter().any(|e| e["index"] == index))
    {
        return Err(());
    }
    Ok(Some(index))
}

fn normalize_element_reply(prediction: &mut Prediction, snapshot: &Value) {
    // A malformed index must never erase evidence of a changed target binding.
    if prediction.action["snapshot_id"] != snapshot["snapshot_id"] {
        return;
    }
    let Some(raw) = prediction.action.get("element_index") else {
        return;
    };
    let reply = raw
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| raw.to_string());
    match parse_element_reply(&reply, &snapshot["elements"]) {
        Ok(Some(index)) => prediction.action["element_index"] = json!(index),
        result => {
            let action = prediction.action.as_object_mut().unwrap();
            action.remove("element_index");
            action.remove("snapshot_id");
            prediction.grounding.push(json!({"model_result":if result.is_err() {"unparseable"} else {"none"},"raw_reply":reply,"pixel_fallback_reason":"invalid_element_reply"}));
        }
    }
}

pub async fn configuration(
    settings: &AppSettings,
    model: &ModelRef,
    role: &str,
) -> Result<(Value, (String, String)), String> {
    let p = settings
        .providers
        .iter()
        .find(|p| p.id == model.provider_id && p.enabled)
        .ok_or("Enable the selected computer provider")?;
    if !p.models.iter().any(|m| m.id == model.model_id && m.enabled) {
        return Err("Enable the selected computer model".into());
    }
    let key = providers::token(p, &Keychain).await?.unwrap_or_default();
    let mut base = providers::base_url(p).to_string();
    if p.kind == ProviderKind::Google {
        base = format!("{}/openai/", base.trim_end_matches('/'));
    }
    if p.kind == ProviderKind::Anthropic {
        base = base.trim_end_matches("/v1").into();
    }
    let mut config = json!({"kind":p.kind,"model":model.model_id,"base_url":base});
    if role == "grounding" {
        config["mode"] = json!(settings.computer.grounding_mode);
        config["width"] = json!(settings.computer.grounding_width);
        config["height"] = json!(settings.computer.grounding_height);
    }
    Ok((
        config,
        (format!("TAUCODE_CUA_{}_KEY", role.to_uppercase()), key),
    ))
}
pub async fn prepare(data: &Path, resource: Option<&Path>) -> Result<PathBuf, String> {
    crate::shared::engine_runtime::prepare(data, "computer", resource).await
}
pub struct Sidecar {
    child: Child,
    input: ChildStdin,
    output: BufReader<tokio::io::Take<ChildStdout>>,
    id: u64,
    pub grounding: Vec<Value>,
    log: Arc<Mutex<SidecarLog>>,
    logger: tokio::task::JoinHandle<()>,
    flush: tokio::sync::mpsc::Sender<tokio::sync::oneshot::Sender<()>>,
}

const LOG_LIMIT: u64 = 2 * 1024 * 1024;
pub fn setup_error(data: &Path, error: String) -> String {
    match SidecarLog::open(data, vec![]) {
        Ok(log) => log.with_error(error),
        Err(_) => error,
    }
}
struct SidecarLog {
    path: PathBuf,
    file: std::fs::File,
    size: u64,
    secrets: Vec<String>,
    error: Option<String>,
    pending: Vec<u8>,
}
impl SidecarLog {
    fn open(data: &Path, secrets: Vec<String>) -> Result<Self, String> {
        let dir = data.join("computer/logs");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        let path = dir.join("sidecar.log");
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        let size = file.metadata().map_err(|e| e.to_string())?.len();
        Ok(Self {
            path,
            file,
            size,
            secrets,
            error: None,
            pending: Vec::new(),
        })
    }
    fn write(&mut self, bytes: &[u8], eof: bool) -> std::io::Result<()> {
        self.pending.extend_from_slice(bytes);
        let mut redacted = Vec::new();
        let mut consumed = 0;
        while consumed < self.pending.len() {
            let remaining = &self.pending[consumed..];
            if !eof
                && self
                    .secrets
                    .iter()
                    .any(|s| s.len() > remaining.len() && s.as_bytes().starts_with(remaining))
            {
                break; // Retain a credential split across stderr reads.
            }
            if let Some(secret) = self
                .secrets
                .iter()
                .find(|s| !s.is_empty() && remaining.starts_with(s.as_bytes()))
            {
                redacted.extend_from_slice(b"[REDACTED]");
                consumed += secret.len();
            } else {
                redacted.push(remaining[0]);
                consumed += 1;
            }
        }
        self.pending.drain(..consumed);
        let mut bytes = redacted.as_slice();
        while !bytes.is_empty() {
            if self.size >= LOG_LIMIT {
                // Keep two backups in addition to the active log.
                let first = self.path.with_extension("log.1");
                if first.exists() {
                    std::fs::rename(&first, self.path.with_extension("log.2"))?;
                }
                std::fs::rename(&self.path, first)?;
                self.file = std::fs::File::create(&self.path)?;
                self.size = 0;
            }
            let n = bytes.len().min((LOG_LIMIT - self.size) as usize);
            self.file.write_all(&bytes[..n])?;
            self.size += n as u64;
            bytes = &bytes[n..];
        }
        Ok(())
    }
    fn with_error(&self, error: String) -> String {
        let mut bytes = Vec::new();
        for path in [
            self.path.with_extension("log.2"),
            self.path.with_extension("log.1"),
            self.path.clone(),
        ] {
            if let Ok(mut file) = std::fs::File::open(path) {
                use std::io::{Read, Seek, SeekFrom};
                let _ = file
                    .seek(SeekFrom::End(-65536))
                    .or_else(|_| file.seek(SeekFrom::Start(0)));
                let _ = file.take(65536).read_to_end(&mut bytes);
            }
        }
        let text = String::from_utf8_lossy(&bytes);
        let mut lines: Vec<_> = text.lines().rev().take(20).collect();
        lines.reverse();
        let mut result = error;
        if !lines.is_empty() {
            result.push_str(&format!(
                "\nSidecar stderr (last 20 lines):\n{}",
                lines.join("\n")
            ));
        }
        if let Some(error) = &self.error {
            result.push_str(&format!("\nSidecar logging failed: {error}"));
        }
        for secret in &self.secrets {
            if !secret.is_empty() {
                result = result.replace(secret, "[REDACTED]");
            }
        }
        result
    }
}

impl Sidecar {
    pub async fn launch(dir: &Path, data: &Path, settings: &AppSettings) -> Result<Self, String> {
        let (planner, pk) = configuration(
            settings,
            settings.computer.planner.as_ref().ok_or("Select planner")?,
            "planner",
        )
        .await?;
        let (grounding, gk) = configuration(
            settings,
            settings
                .computer
                .grounding
                .as_ref()
                .ok_or("Select grounding")?,
            "grounding",
        )
        .await?;
        let log = Arc::new(Mutex::new(SidecarLog::open(
            data,
            vec![pk.1.clone(), gk.1.clone()],
        )?));
        let mut command = Command::new(dir.join(".venv/bin/python"));
        command
            .args(["-u", "-m", "taucode_cua.server"])
            .current_dir(dir)
            .env_remove("PYTHONPATH")
            .env_remove("PYTHONSTARTUP")
            .env("PYTHONNOUSERSITE", "1")
            .env(pk.0, pk.1)
            .env(gk.0, gk.1)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn().map_err(|_| {
            log.lock()
                .unwrap()
                .with_error("Cannot start computer planner".into())
        })?;
        let input = child.stdin.take().ok_or("Planner stdin unavailable")?;
        let mut stderr = child.stderr.take().ok_or("Planner stderr unavailable")?;
        let writer = log.clone();
        let (flush, mut requests) =
            tokio::sync::mpsc::channel::<tokio::sync::oneshot::Sender<()>>(1);
        let logger = tokio::spawn(async move {
            let mut buffer = [0; 8192];
            loop {
                let read = tokio::select! {
                    biased;
                    read = stderr.read(&mut buffer) => read,
                    request = requests.recv() => {
                        match request { Some(ack) => { let _ = ack.send(()); }, None => break }
                        continue;
                    }
                };
                match read {
                    Ok(0) => {
                        let mut log = writer.lock().unwrap();
                        if let Err(error) = log.write(&[], true) {
                            log.error = Some(error.to_string());
                        }
                        break;
                    }
                    Ok(n) => {
                        let mut log = writer.lock().unwrap();
                        if let Err(error) = log.write(&buffer[..n], false) {
                            log.error = Some(error.to_string());
                        }
                    }
                    Err(error) => {
                        writer.lock().unwrap().error = Some(error.to_string());
                        break;
                    }
                }
            }
        });
        let output = BufReader::new(
            child
                .stdout
                .take()
                .ok_or("Planner stdout unavailable")?
                .take(16 * 1024 * 1024),
        );
        let mut s = Self {
            child,
            input,
            output,
            id: 0,
            grounding: vec![],
            log,
            logger,
            flush,
        };
        s.call(
            "configure",
            json!({"planner":planner,"grounding":grounding,"platform":"darwin"}),
        )
        .await?;
        s.call("reset", json!({})).await?;
        Ok(s)
    }
    pub async fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let result = self.exchange(method, params).await;
        if result.is_err() {
            // The reader acknowledges only after draining currently readable stderr.
            let (tx, rx) = tokio::sync::oneshot::channel();
            if self.flush.send(tx).await.is_ok() {
                let _ = rx.await;
            }
        }
        result.map_err(|error| self.with_log(error))
    }
    pub fn log_execution(&self, step: &Value) {
        let event = json!({"event":"grounding_execution","grounding":step["grounding"],"step":step["step"],"path":step["path"],"element_index":step["action"]["element_index"],"snapshot_id":step["action"]["snapshot_id"],"pixel_fallback_reason":step["executionFallbackReason"]});
        let mut log = self.log.lock().unwrap();
        if let Err(error) = log.write(format!("{event}\n").as_bytes(), false) {
            log.error = Some(error.to_string());
        }
    }
    pub fn with_log(&self, error: String) -> String {
        self.log.lock().unwrap().with_error(error)
    }
    async fn exchange(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.grounding.clear();
        self.id += 1;
        self.input
            .write_all(
                format!(
                    "{}\n",
                    json!({"jsonrpc":"2.0","id":self.id,"method":method,"params":params})
                )
                .as_bytes(),
            )
            .await
            .map_err(|_| "Planner disconnected")?;
        let mut line = String::new();
        let n = self
            .output
            .read_line(&mut line)
            .await
            .map_err(|_| "Planner disconnected")?;
        if n == 0 || line.len() > 1024 * 1024 {
            return Err("Invalid planner response size".into());
        }
        let response: Value =
            serde_json::from_str(&line).map_err(|_| "Invalid planner response")?;
        self.grounding = serde_json::from_value(
            if response.get("error").is_some() {
                &response["error"]["data"]["grounding"]
            } else {
                &response["result"]["grounding"]
            }
            .clone(),
        )
        .unwrap_or_default();
        response_result(response, self.id)
    }

    pub async fn predict(
        &mut self,
        instruction: &str,
        png: &str,
        size: [u32; 2],
        snapshot: Value,
    ) -> Result<Prediction, String> {
        let mut prediction: Prediction = serde_json::from_value(
            self.call(
                "predict",
                json!({"instruction":instruction,"screenshot_png_b64":png,"screen_size":size,"ax_snapshot":snapshot}),
            )
            .await?,
        )
        .map_err(|_| self.with_log("Invalid structured prediction".into()))?;
        normalize_element_reply(&mut prediction, &snapshot);
        Ok(prediction)
    }
}
impl Drop for Sidecar {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
        self.logger.abort();
    }
}

fn response_result(response: Value, id: u64) -> Result<Value, String> {
    if response["id"] != id || response["jsonrpc"] != "2.0" {
        return Err("Invalid planner response".into());
    }
    if let Some(error) = response.get("error") {
        // The sidecar redacts credentials before crossing this boundary.
        return Err(error["message"]
            .as_str()
            .unwrap_or("Planner request failed")
            .chars()
            .take(400)
            .collect());
    }
    Ok(response["result"].clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lenient_element_reply_table_and_pixel_fallback() {
        let snapshot = json!({"snapshot_id":1,"elements":[{"index":42}]});
        for (reply, expected) in [
            ("**42**", Ok(Some(42))),
            ("`42`", Ok(Some(42))),
            ("42.", Ok(Some(42))),
            ("Index: 42", Ok(Some(42))),
            ("Element 42", Ok(Some(42))),
            (" _NoNe_! ", Ok(None)),
            ("42 or 43", Err(())),
            ("43", Err(())),
            ("-42", Err(())),
            ("42.0", Err(())),
            ("garbage", Err(())),
        ] {
            assert_eq!(
                parse_element_reply(reply, &snapshot["elements"]),
                expected,
                "{reply}"
            );
            let mut prediction = Prediction {
                action: json!({"element_index":reply,"snapshot_id":1,"x":10,"y":20}),
                reflection: String::new(),
                plan_text: String::new(),
                grounding: vec![],
            };
            normalize_element_reply(&mut prediction, &snapshot);
            if expected == Ok(Some(42)) {
                assert_eq!(prediction.action["element_index"], 42);
            } else {
                assert!(prediction.action.get("snapshot_id").is_none());
                assert_eq!(prediction.action["x"], 10);
                assert_eq!(
                    prediction.grounding[0]["model_result"],
                    if expected.is_err() {
                        "unparseable"
                    } else {
                        "none"
                    }
                );
            }
        }
        let mut changed = Prediction {
            action: json!({"snapshot_id":2,"element_index":"garbage"}),
            reflection: String::new(),
            plan_text: String::new(),
            grounding: vec![],
        };
        normalize_element_reply(&mut changed, &snapshot);
        assert_eq!(changed.action["snapshot_id"], 2);
        assert!(changed.element_target().is_err());
    }
    #[test]
    fn logs_rotate_keep_two_backups_and_redact_split_credentials() {
        let data = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let mut log = SidecarLog::open(&data, vec!["secret-key".into()]).unwrap();
        for byte in b'a'..=b'd' {
            log.write(&vec![byte; LOG_LIMIT as usize], false).unwrap();
        }
        log.write(b"\nsec", false).unwrap();
        log.write(b"ret-key\n", false).unwrap();
        assert!(log.pending.is_empty());
        let path = data.join("computer/logs/sidecar.log");
        for (suffix, byte) in [("log.1", b'd'), ("log.2", b'c')] {
            let bytes = std::fs::read(path.with_extension(suffix)).unwrap();
            assert_eq!(bytes.len(), LOG_LIMIT as usize);
            assert!(bytes.iter().all(|b| *b == byte));
        }
        assert!(!path.with_extension("log.3").exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "\n[REDACTED]\n");
        for n in 0..25 {
            log.write(format!("diagnostic {n}\n").as_bytes(), false)
                .unwrap();
        }
        let error = log.with_error("Provider error: secret-key".into());
        assert!(error.starts_with("Provider error: [REDACTED]"));
        let tail = error
            .split("Sidecar stderr (last 20 lines):\n")
            .nth(1)
            .unwrap();
        assert_eq!(tail.lines().count(), 20);
        assert!(tail.starts_with("diagnostic 5\n"));
        assert!(tail.ends_with("diagnostic 24"));
        drop(log);
        std::fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn preserves_sidecar_error_and_checks_response_id() {
        for message in [
            "Configuration error: Invalid grounding mode",
            "Input error: Expected PNG matching screen size",
            "Provider error: Model unavailable",
        ] {
            let response = json!({"jsonrpc":"2.0","id":1,"error":{"message":message}});
            assert_eq!(response_result(response.clone(), 1), Err(message.into()));
            assert_eq!(
                response_result(response, 2),
                Err("Invalid planner response".into())
            );
        }
    }
}
