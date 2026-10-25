use super::providers::{valid_slug, KeyStore};
use crate::{
    codegraph::CodegraphSetup,
    types::{AppSettings, WorkspaceSettings},
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
};

pub(crate) const RESERVED: &[&str] = &["codegraph", "taucode-browser", "taucode-computer", "taucode-diagnostics"];
pub(crate) const OAUTH_REQUIRED: &str = "This server requires OAuth sign-in, which taucode doesn't support. Use a token/API-key configuration if the server offers one.";

pub(crate) use crate::types::default_mcp_servers as defaults;
pub(crate) use crate::types::{
    McpApproval as Approval, McpCredential as Credential, McpServerEntry, McpTransport as Transport,
};

pub(crate) fn account(id: &str, name: &str) -> Result<String, String> {
    if !valid_slug(id)
        || name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err("Invalid connector credential name".into());
    }
    Ok(format!("mcp:{id}:{name}"))
}
pub(crate) fn validate(entries: &[McpServerEntry], overrides: bool) -> Result<(), String> {
    let mut ids = HashSet::new();
    for s in entries {
        if !valid_slug(&s.id) || !ids.insert(s.id.replace('-', "_")) {
            return Err(
                "Connector IDs must be unique slugs (hyphens and underscores are equivalent)"
                    .into(),
            );
        }
        if RESERVED
            .iter()
            .any(|id| id.replace('-', "_") == s.id.replace('-', "_"))
        {
            if s.transport.is_some()
                || !matches!(
                    s.id.as_str(),
                    "codegraph" | "taucode-browser" | "taucode-computer"
                )
            {
                return Err("Reserved built-in connector ID".into());
            }
        } else if s.transport.is_none() && !overrides {
            return Err("Connector transport is required".into());
        }
        if let Some(t) = &s.transport {
            if s.name.trim().is_empty() {
                return Err("Connector name is required".into());
            }
            let values = match t {
                Transport::Stdio {
                    command,
                    args,
                    cwd,
                    env,
                } => {
                    if command.trim().is_empty()
                        || command.contains('\0')
                        || args.iter().any(|a| a.contains('\0'))
                        || cwd.as_ref().is_some_and(|v| v.contains('\0'))
                    {
                        return Err("Invalid connector command, arguments or directory".into());
                    }
                    for name in env.keys() {
                        if name.starts_with(|c: char| c.is_ascii_digit())
                            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                        {
                            return Err("Invalid environment variable name".into());
                        }
                    }
                    env
                }
                Transport::Http {
                    url,
                    headers,
                    bearer,
                } => {
                    let url = reqwest::Url::parse(url).map_err(|_| "Invalid connector URL")?;
                    if !matches!(url.scheme(), "http" | "https")
                        || url.host_str().is_none()
                        || !url.username().is_empty()
                        || url.password().is_some()
                        || url.fragment().is_some()
                    {
                        return Err(
                            "Use an HTTP(S) URL without embedded credentials or fragment".into(),
                        );
                    }
                    if bearer
                        .as_ref()
                        .is_some_and(|v| !matches!(v, Credential::Secret { secret: true }))
                    {
                        return Err("Bearer tokens must be stored in Keychain".into());
                    }
                    let mut names = HashSet::new();
                    for (name, value) in headers {
                        if !names.insert(name.to_ascii_lowercase())
                            || reqwest::header::HeaderName::from_bytes(name.as_bytes()).is_err()
                        {
                            return Err("Invalid or duplicate HTTP header".into());
                        }
                        if name.eq_ignore_ascii_case("authorization") && bearer.is_some() {
                            return Err("Choose an Authorization header or a bearer token".into());
                        }
                        if let Credential::Value { value } = value {
                            reqwest::header::HeaderValue::from_str(value)
                                .map_err(|_| "Invalid HTTP header value")?;
                        }
                    }
                    headers
                }
            };
            for (name, v) in values {
                account(&s.id, name)?;
                if matches!(v, Credential::Secret { secret: false }) {
                    return Err("Secret markers must be true".into());
                }
                if let Credential::Value { value } = v {
                    if value.contains('\0') {
                        return Err("Invalid connector value".into());
                    }
                }
            }
        }
        if s.tool_allowlist
            .iter()
            .chain(s.tool_denylist.iter())
            .flatten()
            .any(|n| n.is_empty() || n.contains('*'))
        {
            return Err("Tool lists require exact, nonempty names".into());
        }
    }
    Ok(())
}

