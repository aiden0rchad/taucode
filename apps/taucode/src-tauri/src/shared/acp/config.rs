use crate::{
    shared::{
        connectors,
        providers::{self, KeyStore},
    },
    types::{McpServerEntry, ProviderEntry, ProviderKind},
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};

pub(super) const PROVIDER_ERROR: &str =
    "Claude Code needs an Anthropic or Anthropic-compatible provider";
pub(super) fn environment(
    parent: impl IntoIterator<Item = (String, String)>,
    home: &Path,
    provider: &ProviderEntry,
    token: &str,
    model: &str,
) -> Result<BTreeMap<String, String>, String> {
    if !providers::claude_usable(provider) {
        return Err(PROVIDER_ERROR.into());
    }
    if token.trim().is_empty() {
        return Err("Save an API key for this provider in Settings".into());
    }
    let mut env: BTreeMap<_, _> = parent
        .into_iter()
        .filter(|(key, _)| {
            matches!(
                key.as_str(),
                "PATH" | "HOME" | "LANG" | "TMPDIR" | "SYSTEMROOT" | "WINDIR"
            )
        })
        .collect();
    env.insert("CLAUDE_CONFIG_DIR".into(), home.to_string_lossy().into());
    env.insert(
        "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC".into(),
        "1".into(),
    );
    env.insert("ANTHROPIC_MODEL".into(), model.into());
    if provider.kind == ProviderKind::Anthropic {
        env.insert("ANTHROPIC_API_KEY".into(), token.into());
        if let Some(url) = provider.base_url.as_ref().filter(|url| !url.is_empty()) {
            env.insert("ANTHROPIC_BASE_URL".into(), url.clone());
        }
    } else {
        let url = provider
            .base_url
            .as_ref()
            .filter(|url| !url.is_empty())
            .ok_or("Anthropic-compatible providers need a base URL")?;
        env.insert("ANTHROPIC_BASE_URL".into(), url.clone());
        env.insert("ANTHROPIC_AUTH_TOKEN".into(), token.into());
    }
    Ok(env)
}
pub(super) fn private_directory(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub(super) fn mcp_servers(
    entries: &[McpServerEntry],
    keys: &dyn KeyStore,
    session: &str,
) -> Result<(Value, Vec<String>), String> {
    let delivery = connectors::delivery(entries, keys)?;
    let resolve = |value: &Value| -> String {
        let raw = value.as_str().unwrap_or("");
        let (prefix, reference) = raw
            .strip_prefix("Bearer ")
            .map(|v| ("Bearer ", v))
            .unwrap_or(("", raw));
        let name = reference
            .strip_prefix("${")
            .and_then(|v| v.strip_suffix('}'))
            .unwrap_or("");
        let value = if name == "TAUCODE_SESSION_KEY" {
            session
        } else {
            delivery
                .env
                .get(name)
                .map(String::as_str)
                .unwrap_or(reference)
        };
        format!("{prefix}{value}")
    };
    let mut servers = vec![];
    for (id, server) in delivery.pi["mcpServers"].as_object().into_iter().flatten() {
        let name = id.replace('-', "_");
        if server["url"].is_string() {
            let headers: Vec<_> = server["headers"]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(name, value)| json!({"name":name,"value":resolve(value)}))
                .collect();
            servers.push(json!({"type":"http","name":name,"url":server["url"],"headers":headers}));
        } else {
            let env: Vec<_> = server["env"]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(name, value)| json!({"name":name,"value":resolve(value)}))
                .collect();
            // ACP has no cwd field; the existing launcher preserves connector cwd.
            let (command, args) = if let Some(cwd) = server["cwd"].as_str() {
                if cfg!(windows) {
                    return Err(
                        "ACP connector working directories are unsupported on Windows".into(),
                    );
                }
                let mut args = vec![
                    json!("-c"),
                    json!("cd -- \"$1\" && shift && exec \"$@\""),
                    json!("taucode-mcp"),
                    json!(cwd),
                    server["command"].clone(),
                ];
                args.extend(server["args"].as_array().into_iter().flatten().cloned());
                (json!("/bin/sh"), json!(args))
            } else {
                (server["command"].clone(), server["args"].clone())
            };
            servers.push(json!({"name":name,"command":command,"args":args,"env":env}));
        }
    }
    Ok((json!(servers), delivery.secrets))
}
pub(super) fn session_params(cwd: &str, servers: Value, model: &str) -> Value {
    json!({"cwd":cwd,"mcpServers":servers,"_meta":{"claudeCode":{"options":{
        "allowDangerouslySkipPermissions":false,"settingSources":[],"strictMcpConfig":true,"model":model,
        "settings":{"permissions":{"defaultMode":"default","disableBypassPermissionsMode":"disable","ask":["Bash","Edit","Write","mcp__*"]}}
    }}}})
}
