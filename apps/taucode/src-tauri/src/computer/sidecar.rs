use super::policy::Prediction;
use crate::{
    shared::providers::{self, Keychain},
    types::{AppSettings, ModelRef, ProviderKind},
};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
};

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
        config["width"] = json!(settings.computer.grounding_width);
        config["height"] = json!(settings.computer.grounding_height);
    }
    Ok((
        config,
        (format!("TAUCODE_CUA_{}_KEY", role.to_uppercase()), key),
    ))
}
pub fn source(resource: Option<&Path>) -> Result<PathBuf, String> {
    let dev = Path::new(env!("CARGO_MANIFEST_DIR")).join("../engines/computer");
    [resource.map(|p| p.join("engines/computer")), Some(dev)]
        .into_iter()
        .flatten()
        .find(|p| p.join("taucode_cua/server.py").is_file())
        .ok_or("Computer planner resources missing".into())
}
fn copy_tree(source: &Path, target: &Path) -> Result<(), String> {
    std::fs::create_dir_all(target).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_name() == "__pycache__" {
            continue;
        }
        let path = target.join(entry.file_name());
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_tree(&entry.path(), &path)?;
        } else {
            std::fs::copy(entry.path(), path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
pub async fn prepare(data: &Path, resource: Option<&Path>) -> Result<PathBuf, String> {
    let dir = data.join("engines/computer");
    let src = source(resource)?;
    copy_tree(&src.join("taucode_cua"), &dir.join("taucode_cua"))?;
    for name in ["pyproject.toml", "uv.lock"] {
        std::fs::copy(src.join(name), dir.join(name)).map_err(|e| e.to_string())?;
    }
    let python = dir.join(".venv/bin/python");
    if !python.is_file() {
        let result = Command::new("uv")
            .args(["sync", "--frozen", "--no-dev", "--python", "3.12"])
            .current_dir(&dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .status()
            .await
            .map_err(|_| "Install uv to prepare Computer Use")?;
        if !result.success() {
            return Err(
                "Computer environment installation failed; check uv and network access".into(),
            );
        }
    }
    Ok(dir)
}
pub struct Sidecar {
    child: Child,
    input: ChildStdin,
    output: BufReader<tokio::io::Take<ChildStdout>>,
    id: u64,
}
impl Sidecar {
    pub async fn launch(dir: &Path, settings: &AppSettings) -> Result<Self, String> {
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
        let mut command = Command::new(dir.join(".venv/bin/python"));
        command
            .args(["-m", "taucode_cua.server"])
            .current_dir(dir)
            .env_remove("PYTHONPATH")
            .env_remove("PYTHONSTARTUP")
            .env("PYTHONNOUSERSITE", "1")
            .env(pk.0, pk.1)
            .env(gk.0, gk.1)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = command
            .spawn()
            .map_err(|_| "Cannot start computer planner")?;
        let input = child.stdin.take().ok_or("Planner stdin unavailable")?;
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
        if response["id"] != self.id
            || response["jsonrpc"] != "2.0"
            || response.get("error").is_some()
        {
            return Err("Planner request failed; check provider configuration".into());
        }
        Ok(response["result"].clone())
    }
    pub async fn predict(
        &mut self,
        instruction: &str,
        png: &str,
        size: [u32; 2],
    ) -> Result<Prediction, String> {
        serde_json::from_value(
            self.call(
                "predict",
                json!({"instruction":instruction,"screenshot_png_b64":png,"screen_size":size}),
            )
            .await?,
        )
        .map_err(|_| "Invalid structured prediction".into())
    }
}
impl Drop for Sidecar {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}