pub(crate) fn effective(
    app: &AppSettings,
    workspace: &WorkspaceSettings,
    graph: Option<&CodegraphSetup>,
) -> Result<Vec<McpServerEntry>, String> {
    validate(&app.mcp_servers, false)?;
    validate(&workspace.mcp_servers, true)?;
    let mut entries = app.mcp_servers.clone();
    for entry in defaults() {
        if !entries.iter().any(|s| s.id == entry.id) {
            entries.push(entry);
        }
    }
    for over in &workspace.mcp_servers {
        if let Some(entry) = entries.iter_mut().find(|s| s.id == over.id) {
            if over.transport.is_some() {
                return Err(
                    "Workspace overrides may only enable or disable global connectors".into(),
                );
            }
            entry.enabled = over.enabled;
        } else if over.transport.is_some() {
            entries.push(over.clone());
        }
        // A removed global entry may leave an inert enable/disable override.
    }
    validate(&entries, false)?;
    let cg = entries.iter_mut().find(|s| s.id == "codegraph").unwrap();
    // Preserve pre-registry toggles while loading older settings.
    if !workspace.mcp_servers.iter().any(|s| s.id == "codegraph") {
        cg.enabled &= workspace.codegraph_enabled.unwrap_or(true);
    }
    if let Some(g) = graph {
        cg.transport = Some(Transport::Stdio {
            command: g.proxy_exe.to_string_lossy().into(),
            args: [
                vec!["--tau-mcp-proxy".into(), "--".into()],
                g.upstream.clone(),
            ]
            .concat(),
            cwd: None,
            env: BTreeMap::from([
                (
                    "TAUCODE_STATE_DIR".into(),
                    Credential::Value {
                        value: g.state_dir.to_string_lossy().into(),
                    },
                ),
                (
                    "CODEGRAPH_DIR".into(),
                    Credential::Value {
                        value: crate::codegraph::CODEGRAPH_DIR.into(),
                    },
                ),
                (
                    "CODEGRAPH_EXPLORE_DEDUP".into(),
                    Credential::Value { value: "1".into() },
                ),
            ]),
        });
    }
    let browser = entries
        .iter_mut()
        .find(|s| s.id == "taucode-browser")
        .unwrap();
    browser.approval = app.browser.approval.clone();
    if let Some(c) = entries.iter_mut().find(|s| s.id == "taucode-computer") {
        c.enabled = app.computer.enabled
            && !workspace
                .mcp_servers
                .iter()
                .any(|s| s.id == c.id && !s.enabled)
            && !app.mcp_servers.iter().any(|s| s.id == c.id && !s.enabled);
        c.approval = Approval::Auto;
    }
    let features = app.features();
    for entry in &mut entries {
        entry.enabled &= features.connectors && match entry.id.as_str() { "taucode-browser" => features.browser, "taucode-computer" => features.computer_use, _ => true };
    }
    if features.connectors && features.diagnostics && app.diagnostics.expose_to_agents {
        let exe=std::env::current_exe().map_err(|e|e.to_string())?;
        let bin=exe.parent().ok_or("Missing executable directory")?.join(if cfg!(windows) {"taucode-diag-mcp.exe"} else {"taucode-diag-mcp"});
        entries.push(McpServerEntry { id:"taucode-diagnostics".into(),name:"Diagnostics".into(),enabled:true,transport:Some(Transport::Stdio {command:bin.to_string_lossy().into_owned(),args:vec!["--dir".into(),taucode_diag_mcp::core::default_dir().to_string_lossy().into_owned()],cwd:None,env:BTreeMap::new()}),tool_allowlist:None,tool_denylist:None,approval:Approval::Ask });
    }
    Ok(entries)
}

