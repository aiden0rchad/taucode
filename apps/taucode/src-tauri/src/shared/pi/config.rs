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
            ProviderAuth::Command { .. } => format!("${}", key_env(&p.id)),
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
    let entries = crate::shared::connectors::effective(
        &AppSettings::default(),
        &WorkspaceSettings::default(),
        setup,
    )
    .expect("valid built-in connector");
    crate::shared::connectors::delivery(&entries, &providers::Keychain)
        .expect("built-in connector has no secrets")
        .pi
}

pub(crate) fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    write_file(path, &serde_json::to_vec(value).map_err(|e| e.to_string())?)
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = std::fs::write(&temp, bytes).and_then(|_| std::fs::rename(&temp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result.map_err(|e| e.to_string())
}

pub(crate) fn write_policy(
    home: &Path,
    registry: &[ProviderEntry],
    policy: &AgentPolicy,
) -> Result<(), String> {
    std::fs::create_dir_all(home.join("taucode/threads")).map_err(|e| e.to_string())?;
    let key = |m: &ModelRef| format!("{}/{}", m.provider_id, m.model_id);
    let mut value = serde_json::to_value(&policy.subagents).map_err(|e| e.to_string())?;
    value["allowedModels"] = json!(policy
        .subagents
        .allowed_models
        .iter()
        .map(&key)
        .collect::<Vec<_>>());
    value["defaultModel"] = json!(policy.subagents.default_model.as_ref().map(&key));
    for (v, role) in value["roles"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip(&policy.subagents.roles)
    {
        v["model"] = json!(role.model.as_ref().map(&key));
    }
    let mut models = serde_json::Map::new();
    for p in registry.iter().filter(|p| p.enabled) {
        for m in p.models.iter().filter(|m| m.enabled) {
            models.insert(
                format!("{}/{}", p.id, m.id),
                json!(format!("{}/{}", provider_id(p, registry), m.id)),
            );
        }
    }
    value["models"] = Value::Object(models);
    write_json(&home.join("taucode/policy.json"), &value)
}

pub(crate) fn sync_policies(data: &Path, settings: &AppSettings) -> Result<(), String> {
    let workspaces = crate::storage::read_workspaces(&data.join("workspaces.json"))?;
    for (id, workspace) in workspaces {
        if !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || id.is_empty()
        {
            continue;
        }
        let home = data.join("pi/agent").join(id);
        if home.exists() {
            write_policy(
                &home,
                &settings.providers,
                &providers::effective_policy(
                    &settings.agent_policy,
                    workspace.settings.agent_policy.as_ref(),
                ),
            )?;
            write_json(
                &home.join("taucode/connectors.json"),
                &json!(crate::shared::connectors::effective(
                    settings,
                    &workspace.settings,
                    None
                )?),
            )?;
        }
    }
    Ok(())
}

pub(super) fn write_access(home: &Path, id: &str, mode: Option<&str>) -> Result<(), String> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("Invalid pi session ID".into());
    }
    std::fs::create_dir_all(home.join("taucode/threads")).map_err(|e| e.to_string())?;
    let file = home.join("taucode/threads").join(format!("{id}.json"));
    if mode.is_none() && file.exists() {
        return Ok(());
    }
    let mode = match mode.unwrap_or("ask") {
        "current" | "ask" => "ask",
        "read-only" => "read-only",
        "full-access" => "full-access",
        _ => return Err("Invalid access mode".into()),
    };
    let mut value = match std::fs::read(&file) {
        Ok(bytes) => serde_json::from_slice::<Value>(&bytes).map_err(|e| e.to_string())?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({"sessionAllow":[]}),
        Err(e) => return Err(e.to_string()),
    };
    value["accessMode"] = json!(mode);
    write_json(&file, &value)
}

pub(super) fn prepare(
    home: &Path,
    registry: &[ProviderEntry],
    graph: Option<&CodegraphSetup>,
    keys: &dyn KeyStore,
) -> Result<HashMap<String, String>, String> {
    std::fs::create_dir_all(home.join("extensions")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(home.join("extensions/lib")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(home.join("taucode")).map_err(|e| e.to_string())?;
    for (name, source) in [
        (
            "taucode-gate.ts",
            include_str!("../../../../engines/pi/extensions/taucode-gate.ts"),
        ),
        (
            "taucode-subagents.ts",
            include_str!("../../../../engines/pi/extensions/taucode-subagents.ts"),
        ),
        (
            "taucode-work.ts",
            include_str!("../../../../engines/pi/extensions/taucode-work.ts"),
        ),
        (
            "lib/policy.ts",
            include_str!("../../../../engines/pi/extensions/lib/policy.ts"),
        ),
        (
            "lib/connectors.ts",
            include_str!("../../../../engines/pi/extensions/lib/connectors.ts"),
        ),
        (
            "LICENSE",
            include_str!("../../../../engines/pi/extensions/LICENSE"),
        ),
    ] {
        write_file(&home.join("extensions").join(name), source.as_bytes())?;
    }
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
