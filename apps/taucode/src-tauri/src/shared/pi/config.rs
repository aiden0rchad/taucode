use crate::shared::{
    engines::key_env,
    providers::{self, KeyStore},
};
use crate::{codegraph::CodegraphSetup, types::*};
use serde_json::{json, Value};
use std::{collections::HashMap, path::Path};

pub(super) fn provider_id(p: &ProviderEntry, registry: &[ProviderEntry]) -> String {
    let kind = serde_json::to_value(p.kind)
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned();
    if matches!(
        p.kind,
        ProviderKind::Ollama | ProviderKind::Lmstudio | ProviderKind::OpenaiCompatible
    ) || registry
        .iter()
        .filter(|v| v.enabled && v.kind == p.kind)
        .count()
        > 1
    {
        // Prefix custom IDs so a registry slug cannot activate an OAuth provider.
        format!("taucode-{}", p.id)
    } else {
        kind
    }
}

pub(crate) fn models(registry: &[ProviderEntry]) -> Value {
    let mut entries = serde_json::Map::new();
    for p in registry.iter().filter(|p| p.enabled) {
        let id = provider_id(p, registry);
        let custom = id.starts_with("taucode-");
        let auth = match &p.auth {
            ProviderAuth::None => "taucode-no-key".into(),
            ProviderAuth::ApiKey => format!("${}", key_env(&p.id)),
            ProviderAuth::Env { var } => format!("${var}"),
            ProviderAuth::Command { command, args } => format!(
                "!{}",
                std::iter::once(command)
                    .chain(args)
                    .map(|s| shell_words::quote(s).into_owned())
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
        };
        let mut entry = json!({"name":p.name,"apiKey":auth,"models":p.models.iter().filter(|m|m.enabled).map(|m| {
            let mut v = json!({"id":m.id,"name":m.display_name.as_ref().unwrap_or(&m.id)});
            if let Some(n) = m.context_window { v["contextWindow"] = json!(n); }
            if let Some(images) = m.supports_images { v["input"] = if images { json!(["text","image"]) } else { json!(["text"]) }; }
            v
        }).collect::<Vec<_>>()});
        if custom
            || p.base_url
                .as_deref()
                .is_some_and(|s| !s.is_empty() && s != providers::kind_info(p.kind).0)
        {
            entry["baseUrl"] = json!(providers::base_url(p));
        }
        if custom {
            entry["api"] = json!(match p.kind {
                ProviderKind::Anthropic => "anthropic-messages",
                ProviderKind::Google => "google-generative-ai",
                ProviderKind::Openai => "openai-responses",
                _ if p.api_style == Some(ApiStyle::Responses) => "openai-responses",
                _ => "openai-completions",
            });
        }
        if !p.extra_headers.is_empty() {
            entry["headers"] = json!(p.extra_headers);
        }
        entries.insert(id, entry);
    }
    json!({"providers":entries})
}

pub(super) fn mcp(setup: Option<&CodegraphSetup>) -> Value {
    match setup {
        None => json!({"mcpServers":{},"autoEnableCodemode":false}),
        Some(s) => json!({"autoEnableCodemode":false,"mcpServers":{"codegraph":{
            "command":s.proxy_exe,"args":std::iter::once("--tau-mcp-proxy").chain(std::iter::once("--")).chain(s.upstream.iter().map(String::as_str)).collect::<Vec<_>>(),
            "env":{"TAUCODE_STATE_DIR":s.state_dir,"CODEGRAPH_EXPLORE_DEDUP":"1","CODEGRAPH_DIR":crate::codegraph::CODEGRAPH_DIR,"TAUCODE_SESSION_KEY":"${TAUCODE_SESSION_KEY}"},"exposure":"direct"
        }}}),
    }
}

pub(crate) fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = std::fs::write(&temp, serde_json::to_vec(value).map_err(|e| e.to_string())?)
        .and_then(|_| std::fs::rename(&temp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result.map_err(|e| e.to_string())
}

pub(super) fn prepare(
    home: &Path,
    registry: &[ProviderEntry],
    graph: Option<&CodegraphSetup>,
    keys: &dyn KeyStore,
) -> Result<HashMap<String, String>, String> {
    std::fs::create_dir_all(home.join("extensions")).map_err(|e| e.to_string())?;
    if home.join("auth.json").exists() {
        return Err("Remove auth.json from taucode's isolated pi home; accounts and logins are not supported".into());
    }
    write_json(&home.join("models.json"), &models(registry))?;
    write_json(&home.join("mcp.json"), &mcp(graph))?;
    write_json(
        &home.join("settings.json"),
        &json!({"quietStartup":true,"enableInstallTelemetry":false,"enabledModels":registry.iter().filter(|p|p.enabled).flat_map(|p|p.models.iter().filter(|m|m.enabled).map(move |m|format!("{}/{}",provider_id(p,registry),m.id))).collect::<Vec<_>>()}),
    )?;
    let mut env = HashMap::new();
    for p in registry.iter().filter(|p| p.enabled) {
        if matches!(p.auth, ProviderAuth::ApiKey) {
            env.insert(
                key_env(&p.id),
                keys.get(&p.id)?
                    .ok_or_else(|| format!("Save an API key for {} in Settings", p.name))?,
            );
        }
    }
    env.insert(
        "PI_CODING_AGENT_DIR".into(),
        home.to_string_lossy().into_owned(),
    );
    env.insert("PI_TELEMETRY".into(), "0".into());
    env.insert("PI_SKIP_VERSION_CHECK".into(), "1".into());
    Ok(env)
}