pub(crate) fn load(
    data: &Path,
    workspace: &str,
    graph: Option<&CodegraphSetup>,
) -> Result<Vec<McpServerEntry>, String> {
    let app = crate::storage::read_settings(&data.join("settings.json"))?;
    let workspaces = crate::storage::read_workspaces(&data.join("workspaces.json"))?;
    let mut entries = effective(
        &app,
        &crate::shared::evals::workspace(workspace)
            .or_else(|| workspaces.get(workspace).cloned())
            .map(|w| w.settings)
            .unwrap_or_default(),
        graph,
    )?;
    configure_browser(&mut entries, data)?;
    Ok(entries)
}

pub(crate) fn configure_browser(entries: &mut [McpServerEntry], data: &Path) -> Result<(), String> {
    for (id, flag) in [
        ("taucode-browser", "--tau-browser-mcp"),
        ("taucode-computer", "--tau-computer-mcp"),
    ] {
        if let Some(browser) = entries.iter_mut().find(|s| s.id == id) {
            browser.transport = Some(Transport::Stdio {
                command: std::env::current_exe()
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .into(),
                args: vec![flag.into(), data.to_string_lossy().into()],
                cwd: None,
                env: BTreeMap::new(),
            });
        }
    }
    Ok(())
}

pub(crate) struct Delivery {
    pub pi: Value,
    pub codex: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub secrets: Vec<String>,
}
fn env_name(id: &str, name: &str) -> String {
    // Hex preserves case and punctuation: header names and connector IDs cannot alias secrets.
    format!(
        "TAUCODE_MCP_{}",
        format!("{id}:{name}")
            .bytes()
            .map(|b| format!("{b:02X}"))
            .collect::<String>()
    )
}
pub(crate) fn delivery(
    entries: &[McpServerEntry],
    keys: &dyn KeyStore,
) -> Result<Delivery, String> {
    let mut out = Delivery {
        pi: json!({"mcpServers":{}, "autoEnableCodemode":false}),
        codex: vec!["-c".into(), "mcp_servers={}".into()],
        env: BTreeMap::new(),
        secrets: vec![],
    };
    for s in entries.iter().filter(|s| s.enabled) {
        let Some(transport) = &s.transport else {
            continue;
        };
        let mut pi = json!({"exposure": if s.tool_allowlist.is_some() { "hidden" } else { "direct" }, "toolExposure":{}, "description":s.name});
        let mut codex = toml_edit::InlineTable::new();
        codex.insert(
            "default_tools_approval_mode",
            (if s.approval == Approval::Ask {
                "prompt"
            } else {
                "approve"
            })
            .into(),
        );
        if s.id == "taucode-browser" {
            let mut tools = toml_edit::InlineTable::new();
            for name in super::browser::READ_TOOLS {
                let mut rule = toml_edit::InlineTable::new();
                rule.insert("approval_mode", "approve".into());
                tools.insert(*name, rule.into());
            }
            codex.insert("tools", tools.into());
        }
        for (key, list) in [
            ("enabled_tools", &s.tool_allowlist),
            ("disabled_tools", &s.tool_denylist),
        ] {
            if let Some(list) = list {
                codex.insert(
                    key,
                    list.iter()
                        .map(String::as_str)
                        .collect::<toml_edit::Array>()
                        .into(),
                );
            }
        }
        for name in s.tool_allowlist.iter().flatten() {
            pi["toolExposure"][name] = json!("direct");
        }
        for name in s.tool_denylist.iter().flatten() {
            pi["toolExposure"][name] = json!("hidden");
        }
        let (values, field) = match transport {
            Transport::Stdio {
                command,
                args,
                cwd,
                env,
            } => {
                pi["command"] = json!(command);
                pi["args"] = json!(args);
                codex.insert("command", command.as_str().into());
                codex.insert(
                    "args",
                    args.iter()
                        .map(String::as_str)
                        .collect::<toml_edit::Array>()
                        .into(),
                );
                if let Some(cwd) = cwd {
                    pi["cwd"] = json!(cwd);
                    codex.insert("cwd", cwd.as_str().into());
                }
                (env, "env")
            }
            Transport::Http {
                url,
                headers,
                bearer,
            } => {
                pi["url"] = json!(url);
                codex.insert("url", url.as_str().into());
                if bearer.is_some() {
                    let name = env_name(&s.id, "bearer");
                    out.env.insert(
                        name.clone(),
                        keys.get(&account(&s.id, "bearer")?)?
                            .ok_or("Save the connector bearer token in Keychain")?,
                    );
                    out.secrets.push(out.env[&name].clone());
                    pi["headers"] = json!({"Authorization":format!("Bearer ${{{name}}}")});
                    codex.insert("bearer_token_env_var", name.into());
                }
                (headers, "headers")
            }
        };
        let mut literal = toml_edit::InlineTable::new();
        let mut refs = toml_edit::InlineTable::new();
        let mut env_vars = toml_edit::Array::new();
        for (name, value) in values {
            // Pi treats !value and ${...} as executable configuration. Always interpolate
            // once from a host-owned variable, including user-entered literal values.
            let reference = env_name(&s.id, name);
            let value = match value {
                Credential::Value { value } => value.clone(),
                Credential::Secret { .. } => keys
                    .get(&account(&s.id, name)?)?
                    .ok_or_else(|| format!("Save {name} for {} in Keychain", s.name))?,
            };
            out.env.insert(reference.clone(), value.clone());
            if matches!(values.get(name), Some(Credential::Secret { .. })) {
                out.secrets.push(value.clone());
            }
            if pi.get(field).is_none() {
                pi[field] = json!({});
            }
            pi[field][name] = json!(format!("${{{reference}}}"));
            if field == "env" {
                if matches!(values.get(name), Some(Credential::Secret { .. })) {
                    // Codex cannot rename env_vars. A pre-Tauri exec adapter maps only
                    // this server's references, so multiple API_KEY values stay separate.
                    env_vars.push(reference.as_str());
                    refs.insert(name, reference.into());
                } else {
                    literal.insert(name, value.into());
                }
            } else if matches!(values.get(name), Some(Credential::Secret { .. })) {
                refs.insert(name, reference.into());
            } else {
                literal.insert(name, value.into());
            }
        }
        if s.id == "codegraph" {
            pi["env"]["TAUCODE_SESSION_KEY"] = json!("${TAUCODE_SESSION_KEY}");
        }
        if field == "env" {
            if !refs.is_empty() {
                let Transport::Stdio { command, args, .. } = transport else {
                    unreachable!()
                };
                let mapping: BTreeMap<_, _> = refs
                    .iter()
                    .map(|(name, value)| (name, value.as_str().unwrap()))
                    .collect();
                let mut launch = toml_edit::Array::new();
                launch.push("--tau-mcp-env");
                launch.push(
                    serde_json::to_string(&mapping)
                        .map_err(|_| "Invalid connector environment mapping")?,
                );
                launch.push(command.as_str());
                for arg in args {
                    launch.push(arg.as_str());
                }
                codex.insert(
                    "command",
                    std::env::current_exe()
                        .map_err(|_| "Cannot find connector launcher")?
                        .to_string_lossy()
                        .as_ref()
                        .into(),
                );
                codex.insert("args", launch.into());
            }
            codex.insert("env", literal.into());
            codex.insert("env_vars", env_vars.into());
        } else {
            codex.insert("http_headers", literal.into());
            codex.insert("env_http_headers", refs.into());
        }
        out.pi["mcpServers"][&s.id] = pi;
        out.codex
            .extend(["-c".into(), format!("mcp_servers.{}={codex}", s.id)]);
    }
    Ok(out)
}

