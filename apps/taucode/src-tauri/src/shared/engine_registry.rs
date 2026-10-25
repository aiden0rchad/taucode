use crate::types::{AppSettings, EngineDefinition, EngineProtocol, Features};
use serde_json::{json, Value};
use std::{collections::HashMap, path::{Path, PathBuf}, time::Duration};
use tokio::{io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader}, process::Command};

pub(crate) fn registry(settings: &AppSettings) -> Vec<EngineDefinition> {
    let mut engines: Vec<_> = [("pi", "pi", EngineProtocol::PiRpc), ("omp", "oh-my-pi", EngineProtocol::PiRpc), ("codex", "Codex", EngineProtocol::CodexAppServer), ("claude", "Claude Code", EngineProtocol::Acp)].into_iter().map(|(id, name, protocol)| EngineDefinition {
        id: id.into(), name: name.into(), protocol, built_in: true,
        enabled: settings.engines.iter().find(|e| e.id == id).map_or(id != "omp", |e| e.enabled),
        command: None, args: vec![], env: HashMap::new(), config_dir_env: None, description: None,
    }).collect();
    engines.extend(settings.engines.iter().filter(|e| !engines_id(&e.id)).cloned());
    engines
}
fn engines_id(id: &str) -> bool { matches!(id, "pi" | "omp" | "codex" | "claude") }
pub(crate) fn resolve(settings: &AppSettings, id: &str) -> Result<(EngineDefinition, Option<String>), String> {
    let engines = registry(settings);
    if let Some(engine) = engines.iter().find(|e| e.id == id) { return Ok((engine.clone(), None)); }
    let engine = engines.iter().find(|e| e.id == settings.agent_policy.default_engine && e.enabled)
        .or_else(|| engines.iter().find(|e| e.id == "pi" && e.enabled))
        .or_else(|| engines.iter().find(|e| e.enabled)).ok_or("Enable an engine in Settings")?.clone();
    let notice = format!("Unknown engine {id}; using {}.", engine.name);
    Ok((engine, Some(notice)))
}
pub(crate) fn settings(data: Option<&Path>) -> AppSettings {
    data.and_then(|p| crate::storage::read_settings(&p.join("settings.json")).ok()).unwrap_or_default()
}
pub(crate) fn features(data: Option<&Path>) -> Features { settings(data).features() }
pub(crate) fn executable(command: &str) -> Result<PathBuf, String> {
    let candidates = if Path::new(command).components().count() > 1 { vec![PathBuf::from(command)] }
        else { std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).map(|p| p.join(command)).collect() };
    candidates.into_iter().find(|p| p.is_file() && {
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; p.metadata().is_ok_and(|m| m.permissions().mode() & 0o111 != 0) }
        #[cfg(not(unix))] { true }
    }).ok_or_else(|| format!("Command must exist and be executable: {command}"))
}
fn env_name(key: &str) -> bool {
    !key.is_empty() && !key.as_bytes()[0].is_ascii_digit() && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
pub(crate) fn validate(engine: &EngineDefinition) -> Result<(), String> {
    if engine.id.is_empty() || !engine.id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') || engine.name.trim().is_empty() { return Err("Engine needs a name and a safe, unique ID".into()); }
    if engines_id(&engine.id) { return Ok(()); }
    if engine.built_in || engine.protocol == EngineProtocol::CodexAppServer { return Err("Custom engines must use ACP or pi-RPC".into()); }
    executable(engine.command.as_deref().filter(|s| !s.is_empty()).ok_or("Missing engine command")?)?;
    for key in engine.env.keys() {
        let upper = key.to_ascii_uppercase();
        if ["KEY", "TOKEN", "SECRET", "PASSWORD"].iter().any(|part| upper.contains(part)) { return Err(format!("{key}: use a provider instead")); }
        if !env_name(key) || matches!(upper.as_str(), "HOME" | "PATH" | "NODE_OPTIONS" | "BUN_OPTIONS" | "LD_PRELOAD" | "DYLD_INSERT_LIBRARIES" | "PI_CODING_AGENT_DIR" | "PI_CONFIG_DIR") || upper.starts_with("OTEL_") || upper.starts_with("OMP_AUTH_") { return Err(format!("Reserved environment variable: {key}")); }
    }
    if engine.config_dir_env.as_ref().is_some_and(|key| !env_name(key)) { return Err("Invalid config-dir environment variable name".into()); }
    Ok(())
}
pub(crate) fn validate_settings(settings: &AppSettings) -> Result<(), String> {
    let mut ids = std::collections::HashSet::new();
    for engine in &settings.engines { validate(engine)?; if !ids.insert(&engine.id) { return Err("Duplicate engine ID".into()); } }
    if !registry(settings).iter().any(|e| e.enabled) { return Err("Keep at least one engine enabled".into()); }
    Ok(())
}
pub(crate) fn environment(parent: impl IntoIterator<Item=(String, String)>, home: &Path) -> HashMap<String, String> {
    let mut env: HashMap<_, _> = parent.into_iter().filter(|(key, _)| matches!(key.as_str(), "PATH" | "LANG" | "TMPDIR" | "SYSTEMROOT" | "WINDIR" | "TERM")).collect();
    env.insert("HOME".into(), home.to_string_lossy().into());
    env.insert("XDG_CONFIG_HOME".into(), home.to_string_lossy().into());
    env.insert("OTEL_SDK_DISABLED".into(), "true".into());
    env
}
pub(crate) async fn test(data: &Path, engine: &EngineDefinition) -> Result<Value, String> {
    validate(engine)?;
    let home = data.join("engine-tests").join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&home).map_err(|e| e.to_string())?;
    let result = async {
        let mut command = Command::new(executable(engine.command.as_deref().ok_or("Missing engine command")?)?);
        command.args(&engine.args).env_clear().envs(environment(std::env::vars(), &home)).envs(&engine.env)
            .env(engine.config_dir_env.as_deref().unwrap_or("PI_CODING_AGENT_DIR"), &home)
            .current_dir(&home).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
        if engine.protocol == EngineProtocol::PiRpc { command.args(["--mode", "rpc"]); }
        let mut child = command.spawn().map_err(|e| e.to_string())?;
        let mut stderr = child.stderr.take().ok_or("Missing stderr")?;
        let mut diagnostics = tokio::spawn(async move {
            let mut excerpt = Vec::new();
            let mut buffer = [0; 1024];
            while let Ok(count) = stderr.read(&mut buffer).await {
                if count == 0 { break; }
                excerpt.extend_from_slice(&buffer[..count]);
                if excerpt.len() > 4096 { excerpt.drain(..excerpt.len() - 4096); }
            }
            String::from_utf8_lossy(&excerpt).trim().to_owned()
        });
        let request = match engine.protocol {
            EngineProtocol::Acp => json!({"jsonrpc":"2.0","id":"taucode-test","method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{},"clientInfo":{"name":"taucode","version":"1"}}}),
            EngineProtocol::PiRpc => json!({"id":"taucode-test","type":"get_state"}),
            _ => return Err("Custom engines must use ACP or pi-RPC".into()),
        };
        let mut input = child.stdin.take().ok_or("Missing stdin")?;
        input.write_all(format!("{request}\n").as_bytes()).await.map_err(|e| e.to_string())?;
        let mut lines = BufReader::new(child.stdout.take().ok_or("Missing stdout")?).lines();
        let result = tokio::time::timeout(Duration::from_secs(15), async {
            while let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
                let value: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
                if value["id"] == "taucode-test" {
                    if value.get("error").is_some() || value["success"] == false { return Err("Engine rejected the protocol handshake".into()); }
                    if engine.protocol == EngineProtocol::Acp && !value["result"]["protocolVersion"].is_number() { return Err("Invalid ACP initialize response".into()); }
                    if engine.protocol == EngineProtocol::PiRpc && (value["type"] != "response" || value["success"] != true || !value["data"].is_object()) { return Err("Invalid pi-RPC get_state response".into()); }
                    let version = value.pointer("/result/agentInfo/version").or_else(|| value.pointer("/data/version")).and_then(Value::as_str);
                    let message = version.map_or_else(|| "Protocol handshake passed (version not reported)".to_owned(), |version| format!("Protocol handshake passed · v{version}"));
                    return Ok(json!({"engine":engine.id,"state":"ready","message":message,"version":version}));
                }
            }
            Err("Engine exited before handshake".into())
        }).await.unwrap_or_else(|_| Err("Engine handshake timed out".into()));
        let _ = child.kill().await;
        let _ = child.wait().await;
        let excerpt = tokio::time::timeout(Duration::from_millis(200), &mut diagnostics).await.ok().and_then(Result::ok).unwrap_or_default();
        diagnostics.abort();
        result.map_err(|error| if excerpt.is_empty() { error } else { format!("{error}\nStderr: {excerpt}") })
    }.await;
    let _ = std::fs::remove_dir_all(home);
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn migration_and_resolution() {
        let settings: AppSettings = serde_json::from_value(json!({})).unwrap();
        assert_eq!(settings.features(), Features::default());
        for id in ["pi", "codex", "claude"] { assert_eq!(serde_json::from_value::<crate::types::EngineKind>(json!(id)).unwrap(), id); assert!(resolve(&settings,id).unwrap().1.is_none()); }
        assert_eq!(resolve(&settings,"missing").unwrap().0.id,"pi");
        assert!(resolve(&settings,"missing").unwrap().1.is_some());
        assert_eq!(resolve(&settings,"omp").unwrap().0.protocol,EngineProtocol::PiRpc);
    }
    #[test] fn environment_is_isolated() {
        let env = environment([("OMP_AUTH_BROKER_TOKEN".into(),"secret".into()),("OTEL_EXPORTER_OTLP_ENDPOINT".into(),"bad".into()),("HOME".into(),"user".into())],Path::new("/isolated"));
        assert_eq!(env["HOME"],"/isolated"); assert_eq!(env["OTEL_SDK_DISABLED"],"true"); assert_eq!(env.len(),3);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    fn custom(protocol: EngineProtocol) -> EngineDefinition {
        EngineDefinition { id: "custom".into(), name: "Test".into(), protocol, built_in: false, command: Some("python3".into()), args: vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine_handshake.py").to_string_lossy().into(), if protocol == EngineProtocol::Acp { "acp" } else { "pi" }.into()], env: HashMap::new(), config_dir_env: Some("TEST_CONFIG_DIR".into()), enabled: true, description: None }
    }
    #[test] fn custom_validation_and_disabled_resolution() {
        let mut definition = custom(EngineProtocol::PiRpc);
        validate(&definition).unwrap();
        definition.env.insert("access_TOKEN".into(), "do-not-store".into());
        assert!(validate(&definition).unwrap_err().contains("use a provider instead"));
        definition.env.clear(); definition.command = Some("/no/such/command".into());
        assert!(validate(&definition).is_err());
        let mut settings = AppSettings::default();
        let mut pi = resolve(&settings,"pi").unwrap().0; pi.enabled = false; settings.engines.push(pi);
        assert!(!resolve(&settings,"pi").unwrap().0.enabled);
    }
    #[tokio::test] async fn custom_protocol_handshakes() {
        let data = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        for protocol in [EngineProtocol::Acp, EngineProtocol::PiRpc] {
            let response = test(&data,&custom(protocol)).await.unwrap();
            assert_eq!(response["state"], "ready");
            assert_eq!(response["version"], "1.2.3");
        }
        let mut broken = custom(EngineProtocol::Acp);
        broken.args[1] = "fail".into();
        assert!(test(&data, &broken).await.unwrap_err().contains("Stderr: fixture startup failed"));
        std::fs::remove_dir_all(data).unwrap();
    }
    #[test] fn feature_tool_filtering_is_shared_by_all_transports() {
        let mut settings = AppSettings::default(); settings.computer.enabled = true;
        settings.features.browser = false; settings.features.computer_use = false;
        let entries = crate::shared::connectors::effective(&settings, &Default::default(), None).unwrap();
        assert!(entries.iter().filter(|e| matches!(e.id.as_str(),"taucode-browser"|"taucode-computer")).all(|e| !e.enabled));
        settings.features.connectors = false;
        let entries = crate::shared::connectors::effective(&settings, &Default::default(), None).unwrap();
        assert!(entries.iter().all(|e| !e.enabled));
        settings.features.git = false;
        assert!(!settings.features().worktrees); assert!(settings.features().checkpoints);
        let policy = crate::shared::engines::feature_policy(settings.agent_policy, &Features { subagents: false, ..Features::default() });
        assert!(!policy.subagents.enabled); assert_eq!(policy.subagents.max_concurrent,0);
    }
}