#[path = "connectors_test_connection.rs"]
mod test_connection;
pub(crate) use test_connection::test_connection;

#[cfg(test)]
#[path = "connectors_tests.rs"]
mod tests;

pub(crate) async fn request(
    params: Value,
    settings: &tokio::sync::Mutex<AppSettings>,
    workspaces: &tokio::sync::Mutex<
        std::collections::HashMap<String, crate::types::WorkspaceEntry>,
    >,
    graph: Option<&CodegraphSetup>,
    keys: &dyn KeyStore,
    data: &Path,
) -> Result<Value, String> {
    let workspace = match params["workspaceId"].as_str() {
        Some(id) => Some(
            workspaces
                .lock()
                .await
                .get(id)
                .cloned()
                .ok_or("Workspace not found")?,
        ),
        None => None,
    };
    let app = settings.lock().await.clone();
    let mut entries = effective(
        &app,
        &workspace
            .as_ref()
            .map(|w| w.settings.clone())
            .unwrap_or_default(),
        graph,
    )?;
    configure_browser(&mut entries, data)?;
    let action = params["action"]
        .as_str()
        .ok_or("Missing connector action")?;
    if action == "list" {
        return Ok(json!(entries));
    }
    let id = params["id"].as_str().ok_or("Missing connector ID")?;
    let entry = entries
        .iter()
        .find(|s| s.id == id)
        .ok_or("Connector not found")?;
    if action == "test" {
        return Ok(test_connection(
            entry,
            keys,
            Path::new(workspace.as_ref().map(|w| w.path.as_str()).unwrap_or(".")),
        )
        .await);
    }
    let fields = secret_fields(entry);
    if action == "secrets" {
        let mut saved = serde_json::Map::new();
        for name in fields {
            saved.insert(
                name.clone(),
                json!(keys.get(&account(id, &name)?)?.is_some()),
            );
        }
        return Ok(Value::Object(saved));
    }
    let name = params["name"].as_str().ok_or("Missing credential name")?;
    if !fields.iter().any(|n| n == name) {
        return Err("Credential is not declared as a secret in this connector".into());
    }
    let account = account(id, name)?;
    match action {
        "setSecret" => {
            let value = params["value"]
                .as_str()
                .filter(|v| !v.is_empty() && !v.contains('\0'))
                .ok_or("A nonempty credential is required")?;
            keys.set(&account, value)?;
        }
        "deleteSecret" => keys.delete(&account)?,
        _ => return Err("Unknown connector action".into()),
    }
    Ok(Value::Null)
}

pub(crate) fn secret_fields(entry: &McpServerEntry) -> Vec<String> {
    let mut fields = vec![];
    let values = match &entry.transport {
        Some(Transport::Stdio { env, .. }) => env,
        Some(Transport::Http {
            headers, bearer, ..
        }) => {
            if bearer.is_some() {
                fields.push("bearer".into());
            }
            headers
        }
        None => return fields,
    };
    fields.extend(
        values
            .iter()
            .filter(|(_, v)| matches!(v, Credential::Secret { .. }))
            .map(|(name, _)| name.clone()),
    );
    fields
}

pub(crate) fn delete_removed_secrets(
    old: &[McpServerEntry],
    new: &[McpServerEntry],
    keys: &dyn KeyStore,
) -> Result<(), String> {
    for entry in old {
        let retained = new
            .iter()
            .find(|s| s.id == entry.id)
            .map(secret_fields)
            .unwrap_or_default();
        for name in secret_fields(entry)
            .into_iter()
            .filter(|name| !retained.contains(name))
        {
            keys.delete(&account(&entry.id, &name)?)?;
        }
    }
    Ok(())
}

pub(crate) fn validate_workspaces<'a>(
    app: &AppSettings,
    workspaces: impl Iterator<Item = &'a WorkspaceSettings>,
) -> Result<(), String> {
    let mut ids: HashSet<_> = app
        .mcp_servers
        .iter()
        .map(|s| s.id.replace('-', "_"))
        .collect();
    for workspace in workspaces {
        effective(app, workspace, None)?;
        for server in workspace
            .mcp_servers
            .iter()
            .filter(|s| s.transport.is_some())
        {
            if !ids.insert(server.id.replace('-', "_")) {
                return Err(
                    "Workspace-only connector IDs must be unique across all workspaces".into(),
                );
            }
        }
    }
    Ok(())
}
